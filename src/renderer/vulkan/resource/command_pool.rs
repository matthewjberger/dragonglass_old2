use crate::renderer::vulkan::{
    core::{CurrentFrameSynchronization, Fence, VulkanContext},
    resource::Buffer,
};
use anyhow::Result;
use ash::{version::DeviceV1_0, vk};
use std::sync::Arc;

pub struct CommandPool {
    pool: vk::CommandPool,
    context: Arc<VulkanContext>,
    command_buffers: Vec<vk::CommandBuffer>,
}

impl CommandPool {
    pub fn new(context: Arc<VulkanContext>, flags: vk::CommandPoolCreateFlags) -> Result<Self> {
        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(context.graphics_queue_family_index())
            .flags(flags)
            .build();

        let pool = unsafe {
            context
                .logical_device()
                .logical_device()
                .create_command_pool(&command_pool_info, None)?
        };

        let command_pool = CommandPool {
            pool,
            context,
            command_buffers: Vec::new(),
        };

        Ok(command_pool)
    }

    pub fn pool(&self) -> vk::CommandPool {
        self.pool
    }

    pub fn command_buffers(&self) -> &[vk::CommandBuffer] {
        &self.command_buffers
    }

    pub fn allocate_command_buffers(&mut self, size: vk::DeviceSize) -> Result<()> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(size as _)
            .build();

        self.command_buffers = unsafe {
            self.context
                .logical_device()
                .logical_device()
                .allocate_command_buffers(&allocate_info)?
        };

