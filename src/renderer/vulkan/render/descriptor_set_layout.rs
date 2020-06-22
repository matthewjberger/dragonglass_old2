use crate::renderer::vulkan::core::VulkanContext;
use ash::{version::DeviceV1_0, vk};
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create descriptor set layout: {}", source))]
    CreateDescriptorSetLayout { source: ash::vk::Result },
}

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
        }
        .context(CreateDescriptorSetLayout {})?;

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
