use crate::renderer::{
    byte_slice_from,
    vulkan::{
        asset::{GltfAsset, Primitive},
        core::VulkanContext,
        pbr::environment::{
            create_skybox_pipeline, Brdflut, HdrCubemap, IrradianceMap, PrefilterMap,
            SkyboxPipelineData, SkyboxRenderer, SkyboxUniformBufferObject,
        },
        render::{
            DescriptorPool, DescriptorSetLayout, GraphicsPipeline, RenderPass, RenderPipeline,
            RenderPipelineSettingsBuilder,
        },
        resource::{
            image::{DummyImage, TextureBundle},
            Buffer, CommandPool, GeometryBuffer, ShaderCache, ShaderPathSetBuilder,
        },
    },
};
use ash::{version::DeviceV1_0, vk};
use gltf::material::AlphaMode;
use log::debug;
use nalgebra_glm as glm;
use snafu::Snafu;
use std::{mem, sync::Arc};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to create render pipeline: {}", source))]
    CreateRenderPipeline {
        source: crate::renderer::vulkan::resource::shader::Error,
    },

    #[snafu(display("Failed to create shader: {}", source))]
    CreateShader {
        source: crate::renderer::vulkan::resource::shader::Error,
    },

    #[snafu(display("Failed to create shader set: {}", source))]
    CreateShaderSet {
        source: crate::renderer::vulkan::resource::shader::Error,
    },
}

pub struct PushConstantBlockMaterial {
    pub base_color_factor: glm::Vec4,
    pub emissive_factor: glm::Vec3,
    pub color_texture_set: i32,
    pub metallic_roughness_texture_set: i32, // B channel - metalness values. G channel - roughness values
    pub normal_texture_set: i32,
    pub occlusion_texture_set: i32, // R channel - occlusion values
    pub emissive_texture_set: i32,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub alpha_mode: i32,
    pub alpha_cutoff: f32,
}

#[derive(Clone, Copy)]
pub struct UniformBufferObject {
    pub view: glm::Mat4,
    pub projection: glm::Mat4,
    pub camera_position: glm::Vec4,
    pub joint_matrices: [glm::Mat4; UniformBufferObject::MAX_NUM_JOINTS],
}

impl UniformBufferObject {
    // This needs to match the defined value in the shaders
    pub const MAX_NUM_JOINTS: usize = 128;
}

#[derive(Debug, Clone, Copy)]
pub struct DynamicUniformBufferObject {
    pub model: glm::Mat4,
    // X value is the joint count.
    // Y value is the joint matrix offset.
    // A vec4 is necessary for proper alignment
    pub joint_info: glm::Vec4,
}

pub struct PbrPipelineData {
    pub descriptor_pool: DescriptorPool,
    pub uniform_buffer: Buffer,
    pub dynamic_uniform_buffer: Buffer,
    pub dynamic_alignment: u64,
    pub descriptor_set: vk::DescriptorSet,
    pub dummy: DummyImage,
}

impl PbrPipelineData {
    // This should match the number of textures defined in the shader
    pub const MAX_TEXTURES: usize = 100;

    pub fn new(
        context: Arc<VulkanContext>,
        command_pool: &CommandPool,
        number_of_meshes: usize,
        textures: &[&TextureBundle],
        environment_maps: &EnvironmentMapSet,
    ) -> Self {
        let descriptor_set_layout = Self::descriptor_set_layout(context.clone());
        let descriptor_pool = Self::create_descriptor_pool(context.clone());
        let descriptor_set = descriptor_pool
            .allocate_descriptor_sets(descriptor_set_layout.layout(), 1)
            .unwrap()[0];

        let uniform_buffer = Buffer::new_mapped_basic(
            context.clone(),
            mem::size_of::<UniformBufferObject>() as _,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk_mem::MemoryUsage::CpuToGpu,
        )
        .unwrap();

        let dynamic_alignment = Self::calculate_dynamic_alignment(context.clone());

        let dynamic_uniform_buffer = Buffer::new_mapped_basic(
            context.clone(),
            (number_of_meshes as u64 * dynamic_alignment) as vk::DeviceSize,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk_mem::MemoryUsage::CpuToGpu,
        )
        .unwrap();

        let data = PbrPipelineData {
            descriptor_pool,
            uniform_buffer,
            dynamic_uniform_buffer,
            descriptor_set,
            dynamic_alignment,
            dummy: DummyImage::new(context.clone(), &command_pool),
        };

        data.update_descriptor_set(context, number_of_meshes, textures, environment_maps);

        data
    }

