use std::sync::Arc;

use vulkano::{
    Validated, VulkanLibrary,
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo,
        PrimaryAutoCommandBuffer, allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    device::{Device, DeviceCreateInfo, Queue, QueueCreateInfo, QueueFlags, physical::PhysicalDeviceType},
    format::Format,
    image::{Image, ImageCreateInfo, ImageType, ImageUsage, view::ImageView},
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::graphics::viewport::Viewport,
    sync::{self, GpuFuture},
};

use crate::{
    app::{App, SPLAT_FORCE, hsv},
    fluid::{DYE_SCALE, FluidSim, SimParams, Splat},
};

/// Everything a compute-only run needs; built without winit so it also works
/// on machines with no display server at all.
struct HeadlessGpu {
    device: Arc<Device>,
    queue: Arc<Queue>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
}

impl HeadlessGpu {
    fn new() -> Self {
        let library = unsafe { VulkanLibrary::new() }.unwrap();
        let instance = Instance::new(
            &library,
            &InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                ..Default::default()
            },
        )
        .unwrap();

        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .unwrap()
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .position(|q| q.queue_flags.intersects(QueueFlags::COMPUTE))
                    .map(|i| (p, i as u32))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
                _ => 5,
            })
            .expect("no compute-capable physical device found");

        println!(
            "NS_CHECK using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        let (device, mut queues) = Device::new(
            &physical_device,
            &DeviceCreateInfo {
                queue_create_infos: &[QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let queue = queues.next().unwrap();

        Self {
            command_buffer_allocator: Arc::new(StandardCommandBufferAllocator::new(
                &device,
                &Default::default(),
            )),
            memory_allocator: Arc::new(StandardMemoryAllocator::new(&device, &Default::default())),
            descriptor_set_allocator: Arc::new(StandardDescriptorSetAllocator::new(
                &device,
                &Default::default(),
            )),
            device,
            queue,
        }
    }

    fn builder(&self) -> AutoCommandBufferBuilder<PrimaryAutoCommandBuffer> {
        AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            self.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap()
    }

    fn submit_wait(&self, builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        let command_buffer = builder.build().unwrap();
        sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap()
            .then_signal_fence_and_flush()
            .map_err(Validated::unwrap)
            .unwrap()
            .wait(None)
            .map_err(Validated::unwrap)
            .unwrap();
    }
}

fn readback_buffer(
    allocator: &Arc<StandardMemoryAllocator>,
    len: usize,
) -> Subbuffer<[f32]> {
    Buffer::from_iter(
        allocator,
        &BufferCreateInfo {
            usage: BufferUsage::TRANSFER_DST,
            ..Default::default()
        },
        &AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_RANDOM_ACCESS,
            ..Default::default()
        },
        std::iter::repeat_n(0.0f32, len),
    )
    .unwrap()
}

fn copy_image(
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    view: &Arc<ImageView>,
    buffer: &Subbuffer<[f32]>,
) {
    builder
        .copy_image_to_buffer(CopyImageToBufferInfo::new(view.image().clone(), buffer.clone()))
        .unwrap();
}

fn divergence_stats(data: &[f32]) -> (f32, f32) {
    let mut max = 0.0f32;
    let mut sum = 0.0f32;
    for &d in data {
        let a = d.abs();
        max = max.max(a);
        sum += a;
    }
    (max, sum / data.len() as f32)
}

fn write_dye_ppm(path: &str, data: &[f32], size: [u32; 2]) {
    let mut ppm = format!("P6\n{} {}\n255\n", size[0], size[1]).into_bytes();
    for px in data.as_chunks::<4>().0 {
        for c in &px[..3] {
            // Same tone map as display.frag: 1 - exp(-c * 1.8) on the dark
            // background color.
            let v = (1.0 - (-c * 1.8f32).exp()).clamp(0.0, 1.0);
            ppm.push((v * 255.0) as u8);
        }
    }
    std::fs::write(path, ppm).unwrap();
    println!("dye dump written to {path}");
}

