use crate::renderer::vulkan::core::VulkanContext;
use ash::{version::DeviceV1_0, vk};
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create render pass: {}", source))]
    CreateRenderPass { source: ash::vk::Result },
}

pub struct RenderPass {
    render_pass: vk::RenderPass,
    context: Arc<VulkanContext>,
}

impl RenderPass {
    pub fn new(
        context: Arc<VulkanContext>,
        create_info: &vk::RenderPassCreateInfo,
    ) -> Result<Self> {
        let render_pass = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_render_pass(&create_info, None)
        }
        .context(CreateRenderPass {})?;

        let render_pass = Self {
            render_pass,
            context,
        };

        Ok(render_pass)
    }

    pub fn render_pass(&self) -> vk::RenderPass {
        self.render_pass
    }

    pub fn record<T>(
        context: Arc<VulkanContext>,
        command_buffer: vk::CommandBuffer,
        render_pass_begin_info: &vk::RenderPassBeginInfo,
        mut action: T,
    ) where
        T: FnMut(),
    {
        let device = context.logical_device().logical_device();
        unsafe {
            device.cmd_begin_render_pass(
                command_buffer,
                render_pass_begin_info,
                vk::SubpassContents::INLINE,
            )
        };

        action();

        unsafe {
            device.cmd_end_render_pass(command_buffer);
        }
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_render_pass(self.render_pass, None);
        }
    }
}
