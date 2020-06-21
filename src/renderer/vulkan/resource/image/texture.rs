use crate::renderer::vulkan::{
    core::VulkanContext,
    resource::{
        image::{ImageView, Sampler},
        Buffer, CommandPool,
    },
};
use ash::{version::DeviceV1_0, vk};
use gltf::image::Format;
use image::{DynamicImage, ImageBuffer, Pixel, RgbImage};
use snafu::{OptionExt, ResultExt, Snafu};
use std::{iter, sync::Arc};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum Error {
    #[snafu(display("Failed to open HDR file {}: {}", path, source))]
    OpenHdrFile {
        source: std::io::Error,
        path: String,
    },

    #[snafu(display("Failed to create HDR decoder: {}", source))]
    CreateHdrDecoder { source: image::ImageError },

    #[snafu(display("Failed to read HDR image: {}", source))]
    ReadHdrImage { source: image::ImageError },

    #[snafu(display("Failed to open image file {}: {}", path, source))]
    OpenImageFile {
        source: image::ImageError,
        path: String,
    },

    #[snafu(display("Failed to create texture: {}", source))]
    CreateTexture { source: vk_mem::error::Error },

    #[snafu(display("Failed to create image buffer"))]
    CreateImageBuffer,

    #[snafu(display("Failed to image copy buffer: {}", source))]
    CreateImageCopyBuffer {
        source: crate::renderer::vulkan::resource::buffer::Error,
    },

    #[snafu(display("Failed to upload to image copy buffer: {}", source))]
    UploadImageCopyBuffer {
        source: crate::renderer::vulkan::resource::buffer::Error,
    },

    #[snafu(display("Failed to cubemap image copy buffer: {}", source))]
    CreateCubemapImageCopyBuffer {
        source: crate::renderer::vulkan::resource::buffer::Error,
    },

    #[snafu(display("Failed to upload to a cubemap image copy buffer: {}", source))]
    UploadCubemapImageCopyBuffer {
        source: crate::renderer::vulkan::resource::buffer::Error,
    },

    #[snafu(display("Failed to transition mipmap image layout to transfer: {}", source))]
    TransitionMipLayoutToTransfer {
        source: crate::renderer::vulkan::resource::command_pool::Error,
    },

    #[snafu(display(
        "Failed to transition mipmap image layout to fragment shader: {}",
        source
    ))]
    TransitionMipLayoutToFrag {
        source: crate::renderer::vulkan::resource::command_pool::Error,
    },

    #[snafu(display("Failed to blit from source image to a mipmap: {}", source))]
    BlitMipMap {
        source: crate::renderer::vulkan::resource::command_pool::Error,
    },

    #[snafu(display("Failed to transition image layout: {}", source))]
    TransitionImageLayout {
        source: crate::renderer::vulkan::resource::command_pool::Error,
    },

    #[snafu(display("Failed to create image view: {}", source))]
    CreateImageView {
        source: crate::renderer::vulkan::resource::image::image_view::Error,
    },

    #[snafu(display("Failed to create sampler: {}", source))]
    CreateSampler {
        source: crate::renderer::vulkan::resource::sampler::Error,
    },

    #[snafu(display("Failed to create cubemap image view: {}", source))]
    CreateCubemapImageView {
        source: crate::renderer::vulkan::resource::image_view::Error,
    },

    #[snafu(display("Failed to create cubemap sampler: {}", source))]
    CreateCubemapSampler {
        source: crate::renderer::vulkan::resource::image::sampler::Error,
    },

    #[snafu(display("Failed to copy buffer to image: {}", source))]
    CopyBufferToImage {
        source: crate::renderer::vulkan::resource::command_pool::Error,
    },
}

pub struct ImageLayoutTransition {
    pub old_layout: vk::ImageLayout,
    pub new_layout: vk::ImageLayout,
    pub src_access_mask: vk::AccessFlags,
    pub dst_access_mask: vk::AccessFlags,
    pub src_stage_mask: vk::PipelineStageFlags,
    pub dst_stage_mask: vk::PipelineStageFlags,
}

pub struct TextureDescription {
    pub format: vk::Format,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub mip_levels: u32,
}

impl TextureDescription {
    pub fn empty(width: u32, height: u32, format: vk::Format) -> Self {
        Self {
            format,
            width,
            height,
            pixels: Vec::new(),
            mip_levels: Self::calculate_mip_levels(width, height),
        }
    }

