use std::mem::MaybeUninit;
use std::sync::Once;

static ONCE: Once = Once::new();

static mut BIND_GROUP_LAYOUT: MaybeUninit<wgpu::BindGroupLayout> =
    MaybeUninit::<wgpu::BindGroupLayout>::uninit();

pub struct Material {
    pub bind_group: wgpu::BindGroup,
}

impl Material {
    const DESC: wgpu::BindGroupLayoutDescriptor<'static> = wgpu::BindGroupLayoutDescriptor {
        label: Some("Geometry bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
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
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    };

    pub(crate) fn bind_group_layout(device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        unsafe {
            ONCE.call_once(|| {
                BIND_GROUP_LAYOUT.write(device.create_bind_group_layout(&Self::DESC));
            });

            BIND_GROUP_LAYOUT.assume_init_ref()
        }
    }

    pub fn new(
        device: &wgpu::Device,
        albedo: &wgpu::TextureView,
        normal: &wgpu::TextureView,
        metallic_roughness: &wgpu::TextureView,
    ) -> Self {
        let sampler_desc = wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            anisotropy_clamp: std::num::NonZeroU8::new(4),
            ..Default::default()
        };

        let albedo_sampler = device.create_sampler(&sampler_desc);
        let normal_sampler = device.create_sampler(&sampler_desc);
        let metallic_roughness_sampler = device.create_sampler(&sampler_desc);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material bind group"),
            layout: Self::bind_group_layout(device),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(albedo),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&albedo_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(normal),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&normal_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(metallic_roughness),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&metallic_roughness_sampler),
                },
            ],
        });

        Self { bind_group }
    }
}
