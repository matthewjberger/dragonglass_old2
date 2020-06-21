use crate::renderer::vulkan::core::{CurrentFrameSynchronization, Instance, PhysicalDevice};
use ash::{
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};

use snafu::{ResultExt, Snafu};

type Result<T, E = LogicalDeviceError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum LogicalDeviceError {
    #[snafu(display("Failed to create logical device: {}", source))]
    LogicalDeviceCreation { source: vk::Result },
}

pub struct LogicalDevice {
    logical_device: ash::Device,
}

impl LogicalDevice {
    pub fn new(
        instance: &Instance,
        physical_device: &PhysicalDevice,
        device_create_info: vk::DeviceCreateInfo,
    ) -> Result<Self> {
        let logical_device = unsafe {
            instance
                .instance()
                .create_device(physical_device.physical_device(), &device_create_info, None)
                .context(LogicalDeviceCreation)?
        };

        Ok(LogicalDevice { logical_device })
    }

    pub fn logical_device(&self) -> &ash::Device {
        &self.logical_device
    }

    // TODO: Add error handling
    pub fn wait_for_fence(&self, current_frame_synchronization: &CurrentFrameSynchronization) {
        let in_flight_fences = [current_frame_synchronization.in_flight()];
        unsafe {
            self.logical_device
                .wait_for_fences(&in_flight_fences, true, std::u64::MAX)
                .expect("Failed to wait for fences!");
        }
    }

    pub fn reset_fence(&self, current_frame_synchronization: &CurrentFrameSynchronization) {
        let in_flight_fences = [current_frame_synchronization.in_flight()];
        unsafe {
            self.logical_device()
                .reset_fences(&in_flight_fences)
                .expect("Failed to reset fences!");
        }
    }

    pub fn wait_idle(&self) {
        unsafe {
            self.logical_device
                .device_wait_idle()
                .expect("Failed to wait for the logical device to be idle!")
        };
    }

    pub fn update_viewport(&self, command_buffer: vk::CommandBuffer, extent: vk::Extent2D) {
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as _,
            height: extent.height as _,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let viewports = [viewport];

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        };
        let scissors = [scissor];

        unsafe {
            self.logical_device
                .cmd_set_viewport(command_buffer, 0, &viewports);
            self.logical_device
                .cmd_set_scissor(command_buffer, 0, &scissors);
        }
    }

    pub fn record_command_buffer<T>(
        &self,
        command_buffer: vk::CommandBuffer,
        usage: vk::CommandBufferUsageFlags,
        mut action: T,
    ) where
        T: FnMut(),
    {
        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder().flags(usage).build();

        let device = &self.logical_device;

        unsafe {
            device
                .begin_command_buffer(command_buffer, &command_buffer_begin_info)
                .expect("Failed to begin command buffer for the render pass!")
        };

        action();

        unsafe {
            device
                .end_command_buffer(command_buffer)
                .expect("Failed to end the command buffer for a render pass!");
        }
    }
}

impl Drop for LogicalDevice {
    fn drop(&mut self) {
        unsafe {
            self.logical_device.destroy_device(None);
        }
    }
}
