pub use self::forward::*;

pub mod forward;

use crate::renderer::vulkan::{
    core::VulkanContext,
    pbr::PbrScene,
    render::{RenderPass, Swapchain},
    resource::CommandPool,
};
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
    fn initialize(
        &mut self,
        extent: &vk::Extent2D,
        command_pool: &mut CommandPool,
        scene: &mut PbrScene,
    );

    fn recreate_swapchain(
        &mut self,
        swapchain: &Swapchain,
        command_pool: &mut CommandPool,
        scene: &mut PbrScene,
    );

    fn render_pass(&mut self) -> Arc<RenderPass>;
}

impl dyn Strategy {
    pub fn create_strategy(
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
