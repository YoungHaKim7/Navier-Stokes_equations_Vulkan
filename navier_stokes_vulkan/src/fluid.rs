use std::sync::Arc;

use vulkano::{
    buffer::BufferContents,
    command_buffer::{
        AutoCommandBufferBuilder, ClearColorImageInfo, PrimaryAutoCommandBuffer,
    },
    descriptor_set::{
        DescriptorImageInfo, DescriptorSet, WriteDescriptorSet,
        allocator::StandardDescriptorSetAllocator,
        layout::DescriptorSetLayout,
    },
    device::Device,
    format::Format,
    image::{Image, ImageCreateInfo, ImageType, ImageUsage, view::ImageView},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{
        ComputePipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
        compute::ComputePipelineCreateInfo,
    },
};

use crate::shaders::{advect_cs, diffuse_cs, divergence_cs, gradient_cs, pressure_cs, splat_cs};

/// Height of the velocity/pressure simulation grid, in cells. The width follows
/// the window aspect ratio so that cells stay square and the physics isotropic.
pub(crate) const SIM_BASE: u32 = 256;

/// The dye field lives on a grid this many times finer than the velocity grid,
/// so the visible smoke resolves finer detail than the physics.
pub(crate) const DYE_SCALE: u32 = 2;

const WORKGROUP: u32 = 8;

/// One interaction event: an external force (`f` in the momentum equation)
/// plus a matching blob of dye, injected at a point in unit-square coordinates
/// (x to the right, y down, matching the window).
pub(crate) struct Splat {
    pub(crate) point: [f32; 2],
    /// Velocity impulse in cells per second.
    pub(crate) impulse: [f32; 2],
    pub(crate) color: [f32; 3],
    /// Gaussian radius in unit-square coordinates.
    pub(crate) radius: f32,
}

/// Tunables of the solver, adjustable at runtime.
pub(crate) struct SimParams {
    /// Kinematic viscosity ν (cells²/s) — the `ν∇²u` term. 0 disables the pass.
    pub(crate) viscosity: f32,
    /// Jacobi iterations for the viscosity solve.
    pub(crate) diffusion_iters: u32,
    /// Jacobi iterations for the pressure Poisson solve — more means the
    /// incompressibility constraint ∇·u = 0 is enforced more exactly.
    pub(crate) pressure_iters: u32,
    pub(crate) velocity_dissipation: f32,
    pub(crate) dye_dissipation: f32,
}

/// GPU state of the incompressible Navier-Stokes solver. Each field lives in
/// one or more storage images that the compute kernels ping-pong between:
/// velocity needs three (the fixed `u_old` of the viscous Jacobi solve plus a
/// read/write pair), pressure and dye need two, divergence one.
///
/// Every `record_*` method appends dispatches to the caller's command buffer;
/// vulkano inserts the image barriers between them automatically.
pub(crate) struct FluidSim {
    sim_size: [u32; 2],
    dye_size: [u32; 2],
    velocity: [Arc<ImageView>; 3],
    vel_cur: usize,
    pressure: [Arc<ImageView>; 2],
    pressure_cur: usize,
    div: Arc<ImageView>,
    dye: [Arc<ImageView>; 2],
    dye_cur: usize,
    splat_pipeline: Arc<ComputePipeline>,
    advect_pipeline: Arc<ComputePipeline>,
    diffuse_pipeline: Arc<ComputePipeline>,
    divergence_pipeline: Arc<ComputePipeline>,
    pressure_pipeline: Arc<ComputePipeline>,
    gradient_pipeline: Arc<ComputePipeline>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
}

macro_rules! compute_pipeline {
    ($device:expr, $module:ident) => {{
        let entry = unsafe { $module::load($device) }
            .unwrap()
            .entry_point("main")
            .unwrap();
        let stage = PipelineShaderStageCreateInfo::new(&entry);
        let layout = PipelineLayout::from_stages($device, std::slice::from_ref(&stage)).unwrap();
        ComputePipeline::new(
            $device,
            None,
            &ComputePipelineCreateInfo::new(stage, &layout),
        )
        .unwrap()
    }};
}

