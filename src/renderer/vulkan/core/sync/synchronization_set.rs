use crate::renderer::vulkan::core::{Fence, Semaphore, VulkanContext};
use ash::vk;
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create the image available semaphore: {}", source))]
    CreateImageAvailableSemaphore {
        source: crate::renderer::vulkan::core::sync::semaphore::Error,
    },

    #[snafu(display("Failed to create the render finished semaphore: {}", source))]
    CreateRenderFinishedSemaphore {
        source: crate::renderer::vulkan::core::sync::semaphore::Error,
    },

    #[snafu(display("Failed to create a fence: {}", source))]
    CreateInFlightFence {
        source: crate::renderer::vulkan::core::sync::fence::Error,
    },
}

pub trait SynchronizationSetConstants {
    // The maximum number of frames that can be rendered simultaneously
    const MAX_FRAMES_IN_FLIGHT: u32;
}

impl SynchronizationSetConstants for SynchronizationSet {
    const MAX_FRAMES_IN_FLIGHT: u32 = 2;
}

pub struct SynchronizationSet {
    image_available_semaphores: Vec<Semaphore>,
    render_finished_semaphores: Vec<Semaphore>,
    in_flight_fences: Vec<Fence>,
}

impl SynchronizationSet {
    pub fn new(context: Arc<VulkanContext>) -> Result<Self> {
        let mut image_available_semaphores = Vec::new();
        let mut render_finished_semaphores = Vec::new();
        let mut in_flight_fences = Vec::new();
        for _ in 0..SynchronizationSet::MAX_FRAMES_IN_FLIGHT {
            let image_available_semaphore =
                Semaphore::new(context.clone()).context(CreateImageAvailableSemaphore)?;
            image_available_semaphores.push(image_available_semaphore);

            let render_finished_semaphore =
                Semaphore::new(context.clone()).context(CreateRenderFinishedSemaphore)?;
            render_finished_semaphores.push(render_finished_semaphore);

            let in_flight_fence = Fence::new(context.clone(), vk::FenceCreateFlags::SIGNALED)
                .context(CreateInFlightFence)?;
            in_flight_fences.push(in_flight_fence);
        }

        Ok(SynchronizationSet {
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
        })
    }

    pub fn current_frame_synchronization(
        &self,
        current_frame: usize,
    ) -> CurrentFrameSynchronization {
        CurrentFrameSynchronization::new(&self, current_frame)
    }
}

pub struct CurrentFrameSynchronization {
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    in_flight: vk::Fence,
}

impl CurrentFrameSynchronization {
    pub fn new(synchronization_set: &SynchronizationSet, current_frame: usize) -> Self {
        // TODO: Add error checking for vecs being empty
        let image_available =
            synchronization_set.image_available_semaphores[current_frame].semaphore();
        let render_finished =
            synchronization_set.render_finished_semaphores[current_frame].semaphore();
        let in_flight = synchronization_set.in_flight_fences[current_frame].fence();
        Self {
            image_available,
            render_finished,
            in_flight,
        }
    }

    pub fn image_available(&self) -> vk::Semaphore {
        self.image_available
    }

    pub fn render_finished(&self) -> vk::Semaphore {
        self.render_finished
    }

    pub fn in_flight(&self) -> vk::Fence {
        self.in_flight
    }
}
