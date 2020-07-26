use crate::{
    camera::OrbitalCamera,
    renderer::{
        vulkan::{
            core::{
                sync::synchronization_set::{SynchronizationSet, SynchronizationSetConstants},
                VulkanContext,
            },
            gui::GuiRenderer,
            pbr::PbrScene,
            render::{Framebuffer, RenderPass, Swapchain, SwapchainProperties},
            resource::{
                image::{ImageView, Texture},
                CommandPool, ShaderCache,
            },
        },
        Renderer,
    },
    system::System,
};
use anyhow::Result;
use ash::vk;
use imgui::{Context, DrawData};
use legion::prelude::*;
use log::warn;
use nalgebra_glm as glm;
use std::sync::Arc;
use winit::window::Window;

pub struct VulkanRenderer {
    context: Arc<VulkanContext>,
    synchronization_set: SynchronizationSet,
    command_pool: CommandPool,
    transient_command_pool: CommandPool,
    swapchain: Option<Swapchain>,
    handles: Option<ForwardRenderingHandles>,
    current_frame: usize,
    scene: Option<PbrScene>,
    shader_cache: ShaderCache,
    gui_renderer: Option<GuiRenderer>,
}

impl VulkanRenderer {
    pub fn new(window: &mut Window) -> Result<Self> {
        let context = Arc::new(VulkanContext::new(&window)?);

        let synchronization_set = SynchronizationSet::new(context.clone())?;

        let command_pool = CommandPool::new(
            context.clone(),
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        )?;

        let transient_command_pool =
            CommandPool::new(context.clone(), vk::CommandPoolCreateFlags::TRANSIENT)?;

        let logical_size = window.inner_size();
        let dimensions = [logical_size.width as u32, logical_size.height as u32];

        let swapchain = Swapchain::new(context.clone(), dimensions)?;

        let handles = ForwardRenderingHandles::new(
            context.clone(),
            &transient_command_pool,
            &swapchain,
            vk::SampleCountFlags::TYPE_1,
        )
        .unwrap();

        let renderer = Self {
            context,
            synchronization_set,
            command_pool,
            transient_command_pool,
            swapchain: Some(swapchain),
            handles: Some(handles),
            current_frame: 0,
            scene: None,
            shader_cache: ShaderCache::default(),
            gui_renderer: None,
        };

        Ok(renderer)
    }

    fn recreate_swapchain(
        &mut self,
        window_dimensions: &glm::Vec2,
        draw_data: &DrawData,
    ) -> Result<()> {
        self.context.logical_device().wait_idle();

        self.swapchain = None;

        let swapchain = Swapchain::new(
            self.context.clone(),
            [window_dimensions.x as _, window_dimensions.y as _],
        )?;
        self.swapchain = Some(swapchain);

        self.handles = None;
        let handles = ForwardRenderingHandles::new(
            self.context.clone(),
            &self.transient_command_pool,
            self.swapchain(),
            vk::SampleCountFlags::TYPE_1,
        )
        .expect("Failed to create strategy handles");
        self.handles = Some(handles);

        let extent = self.swapchain().properties().extent;
        self.record_all_command_buffers(&extent, draw_data);

        Ok(())
    }

    fn swapchain(&self) -> &Swapchain {
        // FIXME: Use a result here
        self.swapchain.as_ref().expect("Failed to get swapchain!")
    }

    fn record_all_command_buffers(&mut self, extent: &vk::Extent2D, draw_data: &DrawData) {
        let command_buffers = self
            .command_pool
            .command_buffers()
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();

        for (index, command_buffer) in command_buffers {
            let framebuffer = self.handles.as_ref().unwrap().framebuffers[index].framebuffer();
            self.record_single_command_buffer(extent, framebuffer, command_buffer, draw_data);
        }
    }

    fn record_single_command_buffer(
        &mut self,
        extent: &vk::Extent2D,
        framebuffer: vk::Framebuffer,
        command_buffer: vk::CommandBuffer,
        draw_data: &DrawData,
    ) {
        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.39, 0.58, 0.93, 1.0],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];

        let context = self.context.clone();
        let render_pass = self.handles.as_ref().unwrap().render_pass.render_pass();
        context.logical_device().record_command_buffer(
            command_buffer,
            vk::CommandBufferUsageFlags::SIMULTANEOUS_USE,
            || {
                let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                    .render_pass(render_pass)
                    .framebuffer(framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: *extent,
                    })
                    .clear_values(&clear_values)
                    .build();

                RenderPass::record(
                    context.clone(),
                    command_buffer,
                    &render_pass_begin_info,
                    || {
                        context
                            .logical_device()
                            .update_viewport(command_buffer, *extent);

                        if let Some(scene) = self.scene.as_mut() {
                            scene.issue_commands(command_buffer).unwrap();
                        } else {
                            warn!("Scene not loaded!");
                        }

                        if let Some(gui_renderer) = self.gui_renderer.as_mut() {
                            gui_renderer.issue_commands(
                                &self.transient_command_pool,
                                command_buffer,
                                draw_data,
                            );
                        } else {
                            warn!("No gui available!");
                        }
                    },
                );
            },
        );
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        self.context.logical_device().wait_idle();
    }
}

