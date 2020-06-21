use crate::renderer::vulkan::core::VulkanContext;
use ash::{version::DeviceV1_0, vk};
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create framebuffer: {}", source))]
    CreateFrameBuffer { source: ash::vk::Result },
}

pub struct Framebuffer {
    framebuffer: vk::Framebuffer,
    context: Arc<VulkanContext>,
}

impl Framebuffer {
    pub fn new(
        context: Arc<VulkanContext>,
        create_info: vk::FramebufferCreateInfo,
    ) -> Result<Self> {
        let framebuffer = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_framebuffer(&create_info, None)
        }
        .context(CreateFrameBuffer {})?;

        let framebuffer = Framebuffer {
            framebuffer,
            context,
        };

        Ok(framebuffer)
    }

    pub fn framebuffer(&self) -> vk::Framebuffer {
        self.framebuffer
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_framebuffer(self.framebuffer, None);
        }
    }
}
