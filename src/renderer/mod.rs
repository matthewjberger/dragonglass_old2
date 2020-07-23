mod vulkan;

use crate::renderer::vulkan::VulkanRenderer;
use crate::App;
use anyhow::Result;
use imgui::{Context, DrawData};
use nalgebra_glm as glm;
use winit::window::Window;

#[derive(Debug)]
pub enum Backend {
    Vulkan,
}

pub trait Renderer {
    fn initialize(&mut self, imgui: &mut Context);
    fn update(&mut self, app: &App);
    fn render(&mut self, window_dimensions: &glm::Vec2, draw_data: &DrawData);
}

impl dyn Renderer {
    pub fn create_backend(backend: &Backend, window: &mut Window) -> Result<impl Renderer> {
        match backend {
            Backend::Vulkan => VulkanRenderer::new(window),
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
