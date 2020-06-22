mod vulkan;

use crate::renderer::vulkan::VulkanRenderer;
use crate::App;
use snafu::{ResultExt, Snafu};
use winit::window::Window;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create a vulkan renderer: {}", source))]
    CreateVulkanRenderer {
        source: crate::renderer::vulkan::Error,
    },
}

#[derive(Debug)]
pub enum Backend {
    Vulkan,
}

pub trait Renderer {
    fn initialize(&mut self, app: &App);
    fn update(&mut self, app: &App);
    fn render(&mut self, app: &App);
}

impl dyn Renderer {
    pub fn create_backend(backend: &Backend, window: &mut Window) -> Result<impl Renderer> {
        match backend {
            Backend::Vulkan => VulkanRenderer::new(window).context(CreateVulkanRenderer),
        }
    }
}
