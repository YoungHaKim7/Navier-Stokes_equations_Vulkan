use std::time::Instant;

use vulkano::{
    Validated, VulkanError,
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage},
    swapchain::{SwapchainPresentInfo, acquire_next_image},
    sync::{self, GpuFuture},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowId,
};

use crate::{
    debug,
    fluid::{FluidSim, SimParams, Splat},
    gpu::Gpu,
    renderer::RenderContext,
};

const MIN_SPEED: f64 = 0.1;
const MAX_SPEED: f64 = 16.0;
const DEFAULT_SPEED: f64 = 1.0;
const DEFAULT_WINDOW_SIZE: (f64, f64) = (1280.0, 800.0);

/// How strongly a pointer drag injects momentum, in cells per second of
/// velocity per cell of pointer travel.
pub(crate) const SPLAT_FORCE: f32 = 90.0;
const SPLAT_RADIUS: f32 = 0.022;
/// Dye amount injected per pointer-move event.
const SPLAT_DYE: f32 = 0.22;

pub(crate) struct App {
    pub(crate) gpu: Gpu,
    pub(crate) sim: FluidSim,
    pub(crate) params: SimParams,
    paused: bool,
    sim_speed: f64,
    pointer_pos: Option<(f64, f64)>,
    pointer_down: bool,
    pub(crate) pending_splats: Vec<Splat>,
    pub(crate) reset_requested: bool,
    color_phase: f32,
    last_frame: Instant,
    debug_done: bool,
    pub(crate) rcx: Option<RenderContext>,
}

/// HSV to RGB, for hue-cycling dye colors. h in turns, s/v in [0, 1].
pub(crate) fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = (h.fract() + 1.0).fract() * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i as u32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

impl App {
    pub(crate) fn new(event_loop: &EventLoop<()>) -> Self {
        println!("Navier-Stokes fluid simulation (Stam's stable fluids on Vulkan compute)");
        println!(
            "Controls: drag = stir | space = pause | up/down = time speed | V/B = viscosity\n\
             [ ] = pressure iterations | R = reset | Esc = quit"
        );

        let gpu = Gpu::new(event_loop);
        let aspect = (DEFAULT_WINDOW_SIZE.0 / DEFAULT_WINDOW_SIZE.1) as f32;
        let sim = FluidSim::new(
            &gpu.device,
            &gpu.memory_allocator,
            &gpu.descriptor_set_allocator,
            aspect,
        );
        println!(
            "Simulation grid: {}x{} cells (velocity/pressure), dye at 2x",
            sim.sim_size()[0],
            sim.sim_size()[1]
        );

        App {
            gpu,
            sim,
            params: SimParams {
                viscosity: 0.0,
                diffusion_iters: 8,
                pressure_iters: 44,
                velocity_dissipation: 0.25,
                dye_dissipation: 0.35,
            },
            paused: false,
            sim_speed: DEFAULT_SPEED,
            pointer_pos: None,
            pointer_down: false,
            pending_splats: Vec::new(),
            reset_requested: true,
            color_phase: 0.55,
            last_frame: Instant::now(),
            debug_done: false,
            rcx: None,
        }
    }

    /// A few start-up splats so the fluid is already moving on the first frame.
    fn startup_splats(&mut self) {
        let sim = self.sim.sim_size();
        let jets = [
            ([0.30, 0.62], [110.0, -240.0]),
            ([0.70, 0.62], [-110.0, -240.0]),
            ([0.50, 0.85], [0.0, -330.0]),
        ];
        for (i, (point, impulse)) in jets.iter().enumerate() {
            // One splat is a one-shot addition; repeating it a few times gives
            // the jets enough momentum to curl nicely.
            for k in 0..15 {
                self.pending_splats.push(Splat {
                    point: *point,
                    impulse: [
                        impulse[0] / sim[0] as f32,
                        impulse[1] / sim[1] as f32,
                    ],
                    color: hsv(0.58 + i as f32 * 0.09 + k as f32 * 0.004, 0.85, 1.0)
                        .map(|c| c * SPLAT_DYE * 0.6),
                    radius: SPLAT_RADIUS,
                });
            }
        }
    }

    fn splat_color(&self) -> [f32; 3] {
        hsv(self.color_phase, 0.85, 1.0).map(|c| c * SPLAT_DYE)
    }

