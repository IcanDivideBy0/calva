use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AnimationId(u32);

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AnimationState {
    pub animation: AnimationId,
    pub time: f32,
}

impl From<AnimationId> for AnimationState {
    fn from(animation: AnimationId) -> Self {
        Self {
            animation,
            time: 0.0,
        }
    }
}

pub struct AnimationsManager {
    views: Vec<wgpu::TextureView>,
    sampler: wgpu::Sampler,

    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl AnimationsManager {
    // pub const SAMPLE_RATE: Duration = Duration::from_secs_f32(1.0 / 15.0);
    pub const SAMPLES_PER_SEC: f32 = 15.0;

    const MAX_ANIMATIONS: usize = 64;

    pub fn new(device: &wgpu::Device) -> Self {
        let mut views = Vec::with_capacity(Self::MAX_ANIMATIONS);

        views.push(
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("AnimationsManager null texture"),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 4,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::Rgba32Float],
                })
                .create_view(&Default::default()),
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("AnimationsManager sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("AnimationsManager bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: core::num::NonZeroU32::new(Self::MAX_ANIMATIONS as _),
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = Self::create_bind_group(device, &bind_group_layout, &views, &sampler);

        Self {
            views,
            sampler,

            bind_group_layout,
            bind_group,
        }
    }

    pub fn add(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        animation: Vec<Vec<glam::Mat4>>,
    ) -> AnimationId {
        let pixels = (0..4)
            .flat_map(|i| {
                animation
                    .iter()
                    .flatten()
                    .map(move |joint_transform| joint_transform.col(i))
            })
            .collect::<Vec<_>>();

        let view = device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("Animations texture"),
                    size: wgpu::Extent3d {
                        width: animation[0].len() as _,
                        height: animation.len() as _,
                        depth_or_array_layers: 4,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::Rgba32Float],
                },
                bytemuck::cast_slice(&pixels),
            )
            .create_view(&Default::default());

        self.views.push(view);
        self.bind_group =
            Self::create_bind_group(device, &self.bind_group_layout, &self.views, &self.sampler);
        AnimationId(self.views.len() as u32 - 1)
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        views: &[wgpu::TextureView],
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AnimationsManager bind group"),
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
