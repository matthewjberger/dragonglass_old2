use crate::renderer::vulkan::core::VulkanContext;
use ash::{version::DeviceV1_0, vk};
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create image view: {}", source))]
    CreateImageView { source: ash::vk::Result },
}

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
        }
        .context(CreateImageView {})?;

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
