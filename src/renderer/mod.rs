mod vulkan;

use crate::renderer::vulkan::VulkanRenderer;
use anyhow::Result;
use imgui::{Context, DrawData};
use legion::prelude::*;
use nalgebra_glm as glm;
use winit::window::Window;

#[derive(Debug)]
pub enum Backend {
    Vulkan,
}

// FIXME: Make the renderer trait take something more specific than the world and resources
pub trait Renderer {
    fn initialize(&mut self, world: &World, resources: &Resources, imgui: &mut Context);
    fn render(&mut self, world: &World, resources: &Resources, draw_data: &DrawData);
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

#[derive(Debug)]
pub struct AssetName(pub String);

#[derive(Debug)]
pub struct Transform {
    pub translate: glm::Mat4,
    pub rotate: glm::Mat4,
    pub scale: glm::Mat4,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translate: glm::Mat4::identity(),
            rotate: glm::Mat4::identity(),
            scale: glm::Mat4::identity(),
        }
    }
}