fn storage_image(
    memory_allocator: &Arc<StandardMemoryAllocator>,
    format: Format,
    extent: [u32; 2],
) -> Arc<ImageView> {
    ImageView::new_default(
        &Image::new(
            memory_allocator,
            &ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format,
                extent: [extent[0], extent[1], 1],
                usage: ImageUsage::TRANSFER_DST | ImageUsage::TRANSFER_SRC | ImageUsage::STORAGE,
                ..Default::default()
            },
            &AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )
        .unwrap(),
    )
    .unwrap()
}

impl FluidSim {
    pub(crate) fn new(
        device: &Arc<Device>,
        memory_allocator: &Arc<StandardMemoryAllocator>,
        descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>,
        aspect: f32,
    ) -> Self {
        let sim_size = [(SIM_BASE as f32 * aspect).round() as u32, SIM_BASE];
        let dye_size = [sim_size[0] * DYE_SCALE, sim_size[1] * DYE_SCALE];

        Self {
            sim_size,
            dye_size,
            velocity: [
                storage_image(memory_allocator, Format::R32G32_SFLOAT, sim_size),
                storage_image(memory_allocator, Format::R32G32_SFLOAT, sim_size),
                storage_image(memory_allocator, Format::R32G32_SFLOAT, sim_size),
            ],
            vel_cur: 0,
            pressure: [
                storage_image(memory_allocator, Format::R32_SFLOAT, sim_size),
                storage_image(memory_allocator, Format::R32_SFLOAT, sim_size),
            ],
            pressure_cur: 0,
            div: storage_image(memory_allocator, Format::R32_SFLOAT, sim_size),
            dye: [
                storage_image(memory_allocator, Format::R32G32B32A32_SFLOAT, dye_size),
                storage_image(memory_allocator, Format::R32G32B32A32_SFLOAT, dye_size),
            ],
            dye_cur: 0,
            splat_pipeline: compute_pipeline!(device, splat_cs),
            advect_pipeline: compute_pipeline!(device, advect_cs),
            diffuse_pipeline: compute_pipeline!(device, diffuse_cs),
            divergence_pipeline: compute_pipeline!(device, divergence_cs),
            pressure_pipeline: compute_pipeline!(device, pressure_cs),
            gradient_pipeline: compute_pipeline!(device, gradient_cs),
            descriptor_set_allocator: descriptor_set_allocator.clone(),
        }
    }

    pub(crate) fn sim_size(&self) -> [u32; 2] {
        self.sim_size
    }

    /// The dye image holding the current frame's colors, for display.
    pub(crate) fn dye_view(&self) -> Arc<ImageView> {
        self.dye[self.dye_cur].clone()
    }

    /// The divergence image, updated by the last `record_divergence` call.
    pub(crate) fn div_view(&self) -> Arc<ImageView> {
        self.div.clone()
    }

    /// Builds a descriptor set binding the given images to consecutive
    /// bindings. `WriteDescriptorSet` borrows the image infos, so everything
    /// stays scoped to this function's `DescriptorSet::new` call.
    fn set(
        &self,
        layout: &Arc<DescriptorSetLayout>,
        views: &[(u32, &Arc<ImageView>)],
    ) -> Arc<DescriptorSet> {
        let infos: Vec<DescriptorImageInfo> = views
            .iter()
            .map(|(_, view)| DescriptorImageInfo {
                image_view: Some(view),
                ..Default::default()
            })
            .collect();
        let writes: Vec<WriteDescriptorSet> = infos
            .iter()
            .zip(views.iter())
            .map(|(info, (binding, _))| WriteDescriptorSet::image(*binding, info))
            .collect();
        DescriptorSet::new(&self.descriptor_set_allocator, layout, &writes, &[]).unwrap()
    }