    pub fn from_hdr(path: &str) -> Result<Self> {
        let file = std::fs::File::open(&path).context(OpenHdrFile {
            path: path.to_string(),
        })?;

        let decoder = image::hdr::HdrDecoder::new(std::io::BufReader::new(file))
            .context(CreateHdrDecoder {})?;

        let metadata = decoder.metadata();
        let decoded = decoder.read_image_hdr().context(ReadHdrImage {})?;
        let format = vk::Format::R32G32B32A32_SFLOAT;
        let width = metadata.width as u32;
        let height = metadata.height as u32;
        let mip_levels = Self::calculate_mip_levels(width, height);
        let data = decoded
            .iter()
            .flat_map(|pixel| vec![pixel[0], pixel[1], pixel[2], 1.0])
            .collect::<Vec<_>>();
        let pixels =
            unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) }
                .to_vec();

        let description = Self {
            format,
            width,
            height,
            pixels,
            mip_levels,
        };

        Ok(description)
    }

    pub fn from_file(path: &str) -> Result<Self> {
        let image = image::open(path).context(OpenImageFile {
            path: path.to_string(),
        })?;
        Self::from_image(&image)
    }

    pub fn from_image(image: &DynamicImage) -> Result<Self> {
        let (format, (width, height)) = match image {
            DynamicImage::ImageRgb8(buffer) => (vk::Format::R8G8B8_UNORM, buffer.dimensions()),
            DynamicImage::ImageRgba8(buffer) => (vk::Format::R8G8B8A8_UNORM, buffer.dimensions()),
            DynamicImage::ImageBgr8(buffer) => (vk::Format::B8G8R8_UNORM, buffer.dimensions()),
            DynamicImage::ImageBgra8(buffer) => (vk::Format::B8G8R8A8_UNORM, buffer.dimensions()),
            DynamicImage::ImageRgb16(buffer) => (vk::Format::R16G16B16_UNORM, buffer.dimensions()),
            DynamicImage::ImageRgba16(buffer) => {
                (vk::Format::R16G16B16A16_UNORM, buffer.dimensions())
            }
            _ => panic!("Failed to match the provided image format to a vulkan format!"),
        };

        let mut description = Self {
            format,
            width,
            height,
            pixels: image.to_bytes(),
            mip_levels: Self::calculate_mip_levels(width, height),
        };
        description.convert_24bit_formats()?;
        Ok(description)
    }

    pub fn from_gltf(data: &gltf::image::Data) -> Result<Self> {
        let format = Self::convert_to_vulkan_format(data.format);
        let mut description = Self {
            format,
            width: data.width,
            height: data.height,
            pixels: data.pixels.to_vec(),
            mip_levels: Self::calculate_mip_levels(data.width, data.height),
        };
        description.convert_24bit_formats()?;
        Ok(description)
    }

    pub fn calculate_mip_levels(width: u32, height: u32) -> u32 {
        ((width.min(height) as f32).log2().floor() + 1.0) as u32
    }

    fn convert_24bit_formats(&mut self) -> Result<()> {
        // 24-bit formats are unsupported, so they
        // need to have an alpha channel added to make them 32-bit
        match self.format {
            vk::Format::R8G8B8_UNORM => {
                self.format = vk::Format::R8G8B8A8_UNORM;
                self.attach_alpha_channel()?;
            }
            vk::Format::B8G8R8_UNORM => {
                self.format = vk::Format::B8G8R8A8_UNORM;
                self.attach_alpha_channel()?;
            }
            _ => {}
        };

        Ok(())
    }

    fn attach_alpha_channel(&mut self) -> Result<()> {
        let image_buffer: RgbImage =
            ImageBuffer::from_raw(self.width, self.height, self.pixels.to_vec())
                .context(CreateImageBuffer {})?;

        self.pixels = image_buffer
            .pixels()
            .flat_map(|pixel| pixel.to_rgba().channels().to_vec())
            .collect::<Vec<_>>();

        Ok(())
    }

    fn convert_to_vulkan_format(format: Format) -> vk::Format {
        match format {
            Format::R8 => vk::Format::R8_UNORM,
            Format::R8G8 => vk::Format::R8G8_UNORM,
            Format::R8G8B8A8 => vk::Format::R8G8B8A8_UNORM,
            Format::B8G8R8A8 => vk::Format::B8G8R8A8_UNORM,
            Format::R8G8B8 => vk::Format::R8G8B8_UNORM,
            Format::B8G8R8 => vk::Format::B8G8R8_UNORM,
            Format::R16 => vk::Format::R16_UNORM,
            Format::R16G16 => vk::Format::R16G16_UNORM,
            Format::R16G16B16 => vk::Format::R16G16B16_UNORM,
            Format::R16G16B16A16 => vk::Format::R16G16B16A16_UNORM,
        }
    }
}

