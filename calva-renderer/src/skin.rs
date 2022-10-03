use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use crate::MeshesManager;

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinId(u32);

pub struct SkinsManager {
    vertex_offset: AtomicI32,
    skin_index: AtomicU32,

    pub joints: wgpu::Buffer,
    pub weights: wgpu::Buffer,
}

impl SkinsManager {
    pub const JOINTS_SIZE: wgpu::BufferAddress = std::mem::size_of::<[u8; 4]>() as _;
    pub const WEIGHTS_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 4]>() as _;

    pub fn new(device: &wgpu::Device) -> Self {
        let max_verts: wgpu::BufferAddress = MeshesManager::MAX_VERTS as wgpu::BufferAddress;

        let joints = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SkinsManager joints"),
            size: Self::JOINTS_SIZE * max_verts,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let weights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SkinsManager weights"),
            size: Self::WEIGHTS_SIZE * max_verts,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            vertex_offset: AtomicI32::new(0),
            skin_index: AtomicU32::new(0),
            joints,
            weights,
        }
    }

    pub fn add(&self, queue: &wgpu::Queue, joints: &[u8], weights: &[u8]) -> SkinId {
        let vertex_len = (joints.len() / Self::JOINTS_SIZE as usize) as i32;
        let vertex_offset = self.vertex_offset.fetch_add(vertex_len, Ordering::Relaxed);

        queue.write_buffer(
            &self.joints,
            vertex_offset as wgpu::BufferAddress * Self::JOINTS_SIZE,
            joints,
        );

        queue.write_buffer(
            &self.weights,
            vertex_offset as wgpu::BufferAddress * Self::WEIGHTS_SIZE,
            weights,
        );

        let skin_index = self.skin_index.fetch_add(1, Ordering::Relaxed);
        SkinId(skin_index)
    }
}
