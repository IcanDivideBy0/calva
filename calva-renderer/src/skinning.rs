use wgpu::util::DeviceExt;

#[derive(Debug)]
pub struct Skin {
    pub joint_indices: wgpu::Buffer,
    pub joint_weights: wgpu::Buffer,
}

pub struct SkinAnimation {
    // pub texture: wgpu::Texture,
    pub data: Vec<Vec<glam::Mat4>>,

    pub bind_group: wgpu::BindGroup,
}

use std::mem::MaybeUninit;
use std::sync::Once;

static ONCE: Once = Once::new();

static mut BIND_GROUP_LAYOUT: MaybeUninit<wgpu::BindGroupLayout> =
    MaybeUninit::<wgpu::BindGroupLayout>::uninit();

impl SkinAnimation {
    const DESC: wgpu::BindGroupLayoutDescriptor<'static> = wgpu::BindGroupLayoutDescriptor {
        label: Some("Animation bind group layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    };

    pub(crate) fn bind_group_layout(device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        unsafe {
            ONCE.call_once(|| {
                BIND_GROUP_LAYOUT.write(device.create_bind_group_layout(&Self::DESC));
            });

            BIND_GROUP_LAYOUT.assume_init_ref()
        }
    }

    pub fn new(device: &wgpu::Device, data: Vec<Vec<glam::Mat4>>) -> Self {
        let mut buf_data = vec![glam::Mat4::IDENTITY; 100];
        for (i, m) in data[0].iter().enumerate() {
            buf_data[i] = *m;
        }

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Animation buffer"),
            contents: bytemuck::cast_slice(&buf_data),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Animation bind group"),
            layout: Self::bind_group_layout(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self { data, bind_group }
    }
}
