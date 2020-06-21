mod vulkan;

use crate::App;
use snafu::{ResultExt, Snafu};
use vulkan::VulkanRenderer;
use winit::window::Window;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create a vulkan renderer: {}", source))]
    CreateVulkanRenderer { source: vulkan::Error },
}

pub enum Backend {
    Vulkan,
}

pub trait Renderer {
    fn initialize(&mut self, app: &App);
    fn update(&mut self, app: &App);
    fn render(&mut self, app: &App);
}

impl dyn Renderer {
    pub fn new(backend: &Backend, window: &mut Window) -> Result<Box<dyn Renderer>> {
        match backend {
            Backend::Vulkan => {
                let vulkan_renderer = std::boxed::Box::new(
                    VulkanRenderer::new(window).context(CreateVulkanRenderer)?,
                );
                Ok(vulkan_renderer)
            }
        }
    }
}
