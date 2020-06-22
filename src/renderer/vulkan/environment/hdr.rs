use crate::renderer::{
    byte_slice_from,
    vulkan::{
        core::VulkanContext,
        environment::{Offscreen, UnitCube},
        render::{
            DescriptorPool, DescriptorSetLayout, Framebuffer, RenderPass, RenderPipeline,
            RenderPipelineSettingsBuilder,
        },
        resource::{
            image::{Cubemap, ImageLayoutTransition, TextureBundle, TextureDescription},
            CommandPool, ShaderCache, ShaderPathSetBuilder,
        },
    },
};
use ash::{version::DeviceV1_0, vk};
use nalgebra_glm as glm;
use snafu::Snafu;
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create render pipeline: {}", source))]
    CreateRenderPipeline {
        source: crate::renderer::vulkan::resource::shader::Error,
    },

    #[snafu(display("Failed to create shader: {}", source))]
    CreateShader {
        source: crate::renderer::vulkan::resource::shader::Error,
    },

    #[snafu(display("Failed to create shader set: {}", source))]
    CreateShaderSet {
        source: crate::renderer::vulkan::resource::shader::Error,
    },
}

#[allow(dead_code)]
struct PushBlockHdr {
    mvp: glm::Mat4,
}

pub struct HdrCubemap {
    pub cubemap: Cubemap,
}

impl HdrCubemap {
    pub fn new(
        context: Arc<VulkanContext>,
        command_pool: &CommandPool,
        path: &str,
        shader_cache: &mut ShaderCache,
    ) -> Result<Self> {
        let description = TextureDescription::from_hdr(path).unwrap();
        let hdr_texture_bundle =
            TextureBundle::new(context.clone(), &command_pool, &description).unwrap();

        let dimension = description.width;
        let format = vk::Format::R32G32B32A32_SFLOAT;
        let output_cubemap = Cubemap::new(context.clone(), dimension, format).unwrap();

        let render_pass = Arc::new(Self::create_render_pass(context.clone(), format));

        let offscreen = Offscreen::new(context.clone(), dimension, format);

        let attachments = [offscreen.view.view()];
        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass.render_pass())
            .attachments(&attachments)
            .width(dimension)
            .height(dimension)
            .layers(1)
            .build();
        let framebuffer = Framebuffer::new(context.clone(), create_info).unwrap();

        let transition = ImageLayoutTransition {
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            src_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            dst_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
        };

        offscreen
            .texture
            .transition(&command_pool, &transition, 1)
            .unwrap();

        let descriptor_set_layout = Arc::new(Self::create_descriptor_set_layout(context.clone()));
        let descriptor_pool = Self::create_descriptor_pool(context.clone());
        let descriptor_set = descriptor_pool
            .allocate_descriptor_sets(descriptor_set_layout.layout(), 1)
            .unwrap()[0];

        Self::update_descriptor_set(context.clone(), descriptor_set, &hdr_texture_bundle);

