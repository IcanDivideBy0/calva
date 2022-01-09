use anyhow::Result;
use renderer::{util::mipmap::MipmapGenerator, wgpu};
use std::ops::Deref;

pub fn buffer_reader<'a>(
    buffers: &'a [gltf::buffer::Data],
) -> impl Clone + Fn(gltf::Buffer) -> Option<&'a [u8]> {
    |buffer: gltf::Buffer| buffers.get(buffer.index()).map(Deref::deref)
}

pub fn image_reader(
    images: &[gltf::image::Data],
) -> impl FnMut(gltf::Texture) -> Option<image::DynamicImage> + '_ {
    |texture: gltf::Texture| -> Option<_> {
        let image_index = texture.source().index();

        let image_data = images.get(image_index)?;

        // 3 channels texture formats are not supported by WebGPU
        // https://github.com/gpuweb/gpuweb/issues/66
        if image_data.format == gltf::image::Format::R8G8B8 {
            let buf = image::ImageBuffer::from_raw(
                image_data.width,
                image_data.height,
                image_data.pixels.clone(),
            )?;

            Some(image::DynamicImage::ImageRgb8(buf))
        } else {
            let buf = image::ImageBuffer::from_raw(
                image_data.width,
                image_data.height,
                image_data.pixels.clone(),
            )?;

            Some(image::DynamicImage::ImageRgba8(buf))
        }
    }
}

pub fn texture_builder<'a>(
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
) -> impl FnMut(Option<&str>, wgpu::TextureFormat, image::DynamicImage) -> Result<wgpu::Texture> + 'a
{
    let mipmap_generator = MipmapGenerator::new(device);

    move |label: Option<&str>, format: wgpu::TextureFormat, image: image::DynamicImage| {
        let buf = image.to_rgba8();

        let size = wgpu::Extent3d {
            width: buf.width(),
            height: buf.height(),
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: size.max_mips(),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
        };

        let texture = device.create_texture(&desc);

        queue.write_texture(
            texture.as_image_copy(),
            &buf.to_vec(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * size.width),
                rows_per_image: None,
            },
            size,
        );

        mipmap_generator.generate_mipmaps(device, queue, &texture, &desc)?;

        Ok(texture)
    }
}