    fn set_title(&self) {
        if let Some(rcx) = &self.rcx {
            let speed_label = if self.paused {
                "paused".to_string()
            } else {
                format!("{:.1}x", self.sim_speed)
            };
            let title = format!(
                "Navier-Stokes [{speed_label}] nu={:.2} p-iters={} — drag: stir · space: pause · up/down: speed · V/B: viscosity · [ ]: iterations · R: reset · Esc: quit",
                self.params.viscosity, self.params.pressure_iters
            );
            rcx.window.set_title(&title);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.rcx = Some(RenderContext::new(
            event_loop,
            &self.gpu.instance,
            &self.gpu.device,
        ));
        self.set_title();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                if let Some(rcx) = self.rcx.as_mut() {
                    rcx.recreate_swapchain = true;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard(event, event_loop);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(position.x, position.y);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    self.pointer_down = state == ElementState::Pressed;
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.debug_done && std::env::var_os("NS_DUMP_FRAME").is_some() {
            self.debug_done = true;
            let path = std::env::var("NS_DUMP_FRAME").unwrap();
            debug::dump_frame(self, &path);
            event_loop.exit();
            return;
        }
        if let Some(rcx) = self.rcx.as_ref() {
            rcx.window.request_redraw();
        }
    }
}

impl App {
    fn handle_keyboard(&mut self, event: KeyEvent, event_loop: &ActiveEventLoop) {
        if event.state != ElementState::Pressed {
            return;
        }
        match event.logical_key {
            Key::Named(NamedKey::Space) => {
                self.paused = !self.paused;
                self.last_frame = Instant::now();
                self.set_title();
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.sim_speed = (self.sim_speed * 1.5).min(MAX_SPEED);
                self.set_title();
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.sim_speed = (self.sim_speed / 1.5).max(MIN_SPEED);
                self.set_title();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("v") => {
                self.params.viscosity = (self.params.viscosity / 1.6).max(0.0);
                if self.params.viscosity < 0.01 {
                    self.params.viscosity = 0.0;
                }
                self.set_title();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("b") => {
                self.params.viscosity = if self.params.viscosity == 0.0 {
                    0.05
                } else {
                    (self.params.viscosity * 1.6).min(12.0)
                };
                self.set_title();
            }
            Key::Character(c) if c.as_str() == "[" => {
                self.params.pressure_iters = self.params.pressure_iters.saturating_sub(8).max(8);
                self.set_title();
            }
            Key::Character(c) if c.as_str() == "]" => {
                self.params.pressure_iters = (self.params.pressure_iters + 8).min(160);
                self.set_title();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("r") => {
                self.reset_requested = true;
                self.pending_splats.clear();
                self.last_frame = Instant::now();
            }
            Key::Named(NamedKey::Escape) => {
                event_loop.exit();
            }
            _ => {}
        }
    }

    fn handle_cursor_moved(&mut self, x: f64, y: f64) {
        let Some(rcx) = self.rcx.as_ref() else {
            return;
        };
        let size = rcx.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        // Unit-square coordinates, y down, matching the simulation grids.
        let pos = (x / size.width as f64, y / size.height as f64);

        if self.pointer_down
            && let Some(prev) = self.pointer_pos
            && self.pointer_pos != Some(pos)
        {
            let sim = self.sim.sim_size();
            let du = pos.0 - prev.0;
            let dv = pos.1 - prev.1;
            self.pending_splats.push(Splat {
                point: [pos.0 as f32, pos.1 as f32],
                impulse: [
                    (du * sim[0] as f64 * SPLAT_FORCE as f64) as f32,
                    (dv * sim[1] as f64 * SPLAT_FORCE as f64) as f32,
                ],
                color: self.splat_color(),
                radius: SPLAT_RADIUS,
            });
            self.color_phase = (self.color_phase + 0.012) % 1.0;
        }

        self.pointer_pos = Some(pos);
    }

    fn redraw(&mut self) {
        let window_size = match self.rcx.as_ref() {
            Some(rcx) => rcx.window.inner_size(),
            None => return,
        };

        if window_size.width == 0 || window_size.height == 0 {
            return;
        }

        let now = Instant::now();
        let dt_real = now.duration_since(self.last_frame).as_secs_f64().min(0.05);
        self.last_frame = now;
        let dt = if self.paused { 0.0 } else { dt_real * self.sim_speed };

        let do_reset = self.reset_requested;
        if do_reset {
            self.reset_requested = false;
            self.color_phase = 0.55;
            self.startup_splats();
        }

        let stepping = dt > 0.0 || !self.pending_splats.is_empty();
        let splats = std::mem::take(&mut self.pending_splats);

        let rcx = self.rcx.as_mut().unwrap();

        rcx.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if rcx.recreate_swapchain {
            rcx.refresh_swapchain(window_size.into());
        }

        if let Some(previous) = rcx.previous_frame_end.take() {
            drop(previous);
        }

        let (image_index, suboptimal, acquire_future) =
            match acquire_next_image(rcx.swapchain.clone(), None).map_err(Validated::unwrap) {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    rcx.recreate_swapchain = true;
                    rcx.previous_frame_end = Some(sync::now(self.gpu.device.clone()).boxed());
                    return;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };

        if suboptimal {
            rcx.recreate_swapchain = true;
        }

        let mut builder = AutoCommandBufferBuilder::primary(
            self.gpu.command_buffer_allocator.clone(),
            self.gpu.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        if do_reset {
            self.sim.record_reset(&mut builder);
        }
        if stepping {
            self.sim
                .record_step(&mut builder, dt as f32, &self.params, &splats);
        }

        let dye_set =
            rcx.dye_descriptor_set(&self.gpu.descriptor_set_allocator, &self.sim.dye_view());
        rcx.record_display(
            &mut builder,
            rcx.attachment_image_views[image_index as usize].clone(),
            rcx.viewport.clone(),
            dye_set,
        );

        let command_buffer = builder.build().unwrap();

        let future = sync::now(self.gpu.device.clone())
            .join(acquire_future)
            .then_execute(self.gpu.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(
                self.gpu.queue.clone(),
                SwapchainPresentInfo::new(rcx.swapchain.clone(), image_index),
            )
            .then_signal_fence_and_flush();

        match future.map_err(Validated::unwrap) {
            Ok(future) => {
                rcx.previous_frame_end = Some(future.boxed());
            }
            Err(VulkanError::OutOfDate) => {
                rcx.recreate_swapchain = true;
                rcx.previous_frame_end = Some(sync::now(self.gpu.device.clone()).boxed());
            }
            Err(e) => {
                println!("failed to flush future: {e}");
                rcx.previous_frame_end = Some(sync::now(self.gpu.device.clone()).boxed());
            }
        }
    }
}
