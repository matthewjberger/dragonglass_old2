use crate::renderer::vulkan::core::VulkanContext;
use anyhow::Result;
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct DescriptorSetLayout {
    layout: vk::DescriptorSetLayout,
    context: Arc<VulkanContext>,
}

impl DescriptorSetLayout {
    pub fn new(
        context: Arc<VulkanContext>,
        create_info: vk::DescriptorSetLayoutCreateInfo,
    ) -> Result<Self> {
        let layout = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_descriptor_set_layout(&create_info, None)
        }?;

        let descriptor_set_layout = DescriptorSetLayout { layout, context };

        Ok(descriptor_set_layout)
    }

    pub fn layout(&self) -> vk::DescriptorSetLayout {
        self.layout
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_descriptor_set_layout(self.layout, None);
        }
    }
}
