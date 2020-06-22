use crate::renderer::vulkan::{
    core::VulkanContext,
    pbr::environment::UnitCube,
    render::{
        DescriptorPool, DescriptorSetLayout, RenderPass, RenderPipeline,
        RenderPipelineSettingsBuilder,
    },
    resource::{image::Cubemap, Buffer, CommandPool, ShaderCache, ShaderPathSetBuilder},
};
use ash::{version::DeviceV1_0, vk};
use nalgebra_glm as glm;
use std::{mem, sync::Arc};

pub fn create_skybox_pipeline(
    context: Arc<VulkanContext>,
    shader_cache: &mut ShaderCache,
    render_pass: Arc<RenderPass>,
) -> RenderPipeline {
    let descriptions = UnitCube::vertex_input_descriptions();
    let attributes = UnitCube::vertex_attributes();
    let vertex_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&descriptions)
        .vertex_attribute_descriptions(&attributes)
        .build();

    let shader_paths = ShaderPathSetBuilder::default()
        .vertex("assets/shaders/environment/skybox.vert.spv")
        .fragment("assets/shaders/environment/skybox.frag.spv")
        .build()
        .unwrap();
    let shader_set = shader_cache
        .create_shader_set(context.clone(), &shader_paths)
        .unwrap();

    let descriptor_set_layout = SkyboxPipelineData::descriptor_set_layout(context.clone());
    let settings = RenderPipelineSettingsBuilder::default()
        .render_pass(render_pass)
        .vertex_state_info(vertex_state_info)
        .descriptor_set_layout(descriptor_set_layout)
        .shader_set(shader_set)
        .sample_shading_enabled(true)
        .rasterization_samples(context.max_usable_samples())
        .depth_test_enabled(false)
        .depth_write_enabled(false)
        .cull_mode(vk::CullModeFlags::FRONT)
        .build()
        .expect("Failed to create render pipeline settings!");

    RenderPipeline::new(context, settings)
}

#[derive(Debug, Clone, Copy)]
pub struct SkyboxUniformBufferObject {
    pub view: glm::Mat4,
    pub projection: glm::Mat4,
}

pub struct SkyboxPipelineData {
    pub descriptor_pool: DescriptorPool,
    pub descriptor_set: vk::DescriptorSet,
    pub uniform_buffer: Buffer,
    pub cube: UnitCube,
}

impl SkyboxPipelineData {
    pub fn new(context: Arc<VulkanContext>, command_pool: &CommandPool, cubemap: &Cubemap) -> Self {
        let descriptor_set_layout = Self::descriptor_set_layout(context.clone());
        let descriptor_pool = Self::create_descriptor_pool(context.clone());
        let descriptor_set = descriptor_pool
            .allocate_descriptor_sets(descriptor_set_layout.layout(), 1)
            .unwrap()[0];

        let uniform_buffer = Buffer::new_mapped_basic(
            context.clone(),
            mem::size_of::<SkyboxUniformBufferObject>() as _,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk_mem::MemoryUsage::CpuToGpu,
        )
        .unwrap();

        let cube = UnitCube::new(command_pool);

        let data = SkyboxPipelineData {
            descriptor_pool,
            uniform_buffer,
            descriptor_set,
            cube,
        };

        data.update_descriptor_set(context, &cubemap);
        data
    }

    pub fn descriptor_set_layout(context: Arc<VulkanContext>) -> DescriptorSetLayout {
        let ubo_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .build();
        let sampler_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(1)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();
        let bindings = [ubo_binding, sampler_binding];

        let layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();
        DescriptorSetLayout::new(context, layout_create_info).unwrap()
    }

    fn create_descriptor_pool(context: Arc<VulkanContext>) -> DescriptorPool {
        let ubo_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
        };

        let sampler_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        };

        let pool_sizes = [ubo_pool_size, sampler_pool_size];

        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(1)
            .build();

        DescriptorPool::new(context, pool_info).unwrap()
    }

    fn update_descriptor_set(&self, context: Arc<VulkanContext>, cubemap: &Cubemap) {
        let uniform_buffer_size = mem::size_of::<SkyboxUniformBufferObject>() as vk::DeviceSize;
        let buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(self.uniform_buffer.buffer())
            .offset(0)
            .range(uniform_buffer_size)
            .build();
        let buffer_infos = [buffer_info];

        let ubo_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(&buffer_infos)
            .build();

        let image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(cubemap.view.view())
            .sampler(cubemap.sampler.sampler())
            .build();
        let image_infos = [image_info];

        let sampler_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(1)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_infos)
            .build();

        let descriptor_writes = vec![ubo_descriptor_write, sampler_descriptor_write];

        unsafe {
            context
                .logical_device()
                .logical_device()
                .update_descriptor_sets(&descriptor_writes, &[])
        }
    }
}

pub struct SkyboxRenderer {
    command_buffer: vk::CommandBuffer,
    pipeline_layout: vk::PipelineLayout,
    descriptor_set: vk::DescriptorSet,
}

impl SkyboxRenderer {
    pub fn new(
        command_buffer: vk::CommandBuffer,
        pipeline: &RenderPipeline,
        pipeline_data: &SkyboxPipelineData,
    ) -> Self {
        Self {
            command_buffer,
            pipeline_layout: pipeline.pipeline.layout(),
            descriptor_set: pipeline_data.descriptor_set,
        }
    }

    pub fn draw(&self, device: &ash::Device, cube: &UnitCube) {
        unsafe {
            device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.descriptor_set],
                &[],
            );
        }

        cube.draw(device, self.command_buffer);
    }
}