impl Renderer for VulkanRenderer {
    fn initialize(&mut self, mut imgui: &mut Context) {
        let asset_names = vec![
            "assets/models/DamagedHelmet.glb",
            "assets/models/CesiumMan.glb",
            "assets/models/AlphaBlendModeTest.glb",
            "assets/models/MetalRoughSpheres.glb",
        ];

        let render_pass = self.handles.as_ref().unwrap().render_pass.clone();
        let scene_data = PbrScene::new(
            self.context.clone(),
            &self.transient_command_pool,
            &mut self.shader_cache,
            render_pass,
            &asset_names,
            vk::SampleCountFlags::TYPE_1,
        );

        self.command_pool
            .allocate_command_buffers(self.handles.as_ref().unwrap().framebuffers.len() as _)
            .unwrap();
        self.scene = Some(scene_data);

        let render_pass = self.handles.as_ref().unwrap().render_pass.clone();

        let gui_renderer = GuiRenderer::new(
            self.context.clone(),
            &mut self.shader_cache,
            render_pass,
            &mut imgui,
            &self.transient_command_pool,
        );
        self.gui_renderer = Some(gui_renderer);
    }

    fn update(&mut self, world: &World, resources: &Resources) {
        // FIXME: Move this to the system struct
        let projection = glm::perspective_zo(
            self.swapchain().properties().aspect_ratio(),
            70_f32.to_radians(),
            0.1_f32,
            1000_f32,
        );

        let camera = &<Read<OrbitalCamera>>::query()
            .iter(world)
            .collect::<Vec<_>>()[0];

        let camera_position = camera.position();
        let view_matrix = camera.view_matrix();

        let system = resources
            .get_mut::<System>()
            .expect("Failed to get system resource!");

        self.scene.as_mut().unwrap().update(
            camera_position,
            projection,
            view_matrix,
            system.delta_time,
        );
    }

    fn render(&mut self, window_dimensions: &glm::Vec2, draw_data: &DrawData) {
        let current_frame_synchronization = self
            .synchronization_set
            .current_frame_synchronization(self.current_frame);

        self.context
            .logical_device()
            .wait_for_fence(&current_frame_synchronization);

        let image_index_result = self.swapchain().acquire_next_image(
            current_frame_synchronization.image_available(),
            vk::Fence::null(),
        );

        let image_index = match image_index_result {
            Ok((image_index, _)) => image_index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(&window_dimensions, draw_data)
                    .expect("Failed to recreate swapchain!");
                return;
            }
            Err(error) => panic!("Error while acquiring next image. Cause: {}", error),
        };
        let image_indices = [image_index];

        self.context
            .logical_device()
            .reset_fence(&current_frame_synchronization);

        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

        let extent = self.swapchain().properties().extent;
        self.record_all_command_buffers(&extent, draw_data);

        self.command_pool
            .submit_command_buffer(
                image_index as usize,
                self.context.graphics_queue(),
                &wait_stages,
                &current_frame_synchronization,
            )
            .unwrap();

        let swapchain_presentation_result = self.swapchain().present_rendered_image(
            &current_frame_synchronization,
            &image_indices,
            self.context.present_queue(),
        );

        match swapchain_presentation_result {
            Ok(is_suboptimal) if is_suboptimal => {
                self.recreate_swapchain(&window_dimensions, draw_data)
                    .expect("Failed to recreate swapchain!");
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(&window_dimensions, draw_data)
                    .expect("Failed to recreate swapchain!");
            }
            Err(error) => panic!("Failed to present queue. Cause: {}", error),
            _ => {}
        }

        self.current_frame +=
            (1 + self.current_frame) % SynchronizationSet::MAX_FRAMES_IN_FLIGHT as usize;
    }
}

pub struct ForwardRenderingHandles {
    pub render_pass: Arc<RenderPass>,
    pub depth_texture: Texture,
    pub depth_texture_view: ImageView,
    pub color_texture: Texture,
    pub color_texture_view: ImageView,
    pub framebuffers: Vec<Framebuffer>,
}

impl ForwardRenderingHandles {
    pub fn new(
        context: Arc<VulkanContext>,
        command_pool: &CommandPool,
        swapchain: &Swapchain,
        samples: vk::SampleCountFlags,
    ) -> Result<Self> {
        let depth_format = context.determine_depth_format(
            vk::ImageTiling::OPTIMAL,
            vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
        );

        let render_pass = Arc::new(Self::create_render_pass(
            context.clone(),
            &swapchain.properties(),
            depth_format,
            samples,
        ));

        let swapchain_extent = swapchain.properties().extent;

        let depth_texture =
            Self::create_depth_texture(context.clone(), swapchain_extent, depth_format, samples);
        let depth_texture_view =
            Self::create_depth_texture_view(context.clone(), &depth_texture, depth_format);

        let color_format = swapchain.properties().format.format;
        let color_texture =
            Self::create_color_texture(context.clone(), swapchain_extent, color_format, samples);
        let color_texture_view =
            Self::create_color_texture_view(context.clone(), &color_texture, color_format);

        let framebuffers = Self::create_framebuffers(
            context,
            &swapchain,
            &color_texture_view,
            &depth_texture_view,
            &render_pass,
        );

        let handles = ForwardRenderingHandles {
            render_pass,
            depth_texture,
            depth_texture_view,
            color_texture,
            color_texture_view,
            framebuffers,
        };

        Ok(handles)
    }

