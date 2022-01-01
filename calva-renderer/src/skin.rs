use std::collections::HashMap;
use wgpu::util::DeviceExt;

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
    pub const DESC: &'static wgpu::BindGroupLayoutDescriptor<'static> =
        &wgpu::BindGroupLayoutDescriptor {
            label: Some("Animation bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            }],
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
                label: Some("Animation texture"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Animation bind group"),
            layout: &device.create_bind_group_layout(Self::DESC),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });

        Self {
            animations,
            bind_group,
        }
    }
}
