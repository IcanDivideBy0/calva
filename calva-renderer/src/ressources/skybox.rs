use wgpu::util::DeviceExt;

use crate::Ressource;

pub struct SkyboxManager {
    sampler: wgpu::Sampler,

    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: Option<wgpu::BindGroup>,
}

impl SkyboxManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Skybox sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skybox bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        Self {
            sampler,

            bind_group_layout,
            bind_group: None,
        }
    }

    pub fn set_skybox(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, pixels: &[u8]) {
        let size = (pixels.len() as f32 / (4.0 * 6.0)).sqrt() as _;

        let view = device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("Skybox texture"),
                    size: wgpu::Extent3d {
                        width: size,
                        height: size,
                        depth_or_array_layers: 6,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
                },
                pixels,
            )
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Skybox texture view"),
                dimension: Some(wgpu::TextureViewDimension::Cube),
                array_layer_count: std::num::NonZeroU32::new(6),
                ..Default::default()
            });

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
    }
}

impl Ressource for SkyboxManager {
    fn instanciate(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
