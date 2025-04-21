use std::sync::atomic::{AtomicU32, Ordering};

use crate::MeshesManager;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinIndex(u32);

impl SkinIndex {
    pub(crate) fn as_offset(&self, vertex_offset: i32) -> i32 {
        self.0 as i32 - vertex_offset
    }
}

pub struct SkinsManager {
    offset: AtomicU32,
    joints: wgpu::Buffer,
    weights: wgpu::Buffer,

    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl SkinsManager {
    pub const JOINTS_SIZE: wgpu::BufferAddress = std::mem::size_of::<[u8; 4]>() as _;
    pub const WEIGHTS_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 4]>() as _;

    pub fn new(device: &wgpu::Device) -> Self {
        let max_verts: wgpu::BufferAddress = MeshesManager::MAX_VERTS as wgpu::BufferAddress;

        let joints = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SkinsManager joints"),
            size: Self::JOINTS_SIZE * max_verts,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let weights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SkinsManager weights"),
            size: Self::WEIGHTS_SIZE * max_verts,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SkinsManager bind group layout"),
            entries: &[
                // Joints
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(Self::JOINTS_SIZE),
                    },
                    count: None,
                },
                // Weights
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(Self::WEIGHTS_SIZE),
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SkinsManager bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: joints.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: weights.as_entire_binding(),
                },
            ],
        });

        Self {
            offset: AtomicU32::new(1),
            joints,
            weights,

            bind_group_layout,
            bind_group,
        }
    }

    pub fn add(&mut self, queue: &wgpu::Queue, joints: &[u8], weights: &[u8]) -> SkinIndex {
        let size = (joints.len() / Self::JOINTS_SIZE as usize) as u32;
        let offset = self.offset.fetch_add(size, Ordering::Relaxed);

        queue.write_buffer(
            &self.joints,
            offset as wgpu::BufferAddress * Self::JOINTS_SIZE,
            joints,
        );

        queue.write_buffer(
            &self.weights,
            offset as wgpu::BufferAddress * Self::WEIGHTS_SIZE,
            weights,
        );

        SkinIndex(offset)
    }
}

impl From<&wgpu::Device> for SkinsManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
