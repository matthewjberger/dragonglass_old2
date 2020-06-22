use crate::renderer::vulkan::core::VulkanContext;
use ash::{version::DeviceV1_0, vk};
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create descriptor pool: {}", source))]
    CreateDescriptorPool { source: ash::vk::Result },

    #[snafu(display("Failed to allocate descriptor sets: {}", source))]
    AllocateDescriptorSets { source: ash::vk::Result },
}

pub struct DescriptorPool {
    pool: vk::DescriptorPool,
    context: Arc<VulkanContext>,
}

impl DescriptorPool {
    pub fn new(
        context: Arc<VulkanContext>,
        pool_info: vk::DescriptorPoolCreateInfo,
    ) -> Result<Self> {
        let pool = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_descriptor_pool(&pool_info, None)
        }
        .context(CreateDescriptorPool {})?;

        let descriptor_pool = Self { pool, context };

        Ok(descriptor_pool)
    }

    pub fn allocate_descriptor_sets(
        &self,
        layout: vk::DescriptorSetLayout,
        number_of_sets: u32,
    ) -> Result<Vec<vk::DescriptorSet>> {
        let layouts = (0..number_of_sets).map(|_| layout).collect::<Vec<_>>();
        let allocation_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.pool)
            .set_layouts(&layouts)
            .build();
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .allocate_descriptor_sets(&allocation_info)
        }
        .context(AllocateDescriptorSets {})
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_descriptor_pool(self.pool, None);
        }
    }
}
