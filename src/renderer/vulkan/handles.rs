use crate::renderer::vulkan::{
    core::VulkanContext,
    render::{Framebuffer, RenderPass, Swapchain, SwapchainProperties},
    resource::image::{ImageView, Texture},
};
use anyhow::Result;
use ash::vk;
use std::sync::Arc;

pub struct ForwardRenderingHandles {
    pub render_pass: Arc<RenderPass>,
    pub depth_texture: Texture,
    pub depth_texture_view: ImageView,
    pub framebuffers: Vec<Framebuffer>,
}

impl ForwardRenderingHandles {
    pub fn new(context: Arc<VulkanContext>, swapchain: &Swapchain) -> Result<Self> {
        let depth_format = context.determine_depth_format(
            vk::ImageTiling::OPTIMAL,
            vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
        );

        let render_pass = Arc::new(Self::create_render_pass(
            context.clone(),
            &swapchain.properties(),
            depth_format,
        ));

        let swapchain_extent = swapchain.properties().extent;

        let depth_texture =
            Self::create_depth_texture(context.clone(), swapchain_extent, depth_format);
        let depth_texture_view =
            Self::create_depth_texture_view(context.clone(), &depth_texture, depth_format);

        let framebuffers =
            Self::create_framebuffers(context, &swapchain, &depth_texture_view, &render_pass);

        let handles = ForwardRenderingHandles {
            render_pass,
            depth_texture,
            depth_texture_view,
            framebuffers,
        };

        Ok(handles)
    }

    fn create_render_pass(
        context: Arc<VulkanContext>,
        swapchain_properties: &SwapchainProperties,
        depth_format: vk::Format,
    ) -> RenderPass {
        let color_attachment_description = vk::AttachmentDescription::builder()
            .format(swapchain_properties.format.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();

        let depth_attachment_description = vk::AttachmentDescription::builder()
            .format(depth_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let attachment_descriptions = [color_attachment_description, depth_attachment_description];

        let color_attachment_reference = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();
        let color_attachment_references = [color_attachment_reference];

        let depth_attachment_reference = vk::AttachmentReference::builder()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let subpass_description = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_references)
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
        depth_texture_view: &ImageView,
        render_pass: &RenderPass,
    ) -> Vec<Framebuffer> {
        swapchain
            .image_views()
            .iter()
            .map(|view| [view.view(), depth_texture_view.view()])
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
            .samples(vk::SampleCountFlags::TYPE_1)
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
}
