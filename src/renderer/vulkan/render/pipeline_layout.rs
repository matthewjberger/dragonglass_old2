use crate::renderer::vulkan::core::VulkanContext;
use anyhow::Result;
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct PipelineLayout {
    layout: vk::PipelineLayout,
    context: Arc<VulkanContext>,
}

impl PipelineLayout {
    pub fn new(
        context: Arc<VulkanContext>,
        create_info: vk::PipelineLayoutCreateInfo,
    ) -> Result<Self> {
        let layout = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_pipeline_layout(&create_info, None)
        }?;

        let pipeline_layout = Self { layout, context };

        Ok(pipeline_layout)
    }

    pub fn layout(&self) -> vk::PipelineLayout {
        self.layout
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_pipeline_layout(self.layout, None);
        }
    }
}