        Ok(())
    }

    pub fn clear_command_buffers(&mut self) {
        if !self.command_buffers.is_empty() {
            unsafe {
                self.context
                    .logical_device()
                    .logical_device()
                    .free_command_buffers(self.pool, &self.command_buffers);
            }
        }
        self.command_buffers.clear();
    }

    pub fn create_staging_buffer<T: Copy>(&self, data: &[T]) -> Buffer {
        let buffer_size = (data.len() * std::mem::size_of::<T>()) as ash::vk::DeviceSize;

        let staging_buffer = Buffer::new_mapped_basic(
            self.context.clone(),
            buffer_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk_mem::MemoryUsage::CpuToGpu,
        )
        .unwrap();
        staging_buffer.upload_to_buffer(&data, 0).unwrap();
        staging_buffer
    }

    pub fn create_device_local_buffer<T: Copy>(
        &self,
        usage_flags: vk::BufferUsageFlags,
        data: &[T],
        regions: &[vk::BufferCopy],
    ) -> Buffer {
        let staging_buffer = self.create_staging_buffer(&data);

        let device_local_buffer = Buffer::new_mapped_basic(
            self.context.clone(),
            staging_buffer.allocation_info().get_size() as _,
            vk::BufferUsageFlags::TRANSFER_DST | usage_flags,
            vk_mem::MemoryUsage::GpuOnly,
        )
        .unwrap();

        self.copy_buffer_to_buffer(
            staging_buffer.buffer(),
            device_local_buffer.buffer(),
            &regions,
        )
        .unwrap();

        device_local_buffer
    }

    // TODO: refactor this to use less parameters
    pub fn submit_command_buffer(
        &self,
        index: usize,
        queue: vk::Queue,
        wait_stages: &[vk::PipelineStageFlags],
        current_frame_synchronization: &CurrentFrameSynchronization,
    ) -> Result<()> {
        let image_available_semaphores = [current_frame_synchronization.image_available()];
        let render_finished_semaphores = [current_frame_synchronization.render_finished()];
        // TODO: Add error handling, index may be invalid
        let command_buffers_to_use = [self.command_buffers()[index]];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&image_available_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers_to_use)
            .signal_semaphores(&render_finished_semaphores)
            .build();
        let submit_info_arr = [submit_info];
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .queue_submit(
                    queue,
                    &submit_info_arr,
                    current_frame_synchronization.in_flight(),
                )?
        }
        Ok(())
    }

    pub fn copy_image_to_image(
        &self,
        source: vk::Image,
        destination: vk::Image,
        source_layout: vk::ImageLayout,
        destination_layout: vk::ImageLayout,
        regions: &[vk::ImageCopy],
    ) -> Result<()> {
        self.execute_command_once(self.context.graphics_queue(), |command_buffer| {
            unsafe {
                self.context
                    .logical_device()
                    .logical_device()
                    .cmd_copy_image(
                        command_buffer,
                        source,
                        source_layout,
                        destination,
                        destination_layout,
                        &regions,
                    )
            };
        })
    }

    pub fn copy_buffer_to_buffer(
        &self,
        source: vk::Buffer,
        destination: vk::Buffer,
        regions: &[vk::BufferCopy],
    ) -> Result<()> {
        self.execute_command_once(self.context.graphics_queue(), |command_buffer| {
            unsafe {
                self.context
                    .logical_device()
                    .logical_device()
                    .cmd_copy_buffer(command_buffer, source, destination, &regions)
            };
        })
    }

    pub fn copy_buffer_to_image(
        &self,
        buffer: vk::Buffer,
        image: vk::Image,
        regions: &[vk::BufferImageCopy],
    ) -> Result<()> {
        self.execute_command_once(self.context.graphics_queue(), |command_buffer| unsafe {
            self.context
                .logical_device()
                .logical_device()
                .cmd_copy_buffer_to_image(
                    command_buffer,
                    buffer,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    regions,
                )
        })
    }

    // TODO: Refactor this to be smaller. Functionality can probably be reused
    // in generic command buffer submission method
    pub fn execute_command_once<T>(&self, queue: vk::Queue, mut executor: T) -> Result<()>
    where
        T: FnMut(vk::CommandBuffer),
    {
        // Allocate a command buffer using the command pool
        let command_buffer = {
            let allocation_info = vk::CommandBufferAllocateInfo::builder()
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_pool(self.pool)
                .command_buffer_count(1)
                .build();

            unsafe {
                self.context
                    .logical_device()
                    .logical_device()
                    .allocate_command_buffers(&allocation_info)
            }
        }?[0];
        let command_buffers = [command_buffer];

        self.context.logical_device().record_command_buffer(
            command_buffer,
            vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
            || {
                executor(command_buffer);
            },
        );

        // Build the submission info
        let submit_info = vk::SubmitInfo::builder()
            .command_buffers(&command_buffers)
            .build();
        let submit_info_arr = [submit_info];

        // Create a fence to ensure that the command buffer has finished executing
        let fence = Fence::new(self.context.clone(), vk::FenceCreateFlags::empty())?;

        let logical_device = self.context.logical_device().logical_device();

        unsafe {
            // Submit the command buffer
            logical_device.queue_submit(queue, &submit_info_arr, fence.fence())?;

            logical_device.wait_for_fences(&[fence.fence()], true, 100_000_000_000)?;

            // Wait for the command buffer to be executed
            logical_device.queue_wait_idle(queue)?;

            // Free the command buffer
            logical_device.free_command_buffers(self.pool(), &command_buffers);
        };

        Ok(())
    }

    pub fn transition_image_layout(
        &self,
        barriers: &[vk::ImageMemoryBarrier],
        src_stage_mask: vk::PipelineStageFlags,
        dst_stage_mask: vk::PipelineStageFlags,
    ) -> Result<()> {
        self.execute_command_once(self.context.graphics_queue(), |command_buffer| {
            unsafe {
                self.context
                    .logical_device()
                    .logical_device()
                    .cmd_pipeline_barrier(
                        command_buffer,
                        src_stage_mask,
                        dst_stage_mask,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &barriers,
                    )
            };
        })?;

        Ok(())
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        self.clear_command_buffers();
        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .destroy_command_pool(self.pool, None);
        }
    }
}
