use crate::renderer::vulkan::core::VulkanContext;
use ash::{version::DeviceV1_0, vk};
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create sampler: {}", source))]
    CreateSampler { source: ash::vk::Result },
}

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
        }
        .context(CreateSampler {})?;

        let sampler = Sampler { sampler, context };

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
