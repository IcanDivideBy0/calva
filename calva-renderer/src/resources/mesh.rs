use crate::{util::id_generator::IdGenerator, SkinHandle};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshHandle(u16);

impl From<MeshHandle> for u16 {
    fn from(value: MeshHandle) -> u16 {
        value.0
    }
}
impl From<MeshHandle> for usize {
    fn from(value: MeshHandle) -> usize {
        value.0 as _
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeshBoundingSphere {
    center: [f32; 3],
    radius: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeshInfo {
    vertex_count: u32,
    base_index: u32,
    vertex_offset: i32,
    skin_offset: i32,
    bounding_sphere: MeshBoundingSphere,
}
impl MeshInfo {
    pub(crate) const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    fn address(handle: &MeshHandle) -> wgpu::BufferAddress {
        handle.0 as wgpu::BufferAddress * Self::SIZE
    }
}

pub struct MeshesManager {
    vertex_offset: i32,
    base_index: u32,
    ids: IdGenerator,

    pub(crate) meshes_info: wgpu::Buffer,

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

    pub const MAX_MESHES: usize = 1 << 16; // see MeshHandle
    pub const MAX_VERTS: usize = 1 << 22;

    pub fn new(device: &wgpu::Device) -> Self {
        let max_verts = Self::MAX_VERTS as wgpu::BufferAddress;

        let meshes_info = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MeshesManager meshes info"),
            size: std::mem::size_of::<[MeshInfo; Self::MAX_MESHES]>() as _,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
            vertex_offset: 0,
            base_index: 0,
            ids: IdGenerator::new(0),

            meshes_info,

            vertices,
            normals,
            tangents,
            tex_coords0,
            indices,
        }
    }

    pub fn count(&self) -> u16 {
        self.ids.next
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
        skin: Option<SkinHandle>,
    ) -> MeshHandle {
        let handle = MeshHandle(self.ids.get());

        queue.write_buffer(
            &self.vertices,
            self.vertex_offset as wgpu::BufferAddress * Self::VERTEX_SIZE,
            vertices,
        );
        queue.write_buffer(
            &self.normals,
            self.vertex_offset as wgpu::BufferAddress * Self::NORMAL_SIZE,
            normals,
        );
        queue.write_buffer(
            &self.tangents,
            self.vertex_offset as wgpu::BufferAddress * Self::TANGENT_SIZE,
            tangents,
        );
        queue.write_buffer(
            &self.tex_coords0,
            self.vertex_offset as wgpu::BufferAddress * Self::TEX_COORD_SIZE,
            tex_coords0,
        );

        queue.write_buffer(
            &self.indices,
            self.base_index as wgpu::BufferAddress * Self::INDEX_SIZE,
            indices,
        );

        let vertex_len = (vertices.len() as i32) / (Self::VERTEX_SIZE as i32);
        let vertex_count = (indices.len() as u32) / (Self::INDEX_SIZE as u32);

        queue.write_buffer(
            &self.meshes_info,
            MeshInfo::address(&handle),
            bytemuck::bytes_of(&MeshInfo {
                vertex_count,
                base_index: self.base_index,
                vertex_offset: self.vertex_offset,
                skin_offset: skin
                    .map(|skin_handle| skin_handle.as_offset(self.vertex_offset))
                    .unwrap_or_default(),
                bounding_sphere: MeshBoundingSphere {
                    center: bounding_sphere.0.to_array(),
                    radius: bounding_sphere.1,
                },
            }),
        );

        self.vertex_offset += vertex_len;
        self.base_index += vertex_count;

        handle
    }
}

impl From<&wgpu::Device> for MeshesManager {
    fn from(device: &wgpu::Device) -> Self {
        Self::new(device)
    }
}