    fn calculate_dynamic_alignment(context: Arc<VulkanContext>) -> u64 {
        let minimum_ubo_alignment = context
            .physical_device_properties()
            .limits
            .min_uniform_buffer_offset_alignment;
        let dynamic_alignment = std::mem::size_of::<DynamicUniformBufferObject>() as u64;
        if minimum_ubo_alignment > 0 {
            (dynamic_alignment + minimum_ubo_alignment - 1) & !(minimum_ubo_alignment - 1)
        } else {
            dynamic_alignment
        }
    }

    pub fn descriptor_set_layout(context: Arc<VulkanContext>) -> DescriptorSetLayout {
        let ubo_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .build();
        let dynamic_ubo_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(1)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .build();
        let sampler_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(2)
            .descriptor_count(Self::MAX_TEXTURES as _)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();
        let irradiance_cubemap_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(3)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();
        let prefilter_cubemap_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(4)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();
        let brdflut_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(5)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();

        let bindings = [
            ubo_binding,
            dynamic_ubo_binding,
            sampler_binding,
            irradiance_cubemap_binding,
            prefilter_cubemap_binding,
            brdflut_binding,
        ];

        let layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();
        DescriptorSetLayout::new(context, layout_create_info).unwrap()
    }

    fn create_descriptor_pool(context: Arc<VulkanContext>) -> DescriptorPool {
        let ubo_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
        };

        let dynamic_ubo_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
            descriptor_count: 1,
        };

        let sampler_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: Self::MAX_TEXTURES as _,
        };

        let irradiance_cubemap_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        };

        let prefilter_cubemap_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        };

        let brdflut_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        };

        let pool_sizes = [
            ubo_pool_size,
            dynamic_ubo_pool_size,
            sampler_pool_size,
            irradiance_cubemap_pool_size,
            prefilter_cubemap_pool_size,
            brdflut_pool_size,
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(1)
            .build();

        DescriptorPool::new(context, pool_info).unwrap()
    }

    fn update_descriptor_set(
        &self,
        context: Arc<VulkanContext>,
        number_of_meshes: usize,
        textures: &[&TextureBundle],
        environment_maps: &EnvironmentMapSet,
    ) {
        let uniform_buffer_size = mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
        let buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(self.uniform_buffer.buffer())
            .offset(0)
            .range(uniform_buffer_size)
            .build();
        let buffer_infos = [buffer_info];

        let dynamic_uniform_buffer_size =
            (number_of_meshes as u64 * self.dynamic_alignment) as vk::DeviceSize;
        let dynamic_buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(self.dynamic_uniform_buffer.buffer())
            .offset(0)
            .range(dynamic_uniform_buffer_size)
            .build();
        let dynamic_buffer_infos = [dynamic_buffer_info];

        let mut image_infos = textures
            .iter()
            .map(|texture| {
                vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(texture.view.view())
                    .sampler(texture.sampler.sampler())
                    .build()
            })
            .collect::<Vec<_>>();

        let number_of_images = image_infos.len();
        let required_images = Self::MAX_TEXTURES;
        if number_of_images < required_images {
            let remaining = required_images - number_of_images;
            for _ in 0..remaining {
                image_infos.push(
                    vk::DescriptorImageInfo::builder()
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .image_view(self.dummy.view().view())
                        .sampler(self.dummy.sampler().sampler())
                        .build(),
                );
            }
        }

        let irradiance_cubemap_image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(environment_maps.irradiance.cubemap.view.view())
            .sampler(environment_maps.irradiance.cubemap.sampler.sampler())
            .build();
        let irradiance_cubemap_image_infos = [irradiance_cubemap_image_info];

        let prefilter_cubemap_image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(environment_maps.prefilter.cubemap.view.view())
            .sampler(environment_maps.prefilter.cubemap.sampler.sampler())
            .build();
        let prefilter_cubemap_image_infos = [prefilter_cubemap_image_info];

        let brdflut_image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(environment_maps.brdflut.view.view())
            .sampler(environment_maps.brdflut.sampler.sampler())
            .build();
        let brdflut_image_infos = [brdflut_image_info];

        let ubo_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(&buffer_infos)
            .build();

        let dynamic_ubo_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(1)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .buffer_info(&dynamic_buffer_infos)
            .build();

        let sampler_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(2)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_infos)
            .build();

        let irradiance_cubemap_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(3)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&irradiance_cubemap_image_infos)
            .build();

        let prefilter_cubemap_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(4)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&prefilter_cubemap_image_infos)
            .build();

        let brdflut_descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(5)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&brdflut_image_infos)
            .build();

        let descriptor_writes = vec![
            ubo_descriptor_write,
            dynamic_ubo_descriptor_write,
            sampler_descriptor_write,
            irradiance_cubemap_descriptor_write,
            prefilter_cubemap_descriptor_write,
            brdflut_descriptor_write,
        ];

        unsafe {
            context
                .logical_device()
                .logical_device()
                .update_descriptor_sets(&descriptor_writes, &[])
        }
    }
}