// The order of the struct fields matters here
// because it determines drop order
pub struct Texture {
    image: vk::Image,
    allocation: vk_mem::Allocation,
    allocation_info: vk_mem::AllocationInfo,
    context: Arc<VulkanContext>,
}

impl Texture {
    pub fn new(
        context: Arc<VulkanContext>,
        allocation_create_info: &vk_mem::AllocationCreateInfo,
        image_create_info: &vk::ImageCreateInfo,
    ) -> Result<Self> {
        let (image, allocation, allocation_info) = context
            .allocator()
            .create_image(&image_create_info, &allocation_create_info)
            .context(CreateTexture {})?;

        let texture = Self {
            image,
            allocation,
            allocation_info,
            context,
        };

        Ok(texture)
    }

    pub fn upload_texture_data(
        &self,
        command_pool: &CommandPool,
        description: &TextureDescription,
    ) -> Result<()> {
        let region = vk::BufferImageCopy::builder()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width: description.width,
                height: description.height,
                depth: 1,
            })
            .build();
        let regions = [region];

        let buffer = Buffer::new_mapped_basic(
            self.context.clone(),
            self.allocation_info().get_size() as _,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk_mem::MemoryUsage::CpuToGpu,
        )
        .context(CreateImageCopyBuffer {})?;

        buffer
            .upload_to_buffer(&description.pixels, 0)
            .context(UploadImageCopyBuffer {})?;

        let transition = ImageLayoutTransition {
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            src_stage_mask: vk::PipelineStageFlags::TOP_OF_PIPE,
            dst_stage_mask: vk::PipelineStageFlags::TRANSFER,
        };
        self.transition(&command_pool, &transition, description.mip_levels)?;

        command_pool
            .copy_buffer_to_image(buffer.buffer(), self.image(), &regions)
            .unwrap();

        self.generate_mipmaps(&command_pool, &description)?;

        Ok(())
    }

    pub fn generate_mipmaps(
        &self,
        command_pool: &CommandPool,
        texture_description: &TextureDescription,
    ) -> Result<()> {
        let format_properties = self
            .context
            .physical_device_format_properties(texture_description.format);

        if !format_properties
            .optimal_tiling_features
            .contains(vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR)
        {
            panic!(
                "Linear blitting is not supported for format: {:?}",
                texture_description.format
            );
        }

        let mut mip_width = texture_description.width as i32;
        let mut mip_height = texture_description.height as i32;
        for level in 1..texture_description.mip_levels {
            let next_mip_width = if mip_width > 1 {
                mip_width / 2
            } else {
                mip_width
            };

            let next_mip_height = if mip_height > 1 {
                mip_height / 2
            } else {
                mip_height
            };

            let barrier = vk::ImageMemoryBarrier::builder()
                .image(self.image())
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_array_layer: 0,
                    layer_count: 1,
                    level_count: 1,
                    base_mip_level: level - 1,
                })
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .build();
            let barriers = [barrier];

            command_pool
                .transition_image_layout(
                    &barriers,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                )
                .context(TransitionMipLayoutToTransfer {})?;

            let blit = vk::ImageBlit::builder()
                .src_offsets([
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: mip_width,
                        y: mip_height,
                        z: 1,
                    },
                ])
                .src_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: level - 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .dst_offsets([
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: next_mip_width,
                        y: next_mip_height,
                        z: 1,
                    },
                ])
                .dst_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: level,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .build();
            let blits = [blit];

            command_pool
                .execute_command_once(self.context.graphics_queue(), |command_buffer| unsafe {
                    self.context
                        .logical_device()
                        .logical_device()
                        .cmd_blit_image(
                            command_buffer,
                            self.image(),
                            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                            self.image(),
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            &blits,
                            vk::Filter::LINEAR,
                        )
                })
                .context(BlitMipMap {})?;

            let barrier = vk::ImageMemoryBarrier::builder()
                .image(self.image())
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_array_layer: 0,
                    layer_count: 1,
                    level_count: 1,
                    base_mip_level: level - 1,
                })
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .build();
            let barriers = [barrier];

            command_pool
                .transition_image_layout(
                    &barriers,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                )
                .context(TransitionMipLayoutToFrag {})?;

            mip_width = next_mip_width;
            mip_height = next_mip_height;
        }

        let barrier = vk::ImageMemoryBarrier::builder()
            .image(self.image())
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_array_layer: 0,
                layer_count: 1,
                level_count: 1,
                base_mip_level: texture_description.mip_levels - 1,
            })
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)
            .build();
        let barriers = [barrier];

        command_pool
            .transition_image_layout(
                &barriers,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            )
            .context(TransitionMipLayoutToFrag {})?;

        Ok(())
    }

    pub fn transition(
        &self,
        command_pool: &CommandPool,
        transition: &ImageLayoutTransition,
        mip_levels: u32,
    ) -> Result<()> {
        let barrier = vk::ImageMemoryBarrier::builder()
            .old_layout(transition.old_layout)
            .new_layout(transition.new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.image())
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: 1,
            })
            .src_access_mask(transition.src_access_mask)
            .dst_access_mask(transition.dst_access_mask)
            .build();
        let barriers = [barrier];

        command_pool
            .transition_image_layout(
                &barriers,
                transition.src_stage_mask,
                transition.dst_stage_mask,
            )
            .context(TransitionImageLayout {})?;

        Ok(())
    }

    pub fn image(&self) -> vk::Image {
        self.image
    }

    pub fn allocation(&self) -> &vk_mem::Allocation {
        &self.allocation
    }

    pub fn allocation_info(&self) -> &vk_mem::AllocationInfo {
        &self.allocation_info
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        self.context
            .allocator()
            .destroy_image(self.image, &self.allocation)
            .expect("Failed to destroy image!");
    }
}

