use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshId(u32);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshBoundingSphere {
    center: [f32; 3],
    radius: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshData {
    bounding_sphere: MeshBoundingSphere,
    vertex_count: u32,
    vertex_offset: i32,
    base_index: u32,

    _padding: u32,
}

impl MeshData {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;
}

pub struct MeshesManager {
    vertex_offset: AtomicI32,
    base_index: AtomicU32,
    mesh_index: AtomicU32,

    pub vertices: wgpu::Buffer,
    pub normals: wgpu::Buffer,
    pub tangents: wgpu::Buffer,
    pub tex_coords0: wgpu::Buffer,
    pub indices: wgpu::Buffer,

    pub meshes: wgpu::Buffer,
}

impl MeshesManager {
    pub const VERTEX_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 3]>() as _;
    pub const NORMAL_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 3]>() as _;
    pub const TANGENT_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 4]>() as _;
    pub const TEX_COORD_SIZE: wgpu::BufferAddress = std::mem::size_of::<[f32; 2]>() as _;
    pub const INDEX_SIZE: wgpu::BufferAddress = std::mem::size_of::<u32>() as _;

    pub const MAX_MESHES: usize = 100;
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

        let meshes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshesManager meshes data"),
            size: MeshData::SIZE * (Self::MAX_MESHES as wgpu::BufferAddress),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            vertex_offset: AtomicI32::new(0),
            base_index: AtomicU32::new(0),
            mesh_index: AtomicU32::new(0),

            vertices,
            normals,
            tangents,
            tex_coords0,
            indices,

            meshes,
        }
    }

    pub fn count(&self) -> u32 {
        self.mesh_index.load(Ordering::Relaxed)
    }

    pub fn add(
        &self,
        queue: &wgpu::Queue,
        bounding_sphere: (glam::Vec3, f32),
        vertices: &[u8],
        normals: &[u8],
        tangents: &[u8],
        tex_coords0: &[u8],
        indices: &[u8],
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

        let mesh_index = self.mesh_index.fetch_add(1, Ordering::Relaxed);
        let mesh_data = MeshData {
            bounding_sphere: MeshBoundingSphere {
                center: bounding_sphere.0.to_array(),
                radius: bounding_sphere.1,
            },
            vertex_count,
            vertex_offset,
            base_index,

            _padding: 0,
        };

        queue.write_buffer(
            &self.meshes,
            mesh_index as wgpu::BufferAddress * MeshData::SIZE,
            bytemuck::bytes_of(&mesh_data),
        );

        MeshId(mesh_index)
    }
}
