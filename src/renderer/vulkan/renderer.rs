use crate::renderer::vulkan::{
    core::{
        sync::synchronization_set::SynchronizationSetConstants, SynchronizationSet, VulkanContext,
    },
    render::{
        strategy::{Strategy, StrategyKind},
        Swapchain,
    },
    resource::CommandPool,
};
use crate::{app::App, renderer::Renderer};
use ash::vk;
use nalgebra_glm as glm;
use snafu::{ResultExt, Snafu};
use std::sync::Arc;
use winit::window::Window;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create a Vulkan context: {}", source))]
    CreateContext {
        source: crate::renderer::vulkan::core::context::Error,
    },

    #[snafu(display("Failed to create a synchronization set: {}", source))]
    CreateSynchronizationSet {
        source: crate::renderer::vulkan::core::sync::synchronization_set::Error,
    },

    #[snafu(display("Failed to create a command pool: {}", source))]
    CreateCommandPool {
        source: crate::renderer::vulkan::resource::command_pool::Error,
    },

    #[snafu(display("Failed to create a transient command pool: {}", source))]
    CreateTransientCommandPool {
        source: crate::renderer::vulkan::resource::command_pool::Error,
    },

    #[snafu(display("Failed to create a swapchain: {}", source))]
    CreateSwapchain {
        source: crate::renderer::vulkan::render::swapchain::Error,
    },

    #[snafu(display("Failed to recreate a swapchain: {}", source))]
    RecreateSwapchain {
        source: crate::renderer::vulkan::render::swapchain::Error,
    },

    #[snafu(display("Failed to create a rendering strategy '{:#?}': {}", kind, source))]
    CreateRenderingStrategy {
        kind: StrategyKind,
        source: crate::renderer::vulkan::render::strategy::Error,
    },
}

pub struct VulkanRenderer {
    context: Arc<VulkanContext>,
    synchronization_set: SynchronizationSet,
    command_pool: CommandPool,
    transient_command_pool: CommandPool,
    swapchain: Option<Swapchain>,
    strategy: Box<dyn Strategy>,
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

        let swapchain = Swapchain::new(context.clone(), dimensions).context(CreateSwapchain)?;

        let strategy_kind = StrategyKind::Forward;
        let strategy = Strategy::new(
            &strategy_kind,
            context.clone(),
            &transient_command_pool,
            &swapchain,
        )
        .context(CreateRenderingStrategy {
            kind: strategy_kind,
        })?;

        let renderer = Self {
            context,
            synchronization_set,
            command_pool,
            transient_command_pool,
            swapchain: Some(swapchain),
            strategy: Box::new(strategy),
            current_frame: 0,
        };

        Ok(renderer)
    }

    fn recreate_swapchain(&mut self, window_dimensions: &glm::Vec2) -> Result<()> {
        self.context.logical_device().wait_idle();

        self.swapchain = None;

        let swapchain = Swapchain::new(
            self.context.clone(),
            [window_dimensions.x as _, window_dimensions.y as _],
        )
        .context(RecreateSwapchain)?;

        self.strategy
            .recreate_swapchain(&swapchain, &mut self.command_pool);

        self.swapchain = Some(swapchain);

        Ok(())
    }

    fn swapchain(&self) -> &Swapchain {
        // FIXME: Use a result here
        self.swapchain.as_ref().expect("Failed to get swapchain!")
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        self.context.logical_device().wait_idle();
    }
}

impl Renderer for VulkanRenderer {
    fn initialize(&mut self, app: &App) {
        let extent = self.swapchain().properties().extent;
        self.strategy.initialize(&extent, &mut self.command_pool);
    }

    fn update(&mut self, app: &App) {}

    fn render(&mut self, app: &App) {
        let current_frame_synchronization = self
            .synchronization_set
            .current_frame_synchronization(self.current_frame);

        self.context
            .logical_device()
            .wait_for_fence(&current_frame_synchronization);

        let image_index_result = self.swapchain().acquire_next_image(
            current_frame_synchronization.image_available(),
            vk::Fence::null(),
        );

        let image_index = match image_index_result {
            Ok((image_index, _)) => image_index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(&app.window_dimensions)
                    .expect("Failed to recreate swapchain!");
                return;
            }
            Err(error) => panic!("Error while acquiring next image. Cause: {}", error),
        };
        let image_indices = [image_index];

        self.context
            .logical_device()
            .reset_fence(&current_frame_synchronization);

        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

        self.command_pool
            .submit_command_buffer(
                image_index as usize,
                self.context.graphics_queue(),
                &wait_stages,
                &current_frame_synchronization,
            )
            .unwrap();

        let swapchain_presentation_result = self.swapchain().present_rendered_image(
            &current_frame_synchronization,
            &image_indices,
            self.context.present_queue(),
        );

        match swapchain_presentation_result {
            Ok(is_suboptimal) if is_suboptimal => {
                self.recreate_swapchain(&app.window_dimensions)
                    .expect("Failed to recreate swapchain!");
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(&app.window_dimensions)
                    .expect("Failed to recreate swapchain!");
            }
            Err(error) => panic!("Failed to present queue. Cause: {}", error),
            _ => {}
        }

        self.current_frame +=
            (1 + self.current_frame) % SynchronizationSet::MAX_FRAMES_IN_FLIGHT as usize;
    }
}
