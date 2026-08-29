use std::sync::Arc;

use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderingAttachmentInfo, RenderingInfo,
    },
    descriptor_set::{
        DescriptorImageInfo, DescriptorSet, WriteDescriptorSet,
        allocator::StandardDescriptorSetAllocator,
    },
    device::Device,
    image::{ImageUsage, view::ImageView},
    instance::Instance,
    pipeline::{
        DynamicState, GraphicsPipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo,
        graphics::{
            GraphicsPipelineCreateInfo,
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            multisample::MultisampleState,
            rasterization::RasterizationState,
            subpass::PipelineRenderingCreateInfo,
            vertex_input::VertexInputState,
            viewport::{Viewport, ViewportState},
        },
    },
    render_pass::{AttachmentLoadOp, AttachmentStoreOp},
    swapchain::{Surface, Swapchain, SwapchainCreateInfo},
    sync::{self, GpuFuture},
};
use winit::{dpi::LogicalSize, event_loop::ActiveEventLoop, window::Window};

use crate::shaders::{display_fs, display_vs};

/// Window-side Vulkan objects: swapchain plus the single fullscreen-triangle
/// pipeline that shows the dye field. The compute-side images and pipelines
/// live in `FluidSim` instead.
pub(crate) struct RenderContext {
    pub(crate) window: Arc<Window>,
    pub(crate) swapchain: Arc<Swapchain>,
    pub(crate) attachment_image_views: Vec<Arc<ImageView>>,
    display_pipeline: Arc<GraphicsPipeline>,
    pub(crate) viewport: Viewport,
    pub(crate) recreate_swapchain: bool,
    pub(crate) previous_frame_end: Option<Box<dyn GpuFuture>>,
}

impl RenderContext {
    pub(crate) fn new(
        event_loop: &ActiveEventLoop,
        instance: &Arc<Instance>,
        device: &Arc<Device>,
    ) -> Self {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Navier-Stokes fluid")
                        .with_inner_size(LogicalSize::new(1280.0, 800.0)),
                )
                .unwrap(),
        );
        let surface = Surface::from_window(instance, &window).unwrap();
        let window_size = window.inner_size();

        let (swapchain, images) = {
            let surface_capabilities = device
                .physical_device()
                .surface_capabilities(&surface, &Default::default())
                .unwrap();

            let (image_format, _) = device
                .physical_device()
                .surface_formats(&surface, &Default::default())
                .unwrap()[0];

            Swapchain::new(
                device,
                &surface,
                &SwapchainCreateInfo {
                    min_image_count: surface_capabilities.min_image_count.max(2),
                    image_format,
                    image_extent: window_size.into(),
                    image_usage: ImageUsage::COLOR_ATTACHMENT,
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .into_iter()
                        .next()
                        .unwrap(),
                    ..Default::default()
                },
            )
            .unwrap()
        };

        let attachment_image_views = images
            .iter()
            .map(|image| ImageView::new_default(image).unwrap())
            .collect::<Vec<_>>();

        let vs_entry = unsafe { display_vs::load(device) }
            .unwrap()
            .entry_point("main")
            .unwrap();
        let fs_entry = unsafe { display_fs::load(device) }
            .unwrap()
            .entry_point("main")
            .unwrap();
        let stages = [
            PipelineShaderStageCreateInfo::new(&vs_entry),
            PipelineShaderStageCreateInfo::new(&fs_entry),
        ];
        let layout = PipelineLayout::from_stages(device, &stages).unwrap();

        let subpass = PipelineRenderingCreateInfo {
            color_attachment_formats: &[Some(swapchain.image_format())],
            ..Default::default()
        };

        let display_pipeline = GraphicsPipeline::new(
            device,
            None,
            &GraphicsPipelineCreateInfo {
                stages: &stages,
                // The fullscreen triangle is generated from gl_VertexIndex,
                // so the pipeline has no vertex inputs at all.
                vertex_input_state: Some(&VertexInputState::default()),
                input_assembly_state: Some(&InputAssemblyState {
                    topology: PrimitiveTopology::TriangleList,
                    ..Default::default()
                }),
                viewport_state: Some(&ViewportState::default()),
                rasterization_state: Some(&RasterizationState::default()),
                multisample_state: Some(&MultisampleState::default()),
                color_blend_state: Some(&ColorBlendState {
                    attachments: &[ColorBlendAttachmentState::default()],
                    ..Default::default()
                }),
                dynamic_state: &[DynamicState::Viewport],
                subpass: Some((&subpass).into()),
                ..GraphicsPipelineCreateInfo::new(&layout)
            },
        )
        .unwrap();

        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: window_size.into(),
            min_depth: 0.0,
            max_depth: 1.0,
        };

        Self {
            window,
            swapchain,
            attachment_image_views,
            display_pipeline,
            viewport,
            recreate_swapchain: false,
            previous_frame_end: Some(sync::now(device.clone()).boxed()),
        }
    }

    pub(crate) fn refresh_swapchain(&mut self, extent: [u32; 2]) {
        let (new_swapchain, new_images) = self
            .swapchain
            .recreate(&SwapchainCreateInfo {
                image_extent: extent,
                ..self.swapchain.create_info()
            })
            .expect("failed to recreate swapchain");

        self.swapchain = new_swapchain;
        self.attachment_image_views = new_images
            .iter()
            .map(|image| ImageView::new_default(image).unwrap())
            .collect::<Vec<_>>();
        self.viewport.extent = extent.map(|v| v as f32);
        self.recreate_swapchain = false;
    }

    /// Binds the current dye image to the display pipeline's sampler slot.
    pub(crate) fn dye_descriptor_set(
        &self,
        allocator: &Arc<StandardDescriptorSetAllocator>,
        dye: &Arc<ImageView>,
    ) -> Arc<DescriptorSet> {
        let layout = &self.display_pipeline.layout().set_layouts()[0];
        DescriptorSet::new(
            allocator,
            layout,
            &[
                WriteDescriptorSet::image(
                    0,
                    &DescriptorImageInfo {
                        image_view: Some(dye),
                        ..Default::default()
                    },
                ),
            ],
            &[],
        )
        .unwrap()
    }

    /// Records the fullscreen draw that presents the dye field. Assumes the
    /// compute dispatches of this frame were recorded into the same builder
    /// beforehand; vulkano's automatic synchronization orders them.
    pub(crate) fn record_display(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        attachment: Arc<ImageView>,
        viewport: Viewport,
        dye_set: Arc<DescriptorSet>,
    ) {
        builder
            .begin_rendering(RenderingInfo {
                color_attachments: vec![Some(RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Clear,
                    store_op: AttachmentStoreOp::Store,
                    clear_value: Some([0.012, 0.012, 0.03, 1.0].into()),
                    ..RenderingAttachmentInfo::new(attachment)
                })],
                ..Default::default()
            })
            .unwrap()
            .set_viewport(0, [viewport].into_iter().collect())
            .unwrap()
            .bind_pipeline_graphics(self.display_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.display_pipeline.layout().clone(),
                0,
                dye_set,
            )
            .unwrap();

        // Fullscreen triangle: three vertices, no vertex buffer.
        unsafe { builder.draw(3, 1, 0, 0) }.unwrap();

        builder.end_rendering().unwrap();
    }
}
