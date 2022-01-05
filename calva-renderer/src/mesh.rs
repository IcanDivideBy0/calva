use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [f32; 16],
    normal: [f32; 9],
    animation_frame: u32,
}

impl InstanceRaw {
    fn new(instance: MeshInstance) -> Self {
        let MeshInstance {
            transform,
            animation_frame,
        } = instance;

        let normal_matrix = glam::Mat3::from_mat4(transform.inverse().transpose());

        Self {
            model: transform.to_cols_array(),
            normal: normal_matrix.to_cols_array(),
            animation_frame,
        }
    }
}

#[derive(Clone, Copy)]
pub struct MeshInstance {
    pub transform: glam::Mat4,
    pub animation_frame: u32,
}

pub struct MeshInstances {
    pub instances: Vec<MeshInstance>,
    pub buffer: wgpu::Buffer,
}

impl MeshInstances {
    pub const SIZE: usize = std::mem::size_of::<InstanceRaw>();

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE as _,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            // Model matrix
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4,
            3 => Float32x4,

            // Normal matrix
            4 => Float32x3,
            5 => Float32x3,
            6 => Float32x3,

            // Animation frame
            7 => Uint32,
        ],
    };

    pub fn new(device: &wgpu::Device, instances: Vec<MeshInstance>) -> Self {
        let data = [0u8; MeshInstances::SIZE * 100];

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MeshInstances Buffer"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self { instances, buffer }
    }

    pub fn iter(&self) -> impl Iterator<Item = &MeshInstance> {
        self.instances.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut MeshInstance> {
        self.instances.iter_mut()
    }

    pub fn count(&self) -> u32 {
        self.instances.len() as u32
    }

    pub fn write_buffer(&self, queue: &wgpu::Queue) {
        let data = self
            .instances
            .iter()
            .map(|instance| InstanceRaw::new(*instance))
            .collect::<Vec<_>>();

        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&data));
    }
}

#[derive(Debug)]
pub struct Mesh {
    pub vertices: wgpu::Buffer,
    pub normals: wgpu::Buffer,
    pub tangents: wgpu::Buffer,
    pub uv0: wgpu::Buffer,
    pub indices: wgpu::Buffer,

    pub num_elements: u32,
}