#[derive(Default)]
pub struct GltfOffsets {
    pub texture_offset: i32,
    pub mesh_offset: usize,
    pub index_offset: u32,
    pub vertex_offset: u32,
}

pub struct PbrRenderer {
    command_buffer: vk::CommandBuffer,
    pipeline_layout: vk::PipelineLayout,
    dynamic_alignment: u64,
    descriptor_set: vk::DescriptorSet,
}

impl PbrRenderer {
    pub fn new(
        command_buffer: vk::CommandBuffer,
        pipeline: &GraphicsPipeline,
        pipeline_data: &PbrPipelineData,
    ) -> Self {
        Self {
            command_buffer,
            pipeline_layout: pipeline.layout(),
            dynamic_alignment: pipeline_data.dynamic_alignment,
            descriptor_set: pipeline_data.descriptor_set,
        }
    }

    pub fn draw_asset(
        &self,
        device: &ash::Device,
        asset: &GltfAsset,
        offsets: &GltfOffsets,
        alpha_mode: AlphaMode,
    ) {
        asset.walk(|node_index, graph| {
            if let Some(mesh) = graph[node_index].mesh.as_ref() {
                unsafe {
                    device.cmd_bind_descriptor_sets(
                        self.command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline_layout,
                        0,
                        &[self.descriptor_set],
                        &[
                            ((offsets.mesh_offset + mesh.mesh_id) as u64 * self.dynamic_alignment)
                                as _,
                        ],
                    );
                }

                for primitive in mesh.primitives.iter() {
                    let mut primitive_alpha_mode = AlphaMode::Opaque;
                    if let Some(material_index) = primitive.material_index {
                        let primitive_material = asset
                            .gltf
                            .materials()
                            .nth(material_index)
                            .expect("Failed to retrieve material!");
                        primitive_alpha_mode = primitive_material.alpha_mode();
                    }

                    if primitive_alpha_mode != alpha_mode {
                        continue;
                    }

                    let material =
                        Self::create_material(&asset, &primitive, offsets.texture_offset);
                    unsafe {
                        device.cmd_push_constants(
                            self.command_buffer,
                            self.pipeline_layout,
                            vk::ShaderStageFlags::ALL_GRAPHICS,
                            0,
                            byte_slice_from(&material),
                        );

                        device.cmd_draw_indexed(
                            self.command_buffer,
                            primitive.number_of_indices,
                            1,
                            offsets.index_offset + primitive.first_index,
                            offsets.vertex_offset as _,
                            0,
                        );
                    }
                }
            }
        });
    }

