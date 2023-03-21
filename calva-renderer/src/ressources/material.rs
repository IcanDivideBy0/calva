use std::sync::atomic::{AtomicU32, Ordering};

use super::TextureId;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialId(u32);

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Material {
    pub albedo: TextureId,
    pub normal: TextureId,
    pub metallic_roughness: TextureId,
    pub emissive: TextureId,
}

pub struct MaterialsManager {
    material_index: AtomicU32,
    buffer: wgpu::Buffer,

    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl MaterialsManager {
    const MAX_MATERIALS: usize = 256;

    pub fn new(device: &wgpu::Device) -> Self {
        use wgpu::util::DeviceExt;

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MaterialManager buffer"),
            contents: bytemuck::cast_slice(&[Material::default(); Self::MAX_MATERIALS]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialManager bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Material>() as _),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialManager bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            material_index: AtomicU32::new(1),
            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn add(&self, queue: &wgpu::Queue, material: Material) -> MaterialId {
        let index = self.material_index.fetch_add(1, Ordering::Relaxed);
        let offset =
            index as wgpu::BufferAddress * std::mem::size_of::<Material>() as wgpu::BufferAddress;

        queue.write_buffer(&self.buffer, offset, bytemuck::bytes_of(&material));

        MaterialId(index)
    }
}
