pub mod loader;

use calva_renderer::{
    wgpu::{self, util::DeviceExt},
    DrawModel, Renderer,
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

pub struct RenderInstances {
    pub transforms: Vec<(glam::Vec3, glam::Quat)>,
    pub buffer: wgpu::Buffer,
}

impl RenderInstances {
    pub const SIZE: wgpu::BufferAddress =
        (std::mem::size_of::<f32>() * (16 + 9)) as wgpu::BufferAddress;
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

    pub fn new(device: &wgpu::Device, transforms: Vec<(glam::Vec3, glam::Quat)>) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instances Buffer"),
            contents: bytemuck::cast_slice(&[0.0f32; 16 + 9]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self { transforms, buffer }
    }

    pub fn update_buffers(&self, queue: &wgpu::Queue) {
        #[repr(C)]
        #[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        pub struct InstanceRaw {
            model: [f32; 16],
            normal: [f32; 9],
        }

        let data = self
            .transforms
            .iter()
            .map(|(t, r)| {
                let translation = glam::Mat4::from_translation(*t);
                let rotation = glam::Mat4::from_quat(*r);

                let model = translation * rotation;
                let normal = glam::Mat3::from_quat(*r);

                InstanceRaw {
                    model: *model.as_ref(),
                    normal: *normal.as_ref(),
                }
            })
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
            mesh.instances.update_buffers(&renderer.queue);

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
