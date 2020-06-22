use crate::renderer::vulkan::{core::VulkanContext, render::PipelineLayout};
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct GraphicsPipeline {
    pipeline: vk::Pipeline,
    pipeline_layout: PipelineLayout,
    context: Arc<VulkanContext>,
}

impl GraphicsPipeline {
    pub fn new(
        context: Arc<VulkanContext>,
        create_info: vk::GraphicsPipelineCreateInfo,
        pipeline_layout: PipelineLayout,
    ) -> Self {
        let pipeline_create_info_arr = [create_info];
        let pipeline = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &pipeline_create_info_arr,
                    None,
                )
                .expect("Failed to create graphics pipelines!")[0]
        };

        GraphicsPipeline {
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

impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_pipeline(self.pipeline, None);
        }
    }
}