    fn create_material(
        asset: &GltfAsset,
        primitive: &Primitive,
        texture_offset: i32,
    ) -> PushConstantBlockMaterial {
        let mut material = PushConstantBlockMaterial {
            base_color_factor: glm::vec4(0.0, 0.0, 0.0, 1.0),
            emissive_factor: glm::Vec3::identity(),
            color_texture_set: -1,
            metallic_roughness_texture_set: -1,
            normal_texture_set: -1,
            occlusion_texture_set: -1,
            emissive_texture_set: -1,
            metallic_factor: 0.0,
            roughness_factor: 0.0,
            alpha_mode: gltf::material::AlphaMode::Opaque as i32,
            alpha_cutoff: 0.0,
        };

        if let Some(material_index) = primitive.material_index {
            let primitive_material = asset
                .gltf
                .materials()
                .nth(material_index)
                .expect("Failed to retrieve material!");
            let pbr = primitive_material.pbr_metallic_roughness();

            material.base_color_factor = glm::Vec4::from(pbr.base_color_factor());
            material.metallic_factor = pbr.metallic_factor();
            material.roughness_factor = pbr.roughness_factor();
            material.emissive_factor = glm::Vec3::from(primitive_material.emissive_factor());
            material.alpha_mode = primitive_material.alpha_mode() as i32;
            material.alpha_cutoff = primitive_material.alpha_cutoff();

            if let Some(base_color_texture) = pbr.base_color_texture() {
                material.color_texture_set =
                    texture_offset + base_color_texture.texture().index() as i32;
            }

            if let Some(metallic_roughness_texture) = pbr.metallic_roughness_texture() {
                material.metallic_roughness_texture_set =
                    texture_offset + metallic_roughness_texture.texture().index() as i32;
            }

            if let Some(normal_texture) = primitive_material.normal_texture() {
                material.normal_texture_set =
                    texture_offset + normal_texture.texture().index() as i32;
            }

            if let Some(occlusion_texture) = primitive_material.occlusion_texture() {
                material.occlusion_texture_set =
                    texture_offset + occlusion_texture.texture().index() as i32;
            }

            if let Some(emissive_texture) = primitive_material.emissive_texture() {
                material.emissive_texture_set =
                    texture_offset + emissive_texture.texture().index() as i32;
            }
        }

        material
    }
}

pub struct EnvironmentMapSet {
    brdflut: Brdflut,
    hdr: HdrCubemap,
    irradiance: IrradianceMap,
    prefilter: PrefilterMap,
}

impl EnvironmentMapSet {
    pub fn new(
        context: Arc<VulkanContext>,
        command_pool: &CommandPool,
        shader_cache: &mut ShaderCache,
    ) -> Self {
        debug!("Creating Brdflut");
        let brdflut = Brdflut::new(context.clone(), command_pool);

        let cubemap_path = "assets/skyboxes/walk_of_fame/walk_of_fame.hdr";

        debug!("Creating HDR cubemap");
        let hdr = HdrCubemap::new(context.clone(), command_pool, &cubemap_path, shader_cache);

        debug!("Creating Irradiance cubemap");
        let irradiance = IrradianceMap::new(
            context.clone(),
            &command_pool,
            &hdr.as_ref().expect("Failed to lookup hdr cubemap!").cubemap,
        );

        debug!("Creating Prefilter cubemap");
        let prefilter = PrefilterMap::new(
            context,
            &command_pool,
            &hdr.as_ref().expect("Failed to lookup hdr cubemap!").cubemap,
        );

        Self {
            brdflut,
            hdr: hdr.unwrap(),
            irradiance,
            prefilter,
        }
    }
}

