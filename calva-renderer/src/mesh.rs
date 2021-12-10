use wgpu::util::DeviceExt;

use crate::Camera;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [f32; 16],
    normal: [f32; 9],
}

impl From<(glam::Mat4, &Camera)> for InstanceRaw {
    fn from((model, camera): (glam::Mat4, &Camera)) -> Self {
        let normal = (camera.view * model).inverse().transpose();

        InstanceRaw {
            model: model.to_cols_array(),
            normal: glam::Mat3::from_mat4(normal).to_cols_array(),
        }
    }
}

pub struct MeshInstances {
    pub transforms: Vec<glam::Mat4>,
    pub buffer: wgpu::Buffer,
}

impl MeshInstances {
    pub const SIZE: usize = std::mem::size_of::<InstanceRaw>();

    pub const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
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
        ],
    };

    pub fn new(device: &wgpu::Device, transforms: Vec<glam::Mat4>) -> Self {
        let data = [0u8; MeshInstances::SIZE * 10];

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MeshInstances Buffer"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self { transforms, buffer }
    }

    pub fn count(&self) -> u32 {
        self.transforms.len() as u32
    }

    pub fn write_buffer(&self, queue: &wgpu::Queue, camera: &Camera) {
        let data = self
            .transforms
            .iter()
            .map(|transform| InstanceRaw::from((*transform, camera)))
            .collect::<Vec<_>>();

        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&data));
    }
}

pub trait MeshPrimitive {
    fn vertices(&self) -> &wgpu::Buffer;
    fn indices(&self) -> &wgpu::Buffer;
    fn num_elements(&self) -> u32;
}

pub trait Mesh {
    fn instances(&self) -> &MeshInstances;
    fn primitives(&self) -> Box<dyn Iterator<Item = &dyn MeshPrimitive> + '_>;
}
