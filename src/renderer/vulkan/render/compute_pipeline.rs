use crate::renderer::vulkan::{core::VulkanContext, render::PipelineLayout};
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct ComputePipeline {
    pipeline: vk::Pipeline,
    pipeline_layout: PipelineLayout,
    context: Arc<VulkanContext>,
}

impl ComputePipeline {
    pub fn new(
        context: Arc<VulkanContext>,
        create_info: vk::ComputePipelineCreateInfo,
        pipeline_layout: PipelineLayout,
    ) -> Self {
        let pipeline_create_info_arr = [create_info];
        let pipeline = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_compute_pipelines(
                    vk::PipelineCache::null(),
                    &pipeline_create_info_arr,
                    None,
                )
                .expect("Failed to create compute pipelines!")[0]
        };

        Self {
            pipeline,
            pipeline_layout,
            context,
        }
    }

    pub fn pipeline(&self) -> vk::Pipeline {
        self.pipeline
    }

    pub fn layout(&self) -> vk::PipelineLayout {
        self.pipeline_layout.layout()
    }
}

impl Drop for ComputePipeline {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_pipeline(self.pipeline, None);
        }
    }
}
