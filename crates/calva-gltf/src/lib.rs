pub mod loader;

use renderer::{
    wgpu::{self, util::DeviceExt},
    Camera, DrawModel, Renderer,
};

pub struct RenderPrimitive {
    pub positions_buffer: wgpu::Buffer,
    pub normals_buffer: wgpu::Buffer,
    pub tangents_buffer: wgpu::Buffer,
    pub tex_coords_0_buffer: wgpu::Buffer,
    pub indices_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct InstanceTransform {
    translation: glam::Vec3,
    rotation: glam::Quat,
    scale: glam::Vec3,
}

impl Default for InstanceTransform {
    fn default() -> Self {
        Self {
            translation: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [f32; 16],
    normal: [f32; 9],
}

impl InstanceRaw {
    fn from_transform_camera(t: &InstanceTransform, camera: &Camera) -> Self {
        let model = glam::Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation);

        let normal = (camera.view * model).inverse().transpose();

        Self {
            model: model.to_cols_array(),
            normal: glam::Mat3::from_mat4(normal).to_cols_array(),
        }
    }
}

pub struct RenderInstances {
    pub transforms: Vec<InstanceTransform>,
    pub buffer: wgpu::Buffer,
}

impl RenderInstances {
    pub const SIZE: wgpu::BufferAddress = std::mem::size_of::<InstanceRaw>() as _;
    pub const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE,
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

    pub fn new(device: &wgpu::Device) -> Self {
        // TODO: use more dynamic sized buffer?
        let data = [0u8; std::mem::size_of::<InstanceRaw>() * 10];

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instances Buffer"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            transforms: vec![
                // TODO: remove me once animations are properly loaded
                InstanceTransform::default(),
            ],
            buffer,
        }
    }

    pub fn update_buffers(&self, queue: &wgpu::Queue, camera: &Camera) {
        let data = self
            .transforms
            .iter()
            .map(|t| InstanceRaw::from_transform_camera(t, camera))
            .collect::<Vec<_>>();

        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&data));
    }

    pub fn count(&self) -> u32 {
        self.transforms.len() as u32
    }
}

pub struct RenderMesh {
    pub primitives: Vec<RenderPrimitive>,
    pub instances: RenderInstances,
}

pub struct RenderMaterial {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
}

pub struct RenderModel {
    pub meshes: Vec<RenderMesh>,
    pub materials: Vec<RenderMaterial>,
}

impl DrawModel for RenderModel {
    fn draw<'ctx: 'pass, 'pass>(
        &'ctx self,
        renderer: &'ctx Renderer,
        rpass: &mut wgpu::RenderPass<'pass>,
    ) {
        for mesh in &self.meshes {
            mesh.instances
                .update_buffers(&renderer.queue, &renderer.camera);

            for primitive in &mesh.primitives {
                let material = &self.materials[primitive.material];

                rpass.set_pipeline(&material.pipeline);

                rpass.set_bind_group(0, &renderer.camera.bind_group, &[]);
                rpass.set_bind_group(1, &material.bind_group, &[]);

                rpass.set_vertex_buffer(0, mesh.instances.buffer.slice(..));
                rpass.set_vertex_buffer(1, primitive.positions_buffer.slice(..));
                rpass.set_vertex_buffer(2, primitive.normals_buffer.slice(..));
                rpass.set_vertex_buffer(3, primitive.tangents_buffer.slice(..));
                rpass.set_vertex_buffer(4, primitive.tex_coords_0_buffer.slice(..));

                rpass.set_index_buffer(
                    primitive.indices_buffer.slice(..),
                    wgpu::IndexFormat::Uint16,
                );

                rpass.draw_indexed(0..primitive.num_elements, 0, 0..mesh.instances.count());
            }
        }
    }
}
