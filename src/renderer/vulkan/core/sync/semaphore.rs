use crate::renderer::vulkan::core::VulkanContext;
use anyhow::Result;
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct Semaphore {
    semaphore: vk::Semaphore,
    context: Arc<VulkanContext>,
}

impl Semaphore {
    pub fn new(context: Arc<VulkanContext>) -> Result<Self> {
        let semaphore_info = vk::SemaphoreCreateInfo::builder().build();
        let semaphore = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_semaphore(&semaphore_info, None)
        }?;
        Ok(Self { semaphore, context })
    }

    pub fn semaphore(&self) -> vk::Semaphore {
        self.semaphore
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_semaphore(self.semaphore, None)
        }
    }
}
