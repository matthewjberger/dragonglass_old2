use crate::{
    renderer::{
        vulkan::{
            core::{
                sync::synchronization_set::{SynchronizationSet, SynchronizationSetConstants},
                VulkanContext,
            },
            gui::GuiRenderer,
            handles::{ForwardRenderingHandles, Offscreen},
            pbr::PbrScene,
            render::{RenderPass, Swapchain},
            resource::{CommandPool, ShaderCache},
        },
        AssetName, Renderer,
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

        let mut shader_cache = ShaderCache::default();

        let mut handles = ForwardRenderingHandles::new(context.clone(), &swapchain).unwrap();
        handles.recreate_pipeline(&mut shader_cache);

        let renderer = Self {
            context,
            synchronization_set,
            command_pool,
            transient_command_pool,
            swapchain: Some(swapchain),
            handles: Some(handles),
            current_frame: 0,
            scene: None,
            shader_cache,
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
        let mut handles = ForwardRenderingHandles::new(self.context.clone(), self.swapchain())
            .expect("Failed to create strategy handles");
        handles.recreate_pipeline(&mut self.shader_cache);
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

        let (offscreen_framebuffer, offscreen_render_pass) = {
            let offscreen = &self.handles.as_ref().unwrap().offscreen;

            (
                offscreen.framebuffer.framebuffer(),
                offscreen.render_pass.render_pass(),
            )
        };

        context.logical_device().record_command_buffer(
            command_buffer,
            vk::CommandBufferUsageFlags::SIMULTANEOUS_USE,
            || {
                // Render the scene
                let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                    .render_pass(offscreen_render_pass)
                    .framebuffer(offscreen_framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: Offscreen::extent(),
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
                            .update_viewport(command_buffer, Offscreen::extent());

                        if let Some(scene) = self.scene.as_mut() {
                            scene.issue_commands(command_buffer).unwrap();
                        } else {
                            warn!("Scene not loaded!");
                        }
                    },
                );

                // Post-Processing and Gui
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

                        if let Some(handles) = self.handles.as_ref() {
                            handles.issue_commands(command_buffer);
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
    fn initialize(&mut self, world: &World, mut imgui: &mut Context) {
        let asset_names = &<Read<AssetName>>::query()
            .iter(world)
            .map(|asset_name| asset_name.0.to_string())
            .collect::<Vec<_>>();

        let offscreen_render_pass = self.handles.as_ref().unwrap().offscreen.render_pass.clone();
        let scene_data = PbrScene::new(
            self.context.clone(),
            &self.transient_command_pool,
            &mut self.shader_cache,
            offscreen_render_pass,
            asset_names,
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

    fn render(&mut self, world: &World, resources: &Resources, draw_data: &DrawData) {
        let projection = glm::perspective_zo(
            self.swapchain().properties().aspect_ratio(),
            70_f32.to_radians(),
            0.1_f32,
            1000_f32,
        );

        // FIXME: Move this to the system struct
        self.scene
            .as_mut()
            .unwrap()
            .update(world, resources, projection);

        let system = resources
            .get::<System>()
            .expect("Failed to get system resource!");

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
                self.recreate_swapchain(&system.window_dimensions, draw_data)
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
                self.recreate_swapchain(&system.window_dimensions, draw_data)
                    .expect("Failed to recreate swapchain!");
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(&system.window_dimensions, draw_data)
                    .expect("Failed to recreate swapchain!");
            }
            Err(error) => panic!("Failed to present queue. Cause: {}", error),
            _ => {}
        }

        self.current_frame +=
            (1 + self.current_frame) % SynchronizationSet::MAX_FRAMES_IN_FLIGHT as usize;
    }
}