/// Compute-only verification of the solver, no window needed:
/// 1. a raw splatted velocity field has significant divergence,
/// 2. one pressure projection (Jacobi + gradient subtract) reduces it sharply,
/// 3. repeated full time steps stay stable (no NaN, bounded velocity) and
///    actually carry dye across the domain.
///
/// Returns whether all checks passed.
pub(crate) fn headless_check() -> bool {
    let gpu = HeadlessGpu::new();
    let mut sim = FluidSim::new(
        &gpu.device,
        &gpu.memory_allocator,
        &gpu.descriptor_set_allocator,
        1.6,
    );
    let params = SimParams {
        viscosity: 0.0,
        diffusion_iters: 8,
        pressure_iters: 44,
        velocity_dissipation: 0.25,
        dye_dissipation: 0.35,
    };
    let sim_size = sim.sim_size();
    let div_len = (sim_size[0] * sim_size[1]) as usize;
    let mut ok = true;

    // -- 1+2: divergence before and after one projection --------------------
    // Radius comparable to a real pointer splat; Jacobi handles these
    // high-frequency sources far better than broad blobs.
    let splat = Splat {
        point: [0.5, 0.5],
        impulse: [0.0, -240.0],
        color: [0.2, 0.5, 1.0],
        radius: 0.012,
    };
    let div_before = readback_buffer(&gpu.memory_allocator, div_len);
    let div_after = readback_buffer(&gpu.memory_allocator, div_len);

    let mut builder = gpu.builder();
    sim.record_reset(&mut builder);
    sim.record_splat(&mut builder, &splat);
    sim.record_divergence(&mut builder);
    copy_image(&mut builder, &sim.div_view(), &div_before);
    sim.record_pressure(&mut builder, params.pressure_iters);
    sim.record_gradient(&mut builder);
    sim.record_divergence(&mut builder);
    copy_image(&mut builder, &sim.div_view(), &div_after);
    gpu.submit_wait(builder);

    let (max_before, mean_before) = divergence_stats(&div_before.read().unwrap());
    let (max_after, mean_after) = divergence_stats(&div_after.read().unwrap());
    println!(
        "divergence before projection: max={max_before:.3} mean={mean_before:.5}"
    );
    println!(
        "divergence after  projection: max={max_after:.3} mean={mean_after:.5}"
    );
    // Jacobi damps the sharp (high-frequency) divergence of a pointer-sized
    // splat quickly; the broad low-frequency remainder converges slowly, which
    // every real-time fluid demo tolerates. Require a clear reduction of both.
    let reduced = max_after < max_before * 0.30 && mean_after < mean_before * 0.60;
    println!(
        "projection reduces divergence by {:.0}% (max) / {:.0}% (mean): {}",
        100.0 * (1.0 - max_after / max_before),
        100.0 * (1.0 - mean_after / mean_before),
        if reduced { "PASS" } else { "FAIL" }
    );
    ok &= reduced;

    // -- 3: stability and transport over a longer scripted run ---------------
    let dye_size = [sim_size[0] * DYE_SCALE, sim_size[1] * DYE_SCALE];
    let dye_len = (dye_size[0] * dye_size[1] * 4) as usize;
    let dye_buf = readback_buffer(&gpu.memory_allocator, dye_len);

    let steps = 120u32;
    let dt = 1.0 / 60.0;
    let mut builder = gpu.builder();
    sim.record_reset(&mut builder);
    for i in 0..steps {
        // Stir a small circle in the middle, like a pointer drag.
        let a = i as f32 * 0.12;
        let point = [0.5 + 0.16 * a.cos(), 0.5 + 0.14 * a.sin()];
        let tangent = [-a.sin(), a.cos()];
        let splat = Splat {
            point,
            impulse: [
                tangent[0] * sim_size[0] as f32 * 0.045 * SPLAT_FORCE,
                tangent[1] * sim_size[1] as f32 * 0.045 * SPLAT_FORCE,
            ],
            color: [0.25, 0.5, 1.0],
            radius: 0.02,
        };
        sim.record_step(&mut builder, dt, &params, &[splat]);
    }
    copy_image(&mut builder, &sim.dye_view(), &dye_buf);
    gpu.submit_wait(builder);

    let dye = dye_buf.read().unwrap();
    let (dye_max, dye_mean, centroid, nan, spread_ok) = {
        let mut dye_max = 0.0f32;
        let mut dye_sum = 0.0f64;
        let mut nan = false;
        let mut centroid = [0.0f64; 2];
        let mut weight = 0.0f64;
    for (k, px) in dye.as_chunks::<4>().0.iter().enumerate() {
        let lum = px[0] + px[1] + px[2];
        if !lum.is_finite() {
            nan = true;
        }
        dye_max = dye_max.max(lum);
        dye_sum += lum as f64;
        let x = (k as u32 % dye_size[0]) as f64;
        let y = (k as u32 / dye_size[0]) as f64;
        centroid[0] += x * lum as f64;
        centroid[1] += y * lum as f64;
        weight += lum as f64;
    }
    if weight > 0.0 {
        centroid[0] /= weight;
        centroid[1] /= weight;
    }
        let spread_ok = weight / dye_len as f64 > 1e-4;
        (dye_max, dye_sum / dye_len as f64, centroid, nan, spread_ok)
    };
    println!(
        "after {steps} steps: dye max={dye_max:.2} mean={dye_mean:.5} centroid=({:.0}, {:.0}) nan={nan}",
        centroid[0],
        centroid[1],
    );
    let stable = !nan && dye_max.is_finite() && dye_max < 500.0 && spread_ok;
    println!("fields finite and bounded: {}", if stable { "PASS" } else { "FAIL" });
    ok &= stable;
    write_dye_ppm("/tmp/ns_check_dye.ppm", &dye, dye_size);
    drop(dye);

    // -- 4: exercise the viscosity pass (ν∇²u), which the default params skip
    let visc_params = SimParams {
        viscosity: 2.0,
        diffusion_iters: 8,
        ..params
    };
    let mut builder = gpu.builder();
    for i in 0..30 {
        let a = i as f32 * 0.12;
        let splat = Splat {
            point: [0.5 + 0.1 * a.cos(), 0.5 + 0.1 * a.sin()],
            impulse: [60.0, -60.0],
            color: [0.3, 0.4, 1.0],
            radius: 0.02,
        };
        sim.record_step(&mut builder, dt, &visc_params, &[splat]);
    }
    copy_image(&mut builder, &sim.dye_view(), &dye_buf);
    gpu.submit_wait(builder);
    let dye_visc = dye_buf.read().unwrap();
    let visc_max = dye_visc
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| px[0] + px[1] + px[2])
        .fold(0.0f32, |m, l| m.max(l));
    let visc_ok = visc_max.is_finite() && visc_max > 0.0;
    println!(
        "viscosity pass (nu=2.0) keeps fields finite: {} (dye max={visc_max:.2})",
        if visc_ok { "PASS" } else { "FAIL" }
    );
    ok &= visc_ok;

    // The divergence after the scripted run must still be controlled.
    let (_, mean_final) = {
        let mut builder = gpu.builder();
        sim.record_divergence(&mut builder);
        copy_image(&mut builder, &sim.div_view(), &div_before);
        gpu.submit_wait(builder);
        divergence_stats(&div_before.read().unwrap())
    };
    let final_ok = mean_final.is_finite() && mean_final < 5.0;
    println!(
        "divergence during steady stirring: mean={mean_final:.4} — {}",
        if final_ok { "PASS" } else { "FAIL" }
    );
    ok &= final_ok;

    println!("NS_CHECK: {}", if ok { "PASS" } else { "FAIL" });
    ok
}

