use crate::renderer::{
    byte_slice_from,
    vulkan::{
        core::VulkanContext,
        render::{
            DescriptorPool, DescriptorSetLayout, RenderPass, RenderPipeline,
            RenderPipelineSettingsBuilder,
        },
        resource::{
            CommandPool, GeometryBuffer, ShaderCache, ShaderPathSetBuilder, TextureBundle,
            TextureDescription,
        },
    },
};
use ash::{version::DeviceV1_0, vk};
use imgui::{Context, DrawCmd, DrawCmdParams, DrawData};
use log::{debug, warn};
use nalgebra_glm as glm;
use std::{mem, sync::Arc};

pub struct PushConstantBlockGui {
    pub projection: glm::Mat4,
}

pub struct GuiRenderer {
    pub context: Arc<VulkanContext>,
    pub descriptor_set: vk::DescriptorSet,
    pub descriptor_set_layout: Arc<DescriptorSetLayout>,
    pub descriptor_pool: DescriptorPool,
    pub font_texture: TextureBundle,
    pub pipeline: Option<RenderPipeline>,
    pub geometry_buffer: Option<GeometryBuffer>,
}

impl GuiRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        shader_cache: &mut ShaderCache,
        render_pass: Arc<RenderPass>,
        imgui: &mut Context,
        command_pool: &CommandPool,
    ) -> Self {
        debug!("Creating gui renderer");
        let descriptor_set_layout = Arc::new(Self::descriptor_set_layout(context.clone()));
        let descriptor_pool = Self::create_descriptor_pool(context.clone());
        let descriptor_set = descriptor_pool
            .allocate_descriptor_sets(descriptor_set_layout.layout(), 1)
            .unwrap()[0];

        // TODO: Move texture loading out of this class
        let font_texture = {
            let mut fonts = imgui.fonts();
            let atlas_texture = fonts.build_rgba32_texture();
            let atlas_texture_description = TextureDescription {
                format: vk::Format::R8G8B8A8_UNORM,
                width: atlas_texture.width,
                height: atlas_texture.height,
                mip_levels: 1,
                pixels: atlas_texture.data.to_vec(),
            };

            TextureBundle::new(context.clone(), &command_pool, &atlas_texture_description).unwrap()
        };

        Self::update_descriptor_set(context.clone(), descriptor_set, &font_texture);

        let mut gui_renderer = Self {
            context,
            descriptor_set,
            descriptor_set_layout,
            descriptor_pool,
            font_texture,
            pipeline: None,
            geometry_buffer: None,
        };
        gui_renderer.recreate_pipeline(shader_cache, render_pass);
        gui_renderer
    }

    fn update_descriptor_set(
        context: Arc<VulkanContext>,
        descriptor_set: vk::DescriptorSet,
        texture: &TextureBundle,
    ) {
        let image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(texture.view.view())
            .sampler(texture.sampler.sampler())
            .build();
        let image_infos = [image_info];

        let sampler_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_infos)
            .build();

        let descriptor_writes = [sampler_descriptor_write];

        unsafe {
            context
                .logical_device()
                .logical_device()
                .update_descriptor_sets(&descriptor_writes, &[])
        }
    }

    pub fn recreate_pipeline(
        &mut self,
        shader_cache: &mut ShaderCache,
        render_pass: Arc<RenderPass>,
    ) {
        debug!("Recreating gui pipeline");
        let descriptions = Self::vertex_input_descriptions();
        let attributes = Self::vertex_attributes();
        let vertex_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&descriptions)
            .vertex_attribute_descriptions(&attributes)
            .build();

        let push_constant_range = vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .size(mem::size_of::<PushConstantBlockGui>() as u32)
            .build();

        let shader_paths = ShaderPathSetBuilder::default()
            .vertex("assets/shaders/environment/gui.vert.spv")
            .fragment("assets/shaders/environment/gui.frag.spv")
            .build()
            .unwrap();

        let shader_set = shader_cache
            .create_shader_set(self.context.clone(), &shader_paths)
            .unwrap();

        let settings = RenderPipelineSettingsBuilder::default()
            .render_pass(render_pass.clone())
            .vertex_state_info(vertex_state_info)
            .descriptor_set_layout(self.descriptor_set_layout.clone())
            .shader_set(shader_set)
            .push_constant_range(push_constant_range)
            .blended(true)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_test_enabled(false)
            .depth_write_enabled(false)
            .build()
            .expect("Failed to create render pipeline settings");

        let pipeline = RenderPipeline::new(self.context.clone(), settings.clone());
        self.pipeline = Some(pipeline);
    }

    pub fn descriptor_set_layout(context: Arc<VulkanContext>) -> DescriptorSetLayout {
        let sampler_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();

        let bindings = [sampler_binding];

        let layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();

        DescriptorSetLayout::new(context, layout_create_info).unwrap()
    }

    fn create_descriptor_pool(context: Arc<VulkanContext>) -> DescriptorPool {
        let sampler_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        };

        let pool_sizes = [sampler_pool_size];

        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(1)
            .build();

        DescriptorPool::new(context, pool_info).unwrap()
    }

    fn vertex_attributes() -> [vk::VertexInputAttributeDescription; 3] {
        let float_size = std::mem::size_of::<f32>();
        let position_description = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0)
            .build();

        let tex_coord_description = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32_SFLOAT)
            .offset((2 * float_size) as _)
            .build();

        let color_description = vk::VertexInputAttributeDescription::builder()
            .binding(0)
            .location(2)
            .format(vk::Format::R8G8B8A8_UNORM)
            .offset((4 * float_size) as _)
            .build();

        [
            position_description,
            tex_coord_description,
            color_description,
        ]
    }

    fn vertex_input_descriptions() -> [vk::VertexInputBindingDescription; 1] {
        let vertex_input_binding_description = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(20)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build();
        [vertex_input_binding_description]
    }

    fn resize_geometry_buffer(command_pool: &CommandPool, draw_data: &DrawData) -> GeometryBuffer {
        let vertices = draw_data
            .draw_lists()
            .flat_map(|draw_list| draw_list.vtx_buffer())
            .map(|vertex| *vertex)
            .collect::<Vec<_>>();

        let indices = draw_data
            .draw_lists()
            .flat_map(|draw_list| draw_list.idx_buffer())
            .map(|index| *index as u32)
            .collect::<Vec<_>>();

        GeometryBuffer::new(&command_pool, &vertices, Some(&indices))
    }

    pub fn issue_commands(
        &mut self,
        command_pool: &CommandPool,
        command_buffer: vk::CommandBuffer,
        draw_data: &DrawData,
    ) {
        if draw_data.total_vtx_count == 0 {
            return;
        }

        let device = self.context.logical_device();

        // if self.geometry_buffer.is_none() {
        self.geometry_buffer = None;
        let resized_buffer = Self::resize_geometry_buffer(command_pool, draw_data);
        self.geometry_buffer = Some(resized_buffer);
        // }

        // // FIXME: resize vertex and index buffers separately and append vertices
        // if draw_data.total_vtx_count as u32
        //     > self.geometry_buffer.as_ref().unwrap().number_of_vertices
        // {
        //     trace!("Resizing gui vertex buffer");
        //     self.geometry_buffer = None;
        //     let resized_buffer = Self::resize_geometry_buffer(command_pool, draw_data);
        //     self.geometry_buffer = Some(resized_buffer);
        // } else if draw_data.total_idx_count as u32
        //     > self.geometry_buffer.as_ref().unwrap().number_of_indices
        // {
        //     trace!("Resizing gui index buffer");
        //     self.geometry_buffer = None;
        //     let resized_buffer = Self::resize_geometry_buffer(command_pool, draw_data);
        //     self.geometry_buffer = Some(resized_buffer);
        // }

        if let Some(geometry_buffer) = self.geometry_buffer.as_mut() {
            if let Some(pipeline) = self.pipeline.as_ref() {
                pipeline.bind(device.logical_device(), command_buffer);

                let framebuffer_width = draw_data.framebuffer_scale[0] * draw_data.display_size[0];
                let framebuffer_height = draw_data.framebuffer_scale[1] * draw_data.display_size[1];

                let projection =
                    glm::ortho_zo(0.0, framebuffer_width, 0.0, framebuffer_height, -1.0, 1.0);

                let viewport = vk::Viewport {
                    width: framebuffer_width,
                    height: framebuffer_height,
                    max_depth: 1.0,
                    ..Default::default()
                };
                let viewports = [viewport];

                unsafe {
                    device.logical_device().cmd_push_constants(
                        command_buffer,
                        pipeline.pipeline.layout(),
                        vk::ShaderStageFlags::VERTEX,
                        0,
                        byte_slice_from(&PushConstantBlockGui { projection }),
                    );
                }

                unsafe {
                    device
                        .logical_device()
                        .cmd_set_viewport(command_buffer, 0, &viewports);
                }

                geometry_buffer.bind(device.logical_device(), command_buffer);

                // Render draw lists
                // Adapted from: https://github.com/adrien-ben/imgui-rs-vulkan-renderer
                let mut index_offset = 0;
                let mut vertex_offset = 0;
                let clip_offset = draw_data.display_pos;
                let clip_scale = draw_data.framebuffer_scale;
                for draw_list in draw_data.draw_lists() {
                    for command in draw_list.commands() {
                        match command {
                            DrawCmd::Elements {
                                count,
                                cmd_params:
                                    DrawCmdParams {
                                        clip_rect,
                                        texture_id: _texture_id,
                                        vtx_offset,
                                        idx_offset,
                                    },
                            } => {
                                unsafe {
                                    let clip_x = (clip_rect[0] - clip_offset[0]) * clip_scale[0];
                                    let clip_y = (clip_rect[1] - clip_offset[1]) * clip_scale[1];
                                    let clip_w =
                                        (clip_rect[2] - clip_offset[0]) * clip_scale[0] - clip_x;
                                    let clip_h =
                                        (clip_rect[3] - clip_offset[1]) * clip_scale[1] - clip_y;
                                    let scissors = [vk::Rect2D {
                                        offset: vk::Offset2D {
                                            x: clip_x as _,
                                            y: clip_y as _,
                                        },
                                        extent: vk::Extent2D {
                                            width: clip_w as _,
                                            height: clip_h as _,
                                        },
                                    }];
                                    device.logical_device().cmd_set_scissor(
                                        command_buffer,
                                        0,
                                        &scissors,
                                    );
                                }

                                // TODO: Create a map of texture ids to descriptor sets
                                unsafe {
                                    device.logical_device().cmd_bind_descriptor_sets(
                                        command_buffer,
                                        vk::PipelineBindPoint::GRAPHICS,
                                        pipeline.pipeline.layout(),
                                        0,
                                        &[self.descriptor_set],
                                        &[],
                                    )
                                };

                                unsafe {
                                    device.logical_device().cmd_draw_indexed(
                                        command_buffer,
                                        count as _,
                                        1,
                                        index_offset + idx_offset as u32,
                                        vertex_offset + vtx_offset as i32,
                                        0,
                                    )
                                };
                            }
                            _ => (),
                        }
                    }
                    index_offset += draw_list.idx_buffer().len() as u32;
                    vertex_offset += draw_list.vtx_buffer().len() as i32;
                }
            } else {
                warn!("No gui pipeline available");
                return;
            }
        }
    }
}