    fn create_render_pass(
        context: Arc<VulkanContext>,
        swapchain_properties: &SwapchainProperties,
        depth_format: vk::Format,
        samples: vk::SampleCountFlags,
    ) -> RenderPass {
        let color_attachment_description = vk::AttachmentDescription::builder()
            .format(swapchain_properties.format.format)
            .samples(samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();

        let depth_attachment_description = vk::AttachmentDescription::builder()
            .format(depth_format)
            .samples(samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let resolve_attachment_description = vk::AttachmentDescription::builder()
            .format(swapchain_properties.format.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();

        let attachment_descriptions = [
            color_attachment_description,
            depth_attachment_description,
            resolve_attachment_description,
        ];

        let color_attachment_reference = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();
        let color_attachment_references = [color_attachment_reference];

        let depth_attachment_reference = vk::AttachmentReference::builder()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let resolve_attachment_description = vk::AttachmentReference::builder()
            .attachment(2)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();
        let resolve_attachment_references = [resolve_attachment_description];

        let subpass_description = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_references)
            .resolve_attachments(&resolve_attachment_references)
            .depth_stencil_attachment(&depth_attachment_reference)
            .build();
        let subpass_descriptions = [subpass_description];

        let subpass_dependencies = [
            vk::SubpassDependency::builder()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::BOTTOM_OF_PIPE)
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(vk::AccessFlags::MEMORY_READ)
                .dst_access_mask(
                    vk::AccessFlags::COLOR_ATTACHMENT_READ
                        | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                )
                .build(),
            vk::SubpassDependency::builder()
                .src_subpass(0)
                .dst_subpass(vk::SUBPASS_EXTERNAL)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags::BOTTOM_OF_PIPE)
                .src_access_mask(
                    vk::AccessFlags::COLOR_ATTACHMENT_READ
                        | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                )
                .dst_access_mask(vk::AccessFlags::MEMORY_READ)
                .build(),
        ];

        let create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachment_descriptions)
            .subpasses(&subpass_descriptions)
            .dependencies(&subpass_dependencies)
            .build();

        RenderPass::new(context, &create_info).unwrap()
    }

    fn create_framebuffers(
        context: Arc<VulkanContext>,
        swapchain: &Swapchain,
        color_texture_view: &ImageView,
        depth_texture_view: &ImageView,
        render_pass: &RenderPass,
    ) -> Vec<Framebuffer> {
        swapchain
            .image_views()
            .iter()
            .map(|view| {
                [
                    color_texture_view.view(),
                    depth_texture_view.view(),
                    view.view(),
                ]
            })
            .map(|attachments| {
                let create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(render_pass.render_pass())
                    .attachments(&attachments)
                    .width(swapchain.properties().extent.width)
                    .height(swapchain.properties().extent.height)
                    .layers(1)
                    .build();
                Framebuffer::new(context.clone(), create_info).unwrap()
            })
            .collect::<Vec<_>>()
    }

    fn create_depth_texture(
        context: Arc<VulkanContext>,
        swapchain_extent: vk::Extent2D,
        depth_format: vk::Format,
        samples: vk::SampleCountFlags,
    ) -> Texture {
        let image_create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: swapchain_extent.width,
                height: swapchain_extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(depth_format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(samples)
            .flags(vk::ImageCreateFlags::empty())
            .build();

        let image_allocation_create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::GpuOnly,
            ..Default::default()
        };
        Texture::new(context, &image_allocation_create_info, &image_create_info).unwrap()
    }

    fn create_depth_texture_view(
        context: Arc<VulkanContext>,
        depth_texture: &Texture,
        depth_format: vk::Format,
    ) -> ImageView {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(depth_texture.image())
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(depth_format)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            })
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .build();
        ImageView::new(context, create_info).unwrap()
    }

    fn create_color_texture(
        context: Arc<VulkanContext>,
        swapchain_extent: vk::Extent2D,
        color_format: vk::Format,
        samples: vk::SampleCountFlags,
    ) -> Texture {
        let image_create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: swapchain_extent.width,
                height: swapchain_extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(color_format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(
                vk::ImageUsageFlags::TRANSIENT_ATTACHMENT | vk::ImageUsageFlags::COLOR_ATTACHMENT,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(samples)
            .flags(vk::ImageCreateFlags::empty())
            .build();

        let image_allocation_create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::GpuOnly,
            ..Default::default()
        };
        Texture::new(context, &image_allocation_create_info, &image_create_info).unwrap()
    }

    fn create_color_texture_view(
        context: Arc<VulkanContext>,
        color_texture: &Texture,
        color_format: vk::Format,
    ) -> ImageView {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(color_texture.image())
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(color_format)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            })
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .build();
        ImageView::new(context, create_info).unwrap()
    }
}
