use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use crate::SkinIndex;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshId(pub(crate) u32);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeshBoundingSphere {
    center: [f32; 3],
    radius: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshData {
    pub(crate) vertex_count: u32,
    pub(crate) base_index: u32,
    pub(crate) vertex_offset: i32,
    pub(crate) skin_offset: i32,
    pub(crate) bounding_sphere: MeshBoundingSphere,
}
impl MeshData {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    pub(crate) fn address(mesh_id: MeshId) -> wgpu::BufferAddress {
        Self::SIZE * (mesh_id.0 as wgpu::BufferAddress)
    }
}

pub struct MeshesManager {
    vertex_offset: AtomicI32,
    base_index: AtomicU32,
    meshes_data: Vec<MeshData>,

    pub(crate) vertices: wgpu::Buffer,
    pub(crate) normals: wgpu::Buffer,
    pub(crate) tangents: wgpu::Buffer,
    pub(crate) tex_coords0: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
}

impl MeshesManager {
    pub const VERTEX_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 3]>() as _;
    pub const NORMAL_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 3]>() as _;
    pub const TANGENT_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 4]>() as _;
    pub const TEX_COORD_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 2]>() as _;
    pub const INDEX_SIZE: wgpu::BufferAddress = std::mem::size_of::<u32>() as _;

    pub const MAX_MESHES: usize = 1 << 12;
    pub const MAX_VERTS: usize = 1 << 22;

    pub fn new(device: &wgpu::Device) -> Self {
        let max_verts = Self::MAX_VERTS as wgpu::BufferAddress;

        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshesManager vertices"),
            size: Self::VERTEX_SIZE * max_verts,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let normals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshesManager normals"),
            size: Self::NORMAL_SIZE * max_verts,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let tangents = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshesManager tangents"),
            size: Self::TANGENT_SIZE * max_verts,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let tex_coords0 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshesManager UVs"),
            size: Self::TEX_COORD_SIZE * max_verts,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let indices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshesManager indices"),
            size: Self::INDEX_SIZE * max_verts,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            vertex_offset: AtomicI32::new(0),
            base_index: AtomicU32::new(0),
            meshes_data: Vec::with_capacity(Self::MAX_MESHES),

            vertices,
            normals,
            tangents,
            tex_coords0,
            indices,
        }
    }

    pub fn count(&self) -> usize {
        self.meshes_data.len() as _
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        queue: &wgpu::Queue,
        bounding_sphere: (glam::Vec3, f32),
        vertices: &[u8],
        normals: &[u8],
        tangents: &[u8],
        tex_coords0: &[u8],
        indices: &[u8],
        skin: Option<SkinIndex>,
    ) -> MeshId {
        let vertex_len = (vertices.len() / Self::VERTEX_SIZE as usize) as i32;
        let vertex_offset = self.vertex_offset.fetch_add(vertex_len, Ordering::Relaxed);

        queue.write_buffer(
            &self.vertices,
            vertex_offset as wgpu::BufferAddress * Self::VERTEX_SIZE,
            vertices,
        );
        queue.write_buffer(
            &self.normals,
            vertex_offset as wgpu::BufferAddress * Self::NORMAL_SIZE,
            normals,
        );
        queue.write_buffer(
            &self.tangents,
            vertex_offset as wgpu::BufferAddress * Self::TANGENT_SIZE,
            tangents,
        );
        queue.write_buffer(
            &self.tex_coords0,
            vertex_offset as wgpu::BufferAddress * Self::TEX_COORD_SIZE,
            tex_coords0,
        );

        let vertex_count = (indices.len() / Self::INDEX_SIZE as usize) as u32;
        let base_index = self.base_index.fetch_add(vertex_count, Ordering::Relaxed);

        queue.write_buffer(&self.indices, base_index as u64 * Self::INDEX_SIZE, indices);

        let skin_offset = skin
            .map(|skin_index| skin_index.as_offset(vertex_offset))
            .unwrap_or_default();

        // let mesh_index = self.indirect_draws_data.len() as u32;
        // self.indirect_draws_data.push(DrawIndexedIndirect {
        //     vertex_count,
        //     instance_count: 0,
        //     base_index,
        //     vertex_offset,
        //     base_instance: 0,
        // });

        // queue.write_buffer(
        //     &self.meshes_data,
        //     MeshData::SIZE * (mesh_index as wgpu::BufferAddress),
        //     bytemuck::bytes_of(&MeshData {
        //         skin_offset,
        //         vertex_count,
        //         base_index,
        //         vertex_offset,
        //         bounding_sphere: MeshBoundingSphere {
        //             center: bounding_sphere.0.to_array(),
        //             radius: bounding_sphere.1,
        //         },
        //     }),
        // );

        let mesh_index = self.meshes_data.len() as u32;
        self.meshes_data.push(MeshData {
            vertex_count,
            base_index,
            vertex_offset,
            skin_offset,
            bounding_sphere: MeshBoundingSphere {
                center: bounding_sphere.0.to_array(),
                radius: bounding_sphere.1,
            },
        });

        MeshId(mesh_index)
    }

    pub(crate) fn get_mesh_data(&self, mesh_id: MeshId) -> &MeshData {
        &self.meshes_data[mesh_id.0 as usize]
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &MeshData> {
        self.meshes_data.iter()
    }
}
