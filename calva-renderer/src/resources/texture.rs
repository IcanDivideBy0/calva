use crate::{Resource, ResourcesManager};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureHandle(u32);

pub struct TexturesManager {
    resources: ResourcesManager,

    views: Vec<wgpu::TextureView>,
    sampler: wgpu::Sampler,

    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl TexturesManager {
    fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();

        let max_textures = device.limits().max_sampled_textures_per_shader_stage;
        let mut views = Vec::with_capacity(max_textures as _);

        views.push(
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("TexturesManager null texture"),
                    size: Default::default(),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::R8Unorm],
                })
                .create_view(&Default::default()),
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TexturesManager sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TexturesManager bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: core::num::NonZeroU32::new(max_textures),
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = Self::create_bind_group(&device, &bind_group_layout, &views, &sampler);

        Self {
            resources,

            views,
            sampler,

            bind_group_layout,
            bind_group,
        }
    }

    pub fn add(&mut self, view: wgpu::TextureView) -> TextureHandle {
        let device = self.resources.read::<wgpu::Device>();

        self.views.push(view);

        self.bind_group =
            Self::create_bind_group(&device, &self.bind_group_layout, &self.views, &self.sampler);

        TextureHandle(self.views.len() as u32 - 1)
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        views: &[wgpu::TextureView],
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TexturesManager bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(
                        &views.iter().collect::<Vec<_>>(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

impl Resource for TexturesManager {
    fn instanciate(resources: &ResourcesManager) -> Self {
        Self::new(resources)
    }
}
