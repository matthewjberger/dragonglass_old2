use crate::renderer::vulkan::{
    core::VulkanContext,
    handles::offscreen::Offscreen,
    render::{
        DescriptorPool, DescriptorSetLayout, Framebuffer, RenderPass, RenderPipeline,
        RenderPipelineSettingsBuilder, Swapchain,
    },
    resource::{ShaderCache, ShaderPathSetBuilder},
};
use anyhow::Result;
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

// TODO: Rename to something related to post-processing
pub struct ForwardRenderingHandles {
    pub offscreen: Offscreen,
    pub render_pass: Arc<RenderPass>,
    pub framebuffers: Vec<Framebuffer>,
    pub pipeline: Option<RenderPipeline>, // TODO: Move some of the data to a separate struct
    pub descriptor_set_layout: Arc<DescriptorSetLayout>,
    pub descriptor_set: vk::DescriptorSet,
    pub descriptor_pool: DescriptorPool,
    context: Arc<VulkanContext>,
}

impl ForwardRenderingHandles {
    pub fn new(context: Arc<VulkanContext>, swapchain: &Swapchain) -> Result<Self> {
        let format = swapchain.properties().format.format;

        let render_pass = Arc::new(Self::create_render_pass(context.clone(), format));

        let framebuffers = swapchain.create_framebuffers(context.clone(), render_pass.clone());

        let offscreen = Offscreen::new(context.clone())?;

        let descriptor_set_layout = Arc::new(Self::descriptor_set_layout(context.clone()));
        let descriptor_pool = Self::create_descriptor_pool(context.clone());
        let descriptor_set = descriptor_pool
            .allocate_descriptor_sets(descriptor_set_layout.layout(), 1)
            .unwrap()[0];

        let handles = Self {
            render_pass,
            offscreen,
            context,
            framebuffers,
            pipeline: None,
            descriptor_set_layout,
            descriptor_set,
            descriptor_pool,
        };

        handles.update_descriptor_set();

        Ok(handles)
    }

    fn create_render_pass(context: Arc<VulkanContext>, format: vk::Format) -> RenderPass {
        let color_attachment_description = vk::AttachmentDescription::builder()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
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

    pub fn recreate_pipeline(&mut self, shader_cache: &mut ShaderCache) {
        let shader_paths = ShaderPathSetBuilder::default()
            .vertex("assets/shaders/environment/fullscreen_triangle.vert.spv")
            .fragment("assets/shaders/environment/post_process.frag.spv")
            .build()
            .unwrap();
        let shader_set = shader_cache
            .create_shader_set(self.context.clone(), &shader_paths)
            .unwrap();

        let settings = RenderPipelineSettingsBuilder::default()
            .render_pass(self.render_pass.clone())
            .vertex_state_info(vk::PipelineVertexInputStateCreateInfo::builder().build())
            .descriptor_set_layout(self.descriptor_set_layout.clone())
            .shader_set(shader_set)
            .build()
            .expect("Failed to create render pipeline settings");

        self.pipeline = None;
        self.pipeline = Some(RenderPipeline::new(self.context.clone(), settings));
    }

    fn descriptor_set_layout(context: Arc<VulkanContext>) -> DescriptorSetLayout {
        let sampler_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();
        let bindings = [sampler_binding];
        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();
        let descriptor_set_layout =
            DescriptorSetLayout::new(context.clone(), descriptor_set_layout_create_info).unwrap();
        descriptor_set_layout
    }

    fn create_descriptor_pool(context: Arc<VulkanContext>) -> DescriptorPool {
        let sampler_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 6,
        };

        let pool_sizes = [sampler_pool_size];

        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(1)
            .build();

        DescriptorPool::new(context, pool_info).unwrap()
    }

    fn update_descriptor_set(&self) {
        let image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(self.offscreen.color_texture.view.view())
            .sampler(self.offscreen.color_texture.sampler.sampler())
            .build();
        let image_infos = [image_info];

        let sampler_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_infos)
            .build();

        let descriptor_writes = [sampler_descriptor_write];

        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .update_descriptor_sets(&descriptor_writes, &[])
        }
    }

    pub fn issue_commands(&self, command_buffer: vk::CommandBuffer) {
        let device = self.context.logical_device().logical_device();

        if let Some(pipeline) = self.pipeline.as_ref() {
            pipeline.bind(device, command_buffer);

            unsafe {
                device.cmd_bind_descriptor_sets(
                    command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline.pipeline.layout(),
                    0,
                    &[self.descriptor_set],
                    &[],
                );

                device.cmd_draw(command_buffer, 3, 1, 0, 0);
            }
        }
    }
}
