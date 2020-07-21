mod vulkan;

use crate::renderer::vulkan::VulkanRenderer;
use crate::App;
use imgui::{Context, DrawData};
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
    fn initialize(&mut self, app: &App, imgui: &mut Context);
    fn update(&mut self, app: &App);
    fn render(&mut self, app: &App, draw_data: &DrawData);
}

impl dyn Renderer {
    pub fn create_backend(backend: &Backend, window: &mut Window) -> Result<impl Renderer> {
        match backend {
            Backend::Vulkan => VulkanRenderer::new(window).context(CreateVulkanRenderer),
        }
    }
}

/// # Safety
///
/// This method will convert any slice to a byte slice.
/// Use with slices of number primitives.
pub unsafe fn byte_slice_from<T: Sized>(data: &T) -> &[u8] {
    let data_ptr = (data as *const T) as *const u8;
    std::slice::from_raw_parts(data_ptr, std::mem::size_of::<T>())
}
