mod vulkan;

use crate::renderer::vulkan::VulkanRenderer;
use anyhow::Result;
use imgui::{Context, DrawData};
use legion::prelude::*;
use nalgebra::{Matrix4, Quaternion, UnitQuaternion};
use nalgebra_glm as glm;
use winit::window::Window;

#[derive(Debug)]
pub enum Backend {
    Vulkan,
}

// FIXME: Make the renderer trait take something more specific than the world and resources
pub trait Renderer {
    fn initialize(&mut self, world: &World, imgui: &mut Context);
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
    pub translation: glm::Vec3,
    pub rotation: glm::Quat,
    pub scale: glm::Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: glm::vec3(0.0, 0.0, 0.0),
            rotation: glm::Quat::identity(),
            scale: glm::vec3(1.0, 1.0, 1.0),
        }
    }
}

impl Transform {
    pub fn new(translation: glm::Vec3, rotation: glm::Quat, scale: glm::Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn matrix(&self) -> glm::Mat4 {
        Matrix4::new_translation(&self.translation)
            * Matrix4::from(UnitQuaternion::from_quaternion(self.rotation))
            * Matrix4::new_nonuniform_scaling(&self.scale)
    }
}