    fn bind_and_dispatch(
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        pipeline: &Arc<ComputePipeline>,
        set: Arc<DescriptorSet>,
        size: [u32; 2],
    ) {
        builder
            .bind_pipeline_compute(pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(PipelineBindPoint::Compute, pipeline.layout().clone(), 0, set)
            .unwrap();
        let groups = [size[0].div_ceil(WORKGROUP), size[1].div_ceil(WORKGROUP), 1];
        unsafe { builder.dispatch(groups) }.unwrap();
    }

    fn dispatch(
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        pipeline: &Arc<ComputePipeline>,
        set: Arc<DescriptorSet>,
        push: impl BufferContents,
        size: [u32; 2],
    ) {
        builder
            .push_constants(pipeline.layout().clone(), 0, push)
            .unwrap();
        Self::bind_and_dispatch(builder, pipeline, set, size);
    }

    /// Clears every field to zero. All ping-pong images hold zeros afterwards,
    /// so the index bookkeeping can stay where it is.
    pub(crate) fn record_reset(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        for view in &self.velocity {
            builder
                .clear_color_image(ClearColorImageInfo::new(view.image().clone()))
                .unwrap();
        }
        for view in &self.pressure {
            builder
                .clear_color_image(ClearColorImageInfo::new(view.image().clone()))
                .unwrap();
        }
        builder
            .clear_color_image(ClearColorImageInfo::new(self.div.image().clone()))
            .unwrap();
        for view in &self.dye {
            builder
                .clear_color_image(ClearColorImageInfo::new(view.image().clone()))
                .unwrap();
        }
    }

    /// External force `f`: injects the impulse and dye of one interaction.
    pub(crate) fn record_splat(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        splat: &Splat,
    ) {
        let layout = &self.splat_pipeline.layout().set_layouts()[0];
        let set = self.set(
            layout,
            &[(0, &self.velocity[self.vel_cur]), (1, &self.dye[self.dye_cur])],
        );
        Self::dispatch(
            builder,
            &self.splat_pipeline,
            set.clone(),
            splat_cs::Push {
                point: splat.point,
                impulse: splat.impulse,
                color: [splat.color[0], splat.color[1], splat.color[2], 0.0],
                radius: splat.radius,
                mode: 0,
            },
            self.sim_size,
        );
        Self::dispatch(
            builder,
            &self.splat_pipeline,
            set,
            splat_cs::Push {
                point: splat.point,
                impulse: splat.impulse,
                color: [splat.color[0], splat.color[1], splat.color[2], 0.0],
                radius: splat.radius,
                mode: 1,
            },
            self.dye_size,
        );
    }

    /// Viscosity `ν∇²u`: Jacobi-solves (I - ν·dt·∇²)u_new = u_old, keeping
    /// `u_old` fixed while the iterate ping-pongs between two scratch images.
    pub(crate) fn record_diffuse(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        alpha: f32,
        iters: u32,
    ) {
        let u_old = self.vel_cur;
        let mut x = (u_old + 1) % 3;
        let mut y = (u_old + 2) % 3;
        let layout = &self.diffuse_pipeline.layout().set_layouts()[0];
        let push = diffuse_cs::Push { alpha };

        // First iteration starts from x⁰ = u_old.
        let set = self.set(
            layout,
            &[(0, &self.velocity[u_old]), (1, &self.velocity[u_old]), (2, &self.velocity[x])],
        );
        Self::dispatch(builder, &self.diffuse_pipeline, set, push, self.sim_size);

        for _ in 1..iters {
            let set = self.set(
                layout,
                &[(0, &self.velocity[u_old]), (1, &self.velocity[x]), (2, &self.velocity[y])],
            );
            Self::dispatch(builder, &self.diffuse_pipeline, set, push, self.sim_size);
            std::mem::swap(&mut x, &mut y);
        }
        self.vel_cur = x;
    }

    /// Divergence `∇·u` of the current velocity, into the divergence image.
    pub(crate) fn record_divergence(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        let layout = &self.divergence_pipeline.layout().set_layouts()[0];
        let set = self.set(
            layout,
            &[(0, &self.velocity[self.vel_cur]), (1, &self.div)],
        );
        Self::bind_and_dispatch(builder, &self.divergence_pipeline, set, self.sim_size);
    }

    /// Pressure Poisson solve `∇²p = ∇·u`, `iters` Jacobi iterations,
    /// warm-started from the previous frame's pressure.
    pub(crate) fn record_pressure(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        iters: u32,
    ) {
        let layout = &self.pressure_pipeline.layout().set_layouts()[0];
        let mut p = self.pressure_cur;
        for _ in 0..iters.max(1) {
            let set = self.set(
                layout,
                &[(0, &self.pressure[p]), (1, &self.pressure[1 - p]), (2, &self.div)],
            );
            Self::bind_and_dispatch(builder, &self.pressure_pipeline, set, self.sim_size);
            p = 1 - p;
        }
        self.pressure_cur = p;
    }

    /// Pressure gradient `-(1/ρ)∇p`: projects the velocity onto divergence-free
    /// fields, completing the incompressibility constraint.
    pub(crate) fn record_gradient(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        let out = (self.vel_cur + 1) % 3;
        let layout = &self.gradient_pipeline.layout().set_layouts()[0];
        let set = self.set(
            layout,
            &[
                (0, &self.velocity[self.vel_cur]),
                (1, &self.velocity[out]),
                (2, &self.pressure[self.pressure_cur]),
            ],
        );
        Self::bind_and_dispatch(builder, &self.gradient_pipeline, set, self.sim_size);
        self.vel_cur = out;
    }

    /// Semi-Lagrangian advection of the velocity by itself (`(u·∇)u` term).
    pub(crate) fn record_advect_velocity(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        dt: f32,
        dissipation: f32,
    ) {
        let out = (self.vel_cur + 1) % 3;
        let layout = &self.advect_pipeline.layout().set_layouts()[0];
        let set = self.set(
            layout,
            &[
                (0, &self.velocity[self.vel_cur]),
                (1, &self.velocity[out]),
                (2, &self.dye[self.dye_cur]),
                (3, &self.dye[1 - self.dye_cur]),
            ],
        );
        Self::dispatch(
            builder,
            &self.advect_pipeline,
            set,
            advect_cs::Push {
                dt,
                dissipation,
                mode: 0,
            },
            self.sim_size,
        );
        self.vel_cur = out;
    }

    /// Semi-Lagrangian advection of the dye by the current velocity.
    pub(crate) fn record_advect_dye(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        dt: f32,
        dissipation: f32,
    ) {
        let out = 1 - self.dye_cur;
        let layout = &self.advect_pipeline.layout().set_layouts()[0];
        let set = self.set(
            layout,
            &[
                (0, &self.velocity[self.vel_cur]),
                (1, &self.velocity[(self.vel_cur + 1) % 3]),
                (2, &self.dye[self.dye_cur]),
                (3, &self.dye[out]),
            ],
        );
        Self::dispatch(
            builder,
            &self.advect_pipeline,
            set,
            advect_cs::Push {
                dt,
                dissipation,
                mode: 1,
            },
            self.dye_size,
        );
        self.dye_cur = out;
    }

    /// One full time step of the Navier-Stokes solver:
    /// forces → viscosity → projection (divergence, pressure, gradient) →
    /// advection of velocity and dye.
    pub(crate) fn record_step(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        dt: f32,
        params: &SimParams,
        splats: &[Splat],
    ) {
        for splat in splats {
            self.record_splat(builder, splat);
        }
        if params.viscosity > 0.0 && params.diffusion_iters > 0 {
            self.record_diffuse(builder, params.viscosity * dt, params.diffusion_iters);
        }
        self.record_divergence(builder);
        self.record_pressure(builder, params.pressure_iters);
        self.record_gradient(builder);
        self.record_advect_velocity(builder, dt, params.velocity_dissipation);
        self.record_advect_dye(builder, dt, params.dye_dissipation);
    }
}