pub struct PbrScene {
    context: Arc<VulkanContext>,
    asset_geometry_buffer: GeometryBuffer,
    environment_maps: EnvironmentMapSet,
    skybox_pipeline: Option<RenderPipeline>,
    skybox_pipeline_data: SkyboxPipelineData,
    pbr_pipeline: Option<RenderPipeline>,
    pbr_pipeline_blend: Option<RenderPipeline>,
    pbr_pipeline_data: PbrPipelineData,
    assets: Vec<GltfAsset>,
}

impl PbrScene {
    pub fn new(
        context: Arc<VulkanContext>,
        command_pool: &CommandPool,
        shader_cache: &mut ShaderCache,
        render_pass: Arc<RenderPass>,
        asset_names: &[&str],
    ) -> Self {
        let environment_maps = EnvironmentMapSet::new(context.clone(), command_pool, shader_cache);

        let assets = asset_names
            .iter()
            .map(|name| GltfAsset::new(context.clone(), &command_pool, &name))
            .collect::<Vec<_>>();

        let number_of_meshes = assets.iter().fold(0, |total_meshes, asset| {
            total_meshes + asset.number_of_meshes
        });

        let vertices = assets
            .iter()
            .flat_map(|asset| asset.vertices.iter().copied())
            .collect::<Vec<_>>();

        let indices = assets
            .iter()
            .flat_map(|asset| asset.indices.iter().copied())
            .collect::<Vec<_>>();

        let asset_geometry_buffer = GeometryBuffer::new(&command_pool, &vertices, Some(&indices));

        let textures = assets
            .iter()
            .flat_map(|asset| &asset.textures)
            .collect::<Vec<_>>();

        let pbr_pipeline_data = PbrPipelineData::new(
            context.clone(),
            &command_pool,
            number_of_meshes,
            &textures,
            &environment_maps,
        );

        let skybox_pipeline_data = SkyboxPipelineData::new(
            context.clone(),
            &command_pool,
            &environment_maps.hdr.cubemap,
        );

        let mut pbr_scene_data = Self {
            context,
            asset_geometry_buffer,
            environment_maps,
            skybox_pipeline: None,
            skybox_pipeline_data,
            pbr_pipeline: None,
            pbr_pipeline_blend: None,
            pbr_pipeline_data,
            assets,
        };

        pbr_scene_data.recreate_pipelines(shader_cache, render_pass);
        pbr_scene_data
    }

    pub fn recreate_pipelines(
        &mut self,
        shader_cache: &mut ShaderCache,
        render_pass: Arc<RenderPass>,
    ) {
        let descriptions = GltfAsset::create_vertex_input_descriptions();
        let attributes = GltfAsset::create_vertex_attributes();
        let vertex_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&descriptions)
            .vertex_attribute_descriptions(&attributes)
            .build();