        let descriptions = UnitCube::vertex_input_descriptions();
        let attributes = UnitCube::vertex_attributes();
        let vertex_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&descriptions)
            .vertex_attribute_descriptions(&attributes)
            .build();

        let push_constant_range = vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .size(std::mem::size_of::<PushBlockHdr>() as u32)
            .build();

        let shader_paths = ShaderPathSetBuilder::default()
            .vertex("assets/shaders/environment/filtercube.vert.spv")
            .fragment("assets/shaders/environment/equirectangular_to_cubemap.frag.spv")
            .build()
            .unwrap();
        let shader_set = shader_cache
            .create_shader_set(context.clone(), &shader_paths)
            .unwrap();

        let settings = RenderPipelineSettingsBuilder::default()
            .render_pass(render_pass.clone())
            .vertex_state_info(vertex_state_info)
            .descriptor_set_layout(descriptor_set_layout)
            .shader_set(shader_set)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .push_constant_range(push_constant_range)
            .build()
            .expect("Failed to create render pipeline settings!");

        let render_pipeline = RenderPipeline::new(context.clone(), settings);

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 0.0],
            },
        }];

        let extent = vk::Extent2D::builder()
            .width(dimension)
            .height(dimension)
            .build();

        let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(render_pass.render_pass())
            .framebuffer(framebuffer.framebuffer())
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .clear_values(&clear_values)
            .build();

        let device = context.logical_device().logical_device();

        let projection = glm::perspective_zo(1.0, 90_f32.to_radians(), 0.1_f32, 10_f32);

        let matrices = vec![
            glm::look_at(
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(1.0, 0.0, 0.0),
                &glm::vec3(0.0, -1.0, 0.0),
            ),
            glm::look_at(
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(-1.0, 0.0, 0.0),
                &glm::vec3(0.0, -1.0, 0.0),
            ),
            glm::look_at(
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 1.0, 0.0),
                &glm::vec3(0.0, 0.0, 1.0),
            ),
            glm::look_at(
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, -1.0, 0.0),
                &glm::vec3(0.0, 0.0, -1.0),
            ),
            glm::look_at(
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 0.0, 1.0),
                &glm::vec3(0.0, -1.0, 0.0),
            ),
            glm::look_at(
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 0.0, -1.0),
                &glm::vec3(0.0, -1.0, 0.0),
            ),
        ];

        let transition = ImageLayoutTransition {
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            src_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            dst_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
        };
        output_cubemap
            .transition(&command_pool, &transition)
            .unwrap();

        let mut viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: dimension as _,
            height: dimension as _,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        };
        let scissors = [scissor];

        let unit_cube = UnitCube::new(command_pool);

        for mip_level in 0..output_cubemap.description.mip_levels {
            for (face, matrix) in matrices.iter().enumerate() {
                let current_dimension = dimension as f32 * 0.5_f32.powf(mip_level as f32);
                viewport.width = current_dimension;
                viewport.height = current_dimension;
                let viewports = [viewport];

                command_pool
                    .execute_command_once(context.graphics_queue(), |command_buffer| unsafe {
                        device.cmd_set_viewport(command_buffer, 0, &viewports);
                        device.cmd_set_scissor(command_buffer, 0, &scissors);

                        render_pass.record(command_buffer, &render_pass_begin_info, || {
                            let push_block_hdr = PushBlockHdr {
                                mvp: projection * matrix,
                            };

                            device.cmd_push_constants(
                                command_buffer,
                                render_pipeline.pipeline.layout(),
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                0,
                                byte_slice_from(&push_block_hdr),
                            );

                            device.cmd_bind_pipeline(
                                command_buffer,
                                vk::PipelineBindPoint::GRAPHICS,
                                render_pipeline.pipeline.pipeline(),
                            );

                            device.cmd_bind_descriptor_sets(
                                command_buffer,
                                vk::PipelineBindPoint::GRAPHICS,
                                render_pipeline.pipeline.layout(),
                                0,
                                &[descriptor_set],
                                &[],
                            );

                            unit_cube.draw(device, command_buffer);
                        });
                    })
                    .unwrap();

                let transition = ImageLayoutTransition {
                    old_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    dst_access_mask: vk::AccessFlags::TRANSFER_READ,
                    src_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
                    dst_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
                };
                offscreen
                    .texture
                    .transition(&command_pool, &transition, 1)
                    .unwrap();

                let src_subresource = vk::ImageSubresourceLayers::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_array_layer(0)
                    .mip_level(0)
                    .layer_count(1)
                    .build();

                let dst_subresource = vk::ImageSubresourceLayers::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_array_layer(face as _)
                    .mip_level(mip_level)
                    .layer_count(1)
                    .build();

                let extent = vk::Extent3D::builder()
                    .width(current_dimension as _)
                    .height(current_dimension as _)
                    .depth(1)
                    .build();

                let region = vk::ImageCopy::builder()
                    .src_subresource(src_subresource)
                    .dst_subresource(dst_subresource)
                    .extent(extent)
                    .build();
                let regions = [region];

                command_pool
                    .copy_image_to_image(
                        offscreen.texture.image(),
                        output_cubemap.texture.image(),
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &regions,
                    )
                    .unwrap();

                let transition = ImageLayoutTransition {
                    old_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    src_access_mask: vk::AccessFlags::TRANSFER_READ,
                    dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    src_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
                    dst_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
                };

                offscreen
                    .texture
                    .transition(&command_pool, &transition, 1)
                    .unwrap();
            }
        }

        let transition = ImageLayoutTransition {
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            dst_access_mask: vk::AccessFlags::HOST_WRITE | vk::AccessFlags::TRANSFER_WRITE,
            src_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
            dst_stage_mask: vk::PipelineStageFlags::ALL_COMMANDS,
        };

        output_cubemap
            .transition(&command_pool, &transition)
            .unwrap();

        let hdr = Self {
            cubemap: output_cubemap,
        };

        Ok(hdr)
    }

    fn create_render_pass(context: Arc<VulkanContext>, format: vk::Format) -> RenderPass {
        let color_attachment_description = vk::AttachmentDescription::builder()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();
        let attachment_descriptions = [color_attachment_description];

        let color_attachment_reference = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();
        let color_attachment_references = [color_attachment_reference];

        let subpass_description = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_references)
            .build();
        let subpass_descriptions = [subpass_description];

        let subpass_dependency_one = vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::BOTTOM_OF_PIPE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::MEMORY_READ)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .dependency_flags(vk::DependencyFlags::BY_REGION)
            .build();
        let subpass_dependency_two = vk::SubpassDependency::builder()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::BOTTOM_OF_PIPE)
            .src_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .dst_access_mask(vk::AccessFlags::MEMORY_READ)
            .dependency_flags(vk::DependencyFlags::BY_REGION)
            .build();
        let subpass_dependencies = [subpass_dependency_one, subpass_dependency_two];

        let create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachment_descriptions)
            .subpasses(&subpass_descriptions)
            .dependencies(&subpass_dependencies)
            .build();

        RenderPass::new(context, &create_info).unwrap()
    }

    fn create_descriptor_set_layout(context: Arc<VulkanContext>) -> DescriptorSetLayout {
        let binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();
        let bindings = [binding];

        let layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();

        DescriptorSetLayout::new(context, layout_create_info).unwrap()
    }

    fn create_descriptor_pool(context: Arc<VulkanContext>) -> DescriptorPool {
        let pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        };
        let pool_sizes = [pool_size];

        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(2)
            .build();

        DescriptorPool::new(context, pool_info).unwrap()
    }

    fn update_descriptor_set(
        context: Arc<VulkanContext>,
        descriptor_set: vk::DescriptorSet,
        hdr_texture_bundle: &TextureBundle,
    ) {
        let image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(hdr_texture_bundle.view.view())
            .sampler(hdr_texture_bundle.sampler.sampler())
            .build();
        let image_infos = [image_info];

        let sampler_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_infos)
            .build();

        let descriptor_writes = vec![sampler_descriptor_write];

        unsafe {
            context
                .logical_device()
                .logical_device()
                .update_descriptor_sets(&descriptor_writes, &[])
        }
    }
}
