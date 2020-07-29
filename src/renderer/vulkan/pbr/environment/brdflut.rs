use crate::renderer::vulkan::{
    core::VulkanContext,
    render::{
        DescriptorSetLayout, Framebuffer, RenderPass, RenderPipeline, RenderPipelineSettingsBuilder,
    },
    resource::{
        image::{ImageView, Sampler, Texture},
        CommandPool, ShaderCache, ShaderPathSetBuilder,
    },
};
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct Brdflut {
    pub texture: Texture,
    pub view: ImageView,
    pub sampler: Sampler,
}

impl Brdflut {
    pub fn new(
        context: Arc<VulkanContext>,
        command_pool: &CommandPool,
        shader_cache: &mut ShaderCache,
    ) -> Self {
        let dimension = 512;
        let format = vk::Format::R16G16_SFLOAT;
        let texture = Self::create_texture(context.clone(), dimension, format);
        let view = Self::create_image_view(context.clone(), &texture, format);
        let sampler = Self::create_sampler(context.clone());
        let render_pass = Arc::new(Self::create_render_pass(context.clone(), format));

        let attachments = [view.view()];
        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass.render_pass())
            .attachments(&attachments)
            .width(dimension)
            .height(dimension)
            .layers(1)
            .build();
        let framebuffer = Framebuffer::new(context.clone(), create_info).unwrap();

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
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

        let pipeline = Self::create_pipeline(context.clone(), shader_cache, render_pass.clone());

        command_pool
            .execute_command_once(context.graphics_queue(), |command_buffer| unsafe {
                let viewport = vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: dimension as _,
                    height: dimension as _,
                    min_depth: 0.0,
                    max_depth: 1.0,
                };
                let viewports = [viewport];

                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent,
                };
                let scissors = [scissor];

                device.cmd_set_viewport(command_buffer, 0, &viewports);
                device.cmd_set_scissor(command_buffer, 0, &scissors);

                RenderPass::record(
                    context.clone(),
                    command_buffer,
                    &render_pass_begin_info,
                    || {
                        device.cmd_bind_pipeline(
                            command_buffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline.pipeline.pipeline(),
                        );
                        device.cmd_draw(command_buffer, 3, 1, 0, 0);
                    },
                );
            })
            .unwrap();

        Self {
            texture,
            view,
            sampler,
        }
    }

    fn create_texture(context: Arc<VulkanContext>, dimension: u32, format: vk::Format) -> Texture {
        let image_create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: dimension,
                height: dimension,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .flags(vk::ImageCreateFlags::empty())
            .build();

        let allocation_create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::GpuOnly,
            ..Default::default()
        };

        Texture::new(context, &allocation_create_info, &image_create_info).unwrap()
    }

    fn create_image_view(
        context: Arc<VulkanContext>,
        texture: &Texture,
        format: vk::Format,
    ) -> ImageView {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(texture.image())
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
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

    fn create_sampler(context: Arc<VulkanContext>) -> Sampler {
        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .anisotropy_enable(true)
            .max_anisotropy(1.0)
            .border_color(vk::BorderColor::INT_OPAQUE_WHITE)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(1.0)
            .build();
        Sampler::new(context, sampler_info).unwrap()
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
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
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

    fn create_pipeline(
        context: Arc<VulkanContext>,
        shader_cache: &mut ShaderCache,
        render_pass: Arc<RenderPass>,
    ) -> RenderPipeline {
        let shader_paths = ShaderPathSetBuilder::default()
            .vertex("assets/shaders/environment/fullscreen_triangle.vert.spv")
            .fragment("assets/shaders/environment/genbrdflut.frag.spv")
            .build()
            .unwrap();
        let shader_set = shader_cache
            .create_shader_set(context.clone(), &shader_paths)
            .unwrap();

        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&[])
            .build();
        let descriptor_set_layout =
            DescriptorSetLayout::new(context.clone(), descriptor_set_layout_create_info).unwrap();
        let descriptor_set_layout = Arc::new(descriptor_set_layout);

        let settings = RenderPipelineSettingsBuilder::default()
            .render_pass(render_pass.clone())
            .vertex_state_info(vk::PipelineVertexInputStateCreateInfo::builder().build())
            .descriptor_set_layout(descriptor_set_layout)
            .shader_set(shader_set)
            .build()
            .expect("Failed to create render pipeline settings");

        RenderPipeline::new(context, settings)
    }
}
