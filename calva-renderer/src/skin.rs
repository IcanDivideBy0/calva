use std::collections::HashMap;
use std::time::Duration;
use wgpu::util::DeviceExt;

use crate::Instance;

#[derive(Debug)]
pub struct Skin {
    pub joint_indices: wgpu::Buffer,
    pub joint_weights: wgpu::Buffer,
}

pub type SkinAnimationFrame = Vec<glam::Mat4>;
pub type SkinAnimation = Vec<SkinAnimationFrame>;

pub struct SkinAnimations {
    pub animations: HashMap<String, (u32, u32)>,
    pub bind_group: wgpu::BindGroup,
}

impl SkinAnimations {
    // pub const SAMPLE_RATE: Duration = Duration::from_secs_f32(1.0 / 60.0);

    pub fn sample_rate() -> Duration {
        Duration::from_secs_f32(1.0 / 60.0)
    }

    pub const DESC: &'static wgpu::BindGroupLayoutDescriptor<'static> =
        &wgpu::BindGroupLayoutDescriptor {
            label: Some("Animations bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        };

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut animations: HashMap<String, SkinAnimation>,
    ) -> Self {
        let mut layers = vec![vec![]; 4];

        let mut size = wgpu::Extent3d {
            width: 0,
            height: 0,
            depth_or_array_layers: 4,
        };

        let animations = animations
            .drain()
            .map(|(name, animation)| {
                for (i, layer) in layers.iter_mut().enumerate() {
                    for frame in &animation {
                        size.width = frame.len() as u32;

                        for joint_transform in frame {
                            layer.push(joint_transform.col(i));
                        }
                    }
                }

                let result = (name, (size.height, animation.len() as u32));
                size.height += animation.len() as u32;

                result
            })
            .collect::<HashMap<_, _>>();

        let pixels = layers.drain(..).flatten().collect::<Vec<_>>();

        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Animations texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
            },
            bytemuck::cast_slice(&pixels),
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Animations buffer"),
            contents: bytemuck::cast_slice(
                &animations
                    .values()
                    .map(|(offset, length)| [*offset, *length])
                    .collect::<Vec<_>>(),
            ),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Animations bind group"),
            layout: &device.create_bind_group_layout(Self::DESC),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            animations,
            bind_group,
        }
    }

    pub fn get_frame(&self, name: &str, t: Duration, looping: bool) -> Option<u32> {
        let (offset, length) = self.animations.get(name)?;

        let mut frame_index = (t.as_secs_f32() / Self::sample_rate().as_secs_f32()) as u32;

        frame_index = if looping {
            frame_index % length
        } else {
            frame_index.min(*length)
        };

        Some(offset + frame_index)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinAnimationInstance {
    pub frame: u32,
}

impl Instance for SkinAnimationInstance {
    const SIZE: usize = std::mem::size_of::<Self>();

    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE as _,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![5 => Uint32],
    };
}

pub type SkinAnimationInstances = crate::Instances<SkinAnimationInstance>;