pub struct CubemapFaces {
    pub right: String,
    pub left: String,
    pub top: String,
    pub bottom: String,
    pub front: String,
    pub back: String,
}

impl CubemapFaces {
    pub fn ordered_faces(&self) -> impl Iterator<Item = String> {
        iter::once(self.right.to_string())
            .chain(iter::once(self.left.to_string()))
            .chain(iter::once(self.top.to_string()))
            .chain(iter::once(self.bottom.to_string()))
            .chain(iter::once(self.back.to_string()))
            .chain(iter::once(self.front.to_string()))
    }

    pub fn create_descriptions(&self) -> Vec<Result<TextureDescription>> {
        self.ordered_faces()
            .map(|face| TextureDescription::from_file(&face))
            .collect::<Vec<_>>()
    }
}

pub struct Cubemap {
    pub texture: Texture,
    pub view: ImageView,
    pub sampler: Sampler,
    pub description: TextureDescription,
    context: Arc<VulkanContext>,
}

impl Cubemap {
    pub fn new(context: Arc<VulkanContext>, dimension: u32, format: vk::Format) -> Result<Self> {
        let description = TextureDescription::empty(dimension, dimension, format);
        let texture = Self::create_texture(context.clone(), &description)?;
        let view = Self::create_view(context.clone(), &texture, &description)?;
        let sampler = Self::create_sampler(context.clone(), &description)?;

        let cubemap = Self {
            texture,
            view,
            sampler,
            description,
            context,
        };

        Ok(cubemap)
    }

    pub fn upload_texture_data(
        &self,
        command_pool: &CommandPool,
        descriptions: &[TextureDescription],
    ) -> Result<()> {
        let mut pixels: Vec<u8> = Vec::new();
        descriptions.iter().for_each(|description| {
            pixels.extend(&description.pixels);
        });

        let buffer = Buffer::new_mapped_basic(
            self.context.clone(),
            self.texture.allocation_info().get_size() as _,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk_mem::MemoryUsage::CpuToGpu,
        )
        .context(CreateCubemapImageCopyBuffer {})?;

        buffer
            .upload_to_buffer(&pixels, 0)
            .context(UploadCubemapImageCopyBuffer {})?;

        let transition = ImageLayoutTransition {
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            src_stage_mask: vk::PipelineStageFlags::TOP_OF_PIPE,
            dst_stage_mask: vk::PipelineStageFlags::TRANSFER,
        };
        self.transition(&command_pool, &transition)?;

        let mut offset = 0;
        let regions = descriptions
            .iter()
            .enumerate()
            .map(|(face_index, face)| {
                let region = vk::BufferImageCopy::builder()
                    .buffer_offset(offset as _)
                    .buffer_row_length(0)
                    .buffer_image_height(0)
                    .image_subresource(vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: face_index as _,
                        layer_count: 1,
                    })
                    .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                    .image_extent(vk::Extent3D {
                        width: self.description.width,
                        height: self.description.height,
                        depth: 1,
                    })
                    .build();
                offset += face.pixels.len() * std::mem::size_of::<u8>();
                region
            })
            .collect::<Vec<_>>();