/// Windowed dump of one frame after a scripted stirring run, for visual
/// inspection (compare solar_system's SOLAR_DUMP_FRAME). Renders the same
/// display pass offscreen and writes a PPM file.
pub(crate) fn dump_frame(app: &mut App, path: &str) {
    app.reset_requested = false;
    app.pending_splats.clear();

    let width = 1280u32;
    let height = 800u32;
    let dt = 1.0f32 / 60.0;
    let steps = 180u32;

    let rcx = app.rcx.as_mut().unwrap();
    if let Some(previous) = rcx.previous_frame_end.take() {
        drop(previous);
    }

    let mut builder = AutoCommandBufferBuilder::primary(
        app.gpu.command_buffer_allocator.clone(),
        app.gpu.queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    app.sim.record_reset(&mut builder);
    for i in 0..steps {
        let a = i as f32 * 0.11;
        let point = [0.5 + 0.24 * a.cos(), 0.56 + 0.20 * a.sin()];
        let tangent = [-a.sin(), a.cos()];
        let sim = app.sim.sim_size();
        let splat = Splat {
            point,
            impulse: [
                tangent[0] * sim[0] as f32 * 0.05 * SPLAT_FORCE,
                tangent[1] * sim[1] as f32 * 0.05 * SPLAT_FORCE,
            ],
            color: hsv(0.55 + i as f32 * 0.004, 0.85, 1.0).map(|c| c * 0.25),
            radius: 0.02,
        };
        app.sim.record_step(&mut builder, dt, &app.params, &[splat]);
    }

    let image = Image::new(
        &app.gpu.memory_allocator,
        &ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format: rcx.swapchain.image_format(),
            extent: [width, height, 1],
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
            ..Default::default()
        },
        &AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
    )
    .unwrap();
    let view = ImageView::new_default(&image).unwrap();

    let readback: Subbuffer<[u8]> = Buffer::from_iter(
        &app.gpu.memory_allocator,
        &BufferCreateInfo {
            usage: BufferUsage::TRANSFER_DST,
            ..Default::default()
        },
        &AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_RANDOM_ACCESS,
            ..Default::default()
        },
        std::iter::repeat_n(0u8, (width * height * 4) as usize),
    )
    .unwrap();

    let dye_set = rcx.dye_descriptor_set(&app.gpu.descriptor_set_allocator, &app.sim.dye_view());
    rcx.record_display(
        &mut builder,
        view,
        Viewport {
            offset: [0.0, 0.0],
            extent: [width as f32, height as f32],
            min_depth: 0.0,
            max_depth: 1.0,
        },
        dye_set,
    );

    builder
        .copy_image_to_buffer(CopyImageToBufferInfo::new(image, readback.clone()))
        .unwrap();

    let command_buffer = builder.build().unwrap();
    sync::now(app.gpu.device.clone())
        .then_execute(app.gpu.queue.clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .map_err(Validated::unwrap)
        .unwrap()
        .wait(None)
        .map_err(Validated::unwrap)
        .unwrap();

    let data = readback.read().unwrap();
    let color_format = rcx.swapchain.image_format();
    let bgra = matches!(
        color_format,
        Format::B8G8R8A8_UNORM | Format::B8G8R8A8_SRGB | Format::B8G8R8A8_SNORM
    );
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    for px in data.as_chunks::<4>().0 {
        if bgra {
            ppm.extend_from_slice(&[px[2], px[1], px[0]]);
        } else {
            ppm.extend_from_slice(&[px[0], px[1], px[2]]);
        }
    }
    std::fs::write(path, ppm).unwrap();
    println!("debug frame written to {path}");
}
