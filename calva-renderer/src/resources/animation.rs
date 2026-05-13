use anyhow::Result;
use wgpu::util::DeviceExt;

use crate::{
    GpuMeshInstance, MeshInstancesManager, Resource, ResourcesManager, Time, UniformBuffer,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AnimationHandle(u32);

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AnimationState {
    pub animation: AnimationHandle,
    pub time: f32,
}

impl From<AnimationHandle> for AnimationState {
    fn from(animation: AnimationHandle) -> Self {
        Self {
            animation,
            time: 0.0,
        }
    }
}

pub struct AnimationsManager {
    resources: ResourcesManager,

    views: Vec<wgpu::TextureView>,
    sampler: wgpu::Sampler,

    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,

    animate_bind_group: wgpu::BindGroup,
    animate_pipeline: wgpu::ComputePipeline,
}

impl AnimationsManager {
    // pub const SAMPLE_RATE: Duration = Duration::from_secs_f32(1.0 / 15.0);
    pub const SAMPLES_PER_SEC: f32 = 15.0;

    const MAX_ANIMATIONS: u32 = 512;

    fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();
        let time = resources.read::<UniformBuffer<Time>>();

        let mut views = Vec::with_capacity(Self::MAX_ANIMATIONS as _);

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
            mag_filter: wgpu::FilterMode::Linear,
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

        let bind_group = Self::create_bind_group(&device, &bind_group_layout, &views, &sampler);

        let animate_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("AnimationsManager[animate] bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
                                + GpuMeshInstance::SIZE,
                        ),
                    },
                    count: None,
                }],
            });

        let animate_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AnimationsManager[animate] bind group"),
            layout: &animate_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: resources
                    .read::<MeshInstancesManager>()
                    .instances
                    .as_entire_binding(),
            }],
        });

        let animate_pipeline = {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Animate shader"),
                source: wgpu::ShaderSource::Wgsl(
                    wesl::include_wesl!("resources::animation").into(),
                ),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Animate pipeline layout"),
                bind_group_layouts: &[
                    Some(&animate_bind_group_layout),
                    Some(&time.bind_group_layout),
                ],
                immediate_size: 0,
            });

            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Animate pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("maintain"),
                compilation_options: Default::default(),
                cache: None,
            })
        };

        Self {
            resources,

            views,
            sampler,

            bind_group_layout,
            bind_group,

            animate_bind_group,
            animate_pipeline,
        }
    }

    pub fn add(&mut self, animation: Vec<Vec<glam::Mat4>>) -> AnimationHandle {
        let device = self.resources.read::<wgpu::Device>();
        let queue = self.resources.read::<wgpu::Queue>();

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
                &queue,
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
                wgpu::util::TextureDataOrder::LayerMajor,
                bytemuck::cast_slice(&pixels),
            )
            .create_view(&Default::default());

        self.views.push(view);
        self.bind_group =
            Self::create_bind_group(&device, &self.bind_group_layout, &self.views, &self.sampler);

        AnimationHandle(self.views.len() as u32 - 1)
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

    fn update(&mut self) -> Result<()> {
        let device = self.resources.read::<wgpu::Device>();
        let queue = self.resources.read::<wgpu::Queue>();
        let time = self.resources.read::<UniformBuffer<Time>>();
        let mesh_instances = self.resources.read::<MeshInstancesManager>();

        let mut encoder = device.create_command_encoder(&Default::default());

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("AnimationManager[update]"),
            ..Default::default()
        });

        cpass.set_pipeline(&self.animate_pipeline);
        cpass.set_bind_group(0, &self.animate_bind_group, &[]);
        cpass.set_bind_group(1, &time.bind_group, &[]);

        const WORKGROUP_SIZE: usize = 256;
        let workgroups_count =
            (mesh_instances.count() as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

        cpass.dispatch_workgroups(workgroups_count, 1, 1);

        drop(cpass);

        let submission_index = queue.submit(std::iter::once(encoder.finish()));
        device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission_index),
            timeout: None,
        })?;

        Ok(())
    }
}

impl Resource for AnimationsManager {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        Ok(Self::new(resources))
    }

    fn update(&mut self, _resources: &ResourcesManager) -> Result<()> {
        self.update()
    }
}
