use crate::renderer::vulkan::core::VulkanContext;
use anyhow::Result;
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct Sampler {
    sampler: vk::Sampler,
    context: Arc<VulkanContext>,
}

impl Sampler {
    pub fn new(context: Arc<VulkanContext>, create_info: vk::SamplerCreateInfo) -> Result<Self> {
        let sampler = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_sampler(&create_info, None)
        }?;

        let sampler = Self { sampler, context };

        Ok(sampler)
    }

    pub fn sampler(&self) -> vk::Sampler {
        self.sampler
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_sampler(self.sampler, None)
        };
    }
}
