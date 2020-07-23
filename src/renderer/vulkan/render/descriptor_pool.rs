use crate::renderer::vulkan::core::VulkanContext;
use anyhow::Result;
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

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
        }?;

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
        let descriptor_sets = unsafe {
            self.context
                .logical_device()
                .logical_device()
                .allocate_descriptor_sets(&allocation_info)?
        };
        Ok(descriptor_sets)
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
