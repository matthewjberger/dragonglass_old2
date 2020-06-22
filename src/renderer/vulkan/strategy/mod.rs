pub use self::forward::*;

pub mod forward;

use crate::renderer::vulkan::{core::VulkanContext, render::Swapchain, resource::CommandPool};
use ash::vk;
use snafu::{ResultExt, Snafu};
use std::sync::Arc;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create a forward renderer: {}", source))]
    CreateForwardRenderingStrategy {
        source: crate::renderer::vulkan::strategy::forward::Error,
    },
}

#[derive(Debug)]
pub enum StrategyKind {
    Forward,
}

pub trait Strategy {
    fn initialize(&mut self, extent: &vk::Extent2D, command_pool: &mut CommandPool);
    fn recreate_swapchain(&mut self, swapchain: &Swapchain, command_pool: &mut CommandPool);
}

impl dyn Strategy {
    pub fn new(
        kind: &StrategyKind,
        context: Arc<VulkanContext>,
        command_pool: &CommandPool,
        swapchain: &Swapchain,
    ) -> Result<impl Strategy> {
        match kind {
            StrategyKind::Forward => {
                ForwardRenderingStrategy::new(context, &command_pool, &swapchain)
                    .context(CreateForwardRenderingStrategy)
            }
        }
    }
}
