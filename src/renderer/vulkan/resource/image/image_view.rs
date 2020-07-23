use crate::renderer::vulkan::core::VulkanContext;
use anyhow::Result;
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct ImageView {
    view: vk::ImageView,
    context: Arc<VulkanContext>,
}

impl ImageView {
    pub fn new(context: Arc<VulkanContext>, create_info: vk::ImageViewCreateInfo) -> Result<Self> {
        let view = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_image_view(&create_info, None)
        }?;

        let image_view = ImageView { view, context };

        Ok(image_view)
    }

    pub fn view(&self) -> vk::ImageView {
        self.view
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_image_view(self.view, None);
        }
    }
}