        let push_constant_range = vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::ALL_GRAPHICS)
            .size(mem::size_of::<PushConstantBlockMaterial>() as u32)
            .build();

        let shader_paths = ShaderPathSetBuilder::default()
            .vertex("assets/shaders/pbr/pbr.vert.spv")
            .fragment("assets/shaders/pbr/pbr.frag.spv")
            .build()
            .unwrap();
        let shader_set = shader_cache
            .create_shader_set(self.context.clone(), &shader_paths)
            .unwrap();

        let descriptor_set_layout =
            Arc::new(PbrPipelineData::descriptor_set_layout(self.context.clone()));

        let mut settings = RenderPipelineSettingsBuilder::default()
            .render_pass(render_pass.clone())
            .vertex_state_info(vertex_state_info)
            .descriptor_set_layout(descriptor_set_layout)
            .shader_set(shader_set)
            .rasterization_samples(self.context.max_usable_samples())
            .sample_shading_enabled(true)
            .push_constant_range(push_constant_range)
            .build()
            .expect("Failed to create render pipeline settings");

        self.pbr_pipeline = None;
        self.pbr_pipeline_blend = None;
        self.pbr_pipeline = Some(RenderPipeline::new(self.context.clone(), settings.clone()));
        settings.blended = true;
        self.pbr_pipeline_blend = Some(RenderPipeline::new(self.context.clone(), settings));

        self.skybox_pipeline = None;
        self.skybox_pipeline = Some(create_skybox_pipeline(
            self.context.clone(),
            shader_cache,
            render_pass,
        ));
    }

    pub fn issue_commands(
        &mut self,
        command_buffer: vk::CommandBuffer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.render_skybox(command_buffer);
        self.render_pbr_assets(command_buffer);
        Ok(())
    }

    fn render_skybox(&mut self, command_buffer: vk::CommandBuffer) {
        let skybox_pipeline = self.skybox_pipeline.as_ref().unwrap();
        skybox_pipeline.bind(
            self.context.logical_device().logical_device(),
            command_buffer,
        );

        let skybox_renderer =
            SkyboxRenderer::new(command_buffer, &skybox_pipeline, &self.skybox_pipeline_data);

        skybox_renderer.draw(
            self.context.logical_device().logical_device(),
            &self.skybox_pipeline_data.cube,
        );
    }

    fn render_pbr_assets(&mut self, command_buffer: vk::CommandBuffer) {
        let pbr_pipeline = self.pbr_pipeline.as_ref().unwrap();
        let pbr_pipeline_blended = self.pbr_pipeline_blend.as_ref().unwrap();
        let pbr_renderer = PbrRenderer::new(
            command_buffer,
            &pbr_pipeline.pipeline,
            &self.pbr_pipeline_data,
        );
        let pbr_renderer_blended = PbrRenderer::new(
            command_buffer,
            &pbr_pipeline.pipeline,
            &self.pbr_pipeline_data,
        );

        let offsets = [0];
        let vertex_buffers = [self.asset_geometry_buffer.vertex_buffer.buffer()];

        unsafe {
            self.context
                .logical_device()
                .logical_device()
                .cmd_bind_vertex_buffers(command_buffer, 0, &vertex_buffers, &offsets);
            self.context
                .logical_device()
                .logical_device()
                .cmd_bind_index_buffer(
                    command_buffer,
                    self.asset_geometry_buffer
                        .index_buffer
                        .as_ref()
                        .expect("Failed to get an index buffer!")
                        .buffer(),
                    0,
                    vk::IndexType::UINT32,
                );
        }

        [AlphaMode::Opaque, AlphaMode::Mask, AlphaMode::Blend]
            .iter()
            .for_each(|alpha_mode| {
                match alpha_mode {
                    AlphaMode::Opaque => pbr_pipeline.bind(
                        self.context.logical_device().logical_device(),
                        command_buffer,
                    ),
                    AlphaMode::Blend => pbr_pipeline_blended.bind(
                        self.context.logical_device().logical_device(),
                        command_buffer,
                    ),
                    _ => {}
                }

                let mut offsets = GltfOffsets::default();
                for asset in self.assets.iter() {
                    if *alpha_mode == AlphaMode::Blend {
                        pbr_renderer_blended.draw_asset(
                            self.context.logical_device().logical_device(),
                            &asset,
                            &offsets,
                            *alpha_mode,
                        );
                    } else {
                        pbr_renderer.draw_asset(
                            self.context.logical_device().logical_device(),
                            &asset,
                            &offsets,
                            *alpha_mode,
                        );
                    }
                    offsets.texture_offset += asset.textures.len() as i32;
                    offsets.mesh_offset += asset.number_of_meshes;
                    offsets.index_offset += asset.indices.len() as u32;
                    offsets.vertex_offset +=
                        (asset.vertices.len() / GltfAsset::vertex_stride()) as u32;
                }
            });
    }

    pub fn update(
        &mut self,
        camera_position: glm::Vec3,
        projection: glm::Mat4,
        view: glm::Mat4,
        delta_time: f64,
    ) {
        let skybox_ubo = SkyboxUniformBufferObject { view, projection };
        let skybox_ubos = [skybox_ubo];
        self.skybox_pipeline_data
            .uniform_buffer
            .upload_to_buffer(&skybox_ubos, 0)
            .unwrap();

        for asset in self.assets.iter_mut() {
            for animation in asset.animations.iter_mut() {
                animation.time += 0.75 * delta_time as f32;
            }

            // Only animate first animation
            asset.animate(0);
        }

        let mut ubo = UniformBufferObject {
            camera_position: glm::vec4(
                camera_position.x,
                camera_position.y,
                camera_position.z,
                1.0,
            ),
            view,
            projection,
            joint_matrices: [glm::Mat4::identity(); UniformBufferObject::MAX_NUM_JOINTS],
        };

        let spacing = glm::vec3(20.0, 0.0, 0.0);
        let mut asset_transform = glm::Mat4::identity();
        let mut mesh_offset = 0;
        let mut joint_offset = 0;
        for asset in self.assets.iter() {
            asset.walk_mut(|node_index, graph| {
                let global_transform =
                    GltfAsset::calculate_global_transform(node_index, graph);
                if let Some(mesh) = graph[node_index].mesh.as_ref() {
                        let mut dynamic_ubo = DynamicUniformBufferObject {
                            model: asset_transform * global_transform,
                            joint_info: glm::vec4(0.0, 0.0, 0.0, 0.0),
                        };

                        if let Some(skin) = graph[node_index].skin.as_ref() {
                            let joint_count = skin.joints.len();
                            dynamic_ubo.joint_info = glm::vec4(joint_count as f32, joint_offset as f32, 0.0, 0.0);
                            for (index, joint) in skin.joints.iter().enumerate() {
                                if index > UniformBufferObject::MAX_NUM_JOINTS {
                                    eprintln!("Skin joint count {} is greater than the maximum joint limit of {}!", dynamic_ubo.joint_info, UniformBufferObject::MAX_NUM_JOINTS);
                                }

                                let joint_node_index = GltfAsset::matching_node_index(joint.target_gltf_index, &graph)
                                    .expect("Failed to find joint target node index!");

                                let joint_global_transform =
                                    GltfAsset::calculate_global_transform(joint_node_index, &graph);

                                let joint_matrix = glm::inverse(&global_transform)
                                    * joint_global_transform
                                    * joint.inverse_bind_matrix;

                                ubo.joint_matrices[joint_offset + index] = joint_matrix;
                            }
                            joint_offset += joint_count;
                        }

                        let dynamic_ubos = [dynamic_ubo];
                        let buffer = &self.pbr_pipeline_data.dynamic_uniform_buffer;
                        let offset = (self.pbr_pipeline_data.dynamic_alignment
                                      * (mesh_offset + mesh.mesh_id) as u64)
                            as usize;

                        buffer.upload_to_buffer_aligned(
                            &dynamic_ubos,
                            offset,
                            self.pbr_pipeline_data.dynamic_alignment,
                        ).unwrap();

                        let dynamic_ubo_size = (asset.number_of_meshes as u64
                                                * self.pbr_pipeline_data.dynamic_alignment)
                            as u64;
                        buffer
                            .flush(offset, dynamic_ubo_size as _)
                            .expect("Failed to flush buffer!");
                }
            });
            mesh_offset += asset.number_of_meshes;
            asset_transform = glm::translate(&asset_transform, &spacing)
        }

        let ubos = [ubo];
        self.pbr_pipeline_data
            .uniform_buffer
            .upload_to_buffer(&ubos, 0)
            .unwrap();
    }
}