        command_pool
            .copy_buffer_to_image(buffer.buffer(), self.texture.image(), &regions)
            .context(CopyBufferToImage {})?;

        let transition = ImageLayoutTransition {
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            dst_access_mask: vk::AccessFlags::SHADER_READ,
            src_stage_mask: vk::PipelineStageFlags::TRANSFER,
            dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
        };
        self.transition(&command_pool, &transition)?;

        Ok(())
    }

    fn create_texture(
        context: Arc<VulkanContext>,
        description: &TextureDescription,
    ) -> Result<Texture> {
        let image_create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: description.width,
                height: description.height,
                depth: 1,
            })
            .mip_levels(description.mip_levels)
            .array_layers(6)
            .format(description.format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(
                vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::SAMPLED,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .flags(vk::ImageCreateFlags::CUBE_COMPATIBLE)
            .build();

        let allocation_create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::GpuOnly,
            ..Default::default()
        };

        Texture::new(context, &allocation_create_info, &image_create_info)
    }

    fn create_view(
        context: Arc<VulkanContext>,
        texture: &Texture,
        description: &TextureDescription,
    ) -> Result<ImageView> {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(texture.image())
            .view_type(vk::ImageViewType::CUBE)
            .format(description.format)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            })
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: description.mip_levels,
                base_array_layer: 0,
                layer_count: 6,
            })
            .build();
        let image_view = ImageView::new(context, create_info).context(CreateCubemapImageView {})?;
        Ok(image_view)
    }

    fn create_sampler(
        context: Arc<VulkanContext>,
        description: &TextureDescription,
    ) -> Result<Sampler> {
        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .anisotropy_enable(true)
            .max_anisotropy(16.0)
            .border_color(vk::BorderColor::INT_OPAQUE_WHITE)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(description.mip_levels as _)
            .build();
        let sampler = Sampler::new(context, sampler_info).context(CreateSampler {})?;
        Ok(sampler)
    }

    // TODO: Merge this with the texture transition
    pub fn transition(
        &self,
        command_pool: &CommandPool,
        transition: &ImageLayoutTransition,
    ) -> Result<()> {
        let barrier = vk::ImageMemoryBarrier::builder()
            .old_layout(transition.old_layout)
            .new_layout(transition.new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.texture.image())
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: self.description.mip_levels,
                base_array_layer: 0,
                layer_count: 6,
            })
            .src_access_mask(transition.src_access_mask)
            .dst_access_mask(transition.dst_access_mask)
            .build();
        let barriers = [barrier];

        command_pool
            .transition_image_layout(
                &barriers,
                transition.src_stage_mask,
                transition.dst_stage_mask,
            )
            .context(TransitionImageLayout {})?;

        Ok(())
    }
}

pub struct TextureBundle {
    pub texture: Texture,
    pub view: ImageView,
    pub sampler: Sampler,
}

impl TextureBundle {
    pub fn new(
        context: Arc<VulkanContext>,
        command_pool: &CommandPool,
        description: &TextureDescription,
    ) -> Result<Self> {
        let texture = Self::create_texture(context.clone(), &description)?;

        texture.upload_texture_data(&command_pool, &description)?;

        let view = Self::create_image_view(context.clone(), &texture, &description)?;

        let sampler = Self::create_sampler(context, description.mip_levels)?;

        let texture_bundle = Self {
            texture,
            view,
            sampler,
        };

        Ok(texture_bundle)
    }

    fn create_texture(
        context: Arc<VulkanContext>,
        description: &TextureDescription,
    ) -> Result<Texture> {
        let image_create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: description.width,
                height: description.height,
                depth: 1,
            })
            .mip_levels(description.mip_levels)
            .array_layers(1)
            .format(description.format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(
                vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::SAMPLED,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .flags(vk::ImageCreateFlags::empty())
            .build();

        let allocation_create_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::GpuOnly,
            ..Default::default()
        };

        Texture::new(context, &allocation_create_info, &image_create_info)
    }

    fn create_image_view(
        context: Arc<VulkanContext>,
        texture: &Texture,
        description: &TextureDescription,
    ) -> Result<ImageView> {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(texture.image())
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(description.format)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            })
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: description.mip_levels,
                base_array_layer: 0,
                layer_count: 1,
            })
            .build();
        ImageView::new(context, create_info).context(CreateImageView {})
    }

    fn create_sampler(context: Arc<VulkanContext>, mip_levels: u32) -> Result<Sampler> {
        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(true)
            .max_anisotropy(16.0)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(mip_levels as _)
            .build();
        Sampler::new(context, sampler_info).context(CreateCubemapSampler {})
    }
}
