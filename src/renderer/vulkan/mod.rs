mod core;
mod render;
mod resource;

use self::{
    core::{SynchronizationSet, VulkanContext},
    resource::CommandPool,
};
use crate::{app::App, renderer::Renderer};
use ash::vk;
use snafu::{ResultExt, Snafu};
use std::sync::Arc;
use winit::window::Window;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create a Vulkan context: {}", source))]
    CreateContext { source: self::core::context::Error },

    #[snafu(display("Failed to create a synchronization set: {}", source))]
    CreateSynchronizationSet {
        source: self::core::sync::synchronization_set::Error,
    },

    #[snafu(display("Failed to create a command pool: {}", source))]
    CreateCommandPool {
        source: self::resource::command_pool::Error,
    },

    #[snafu(display("Failed to create a transient command pool: {}", source))]
    CreateTransientCommandPool {
        source: self::resource::command_pool::Error,
    },
}

pub(crate) struct VulkanRenderer {
    context: Arc<VulkanContext>,
    synchronization_set: SynchronizationSet,
    command_pool: CommandPool,
    transient_command_pool: CommandPool,
    current_frame: usize,
}

impl VulkanRenderer {
    pub fn new(window: &mut Window) -> Result<Self> {
        let context = Arc::new(VulkanContext::new(&window).context(CreateContext)?);

        let synchronization_set =
            SynchronizationSet::new(context.clone()).context(CreateSynchronizationSet)?;

        let command_pool = CommandPool::new(
            context.clone(),
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        )
        .context(CreateCommandPool)?;

        let transient_command_pool =
            CommandPool::new(context.clone(), vk::CommandPoolCreateFlags::TRANSIENT)
                .context(CreateTransientCommandPool)?;

        let logical_size = window.inner_size();
        let dimensions = [logical_size.width as u32, logical_size.height as u32];

        let renderer = Self {
            context,
            synchronization_set,
            command_pool,
            transient_command_pool,
            current_frame: 0,
        };

        Ok(renderer)
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        self.context.logical_device().wait_idle();
    }
}

impl Renderer for VulkanRenderer {
    fn initialize(&mut self, app: &App) {}
    fn update(&mut self, app: &App) {}
    fn render(&mut self, app: &App) {}
}
