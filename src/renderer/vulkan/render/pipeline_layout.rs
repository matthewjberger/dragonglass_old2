use crate::renderer::vulkan::core::VulkanContext;
use ash::{version::DeviceV1_0, vk};
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create pipeline layout: {}", source))]
    CreatePipelineLayout { source: ash::vk::Result },
}

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
        }
        .context(CreatePipelineLayout {})?;

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
