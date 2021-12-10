use renderer::{
    wgpu::{self},
    DrawModel, Renderer,
};

pub mod loader;

pub struct RenderPrimitive {
    pub positions_buffer: wgpu::Buffer,
    pub normals_buffer: wgpu::Buffer,
    pub tangents_buffer: wgpu::Buffer,
    pub tex_coords_0_buffer: wgpu::Buffer,
    pub indices_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

impl renderer::MeshPrimitive for RenderPrimitive {
    fn vertices(&self) -> &wgpu::Buffer {
        &self.positions_buffer
    }

    fn indices(&self) -> &wgpu::Buffer {
        &self.indices_buffer
    }

    fn num_elements(&self) -> u32 {
        self.num_elements
    }
}

pub struct RenderMesh {
    pub primitives: Vec<RenderPrimitive>,
    pub instances: renderer::MeshInstances,
}

impl renderer::Mesh for RenderMesh {
    fn instances(&self) -> &renderer::MeshInstances {
        &self.instances
    }

    fn primitives(&self) -> Box<dyn Iterator<Item = &dyn renderer::MeshPrimitive> + '_> {
        Box::new(
            self.primitives
                .iter()
                .map(|p| p as &dyn renderer::MeshPrimitive),
        )
    }
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
    fn meshes(&self) -> Box<dyn Iterator<Item = &dyn renderer::Mesh> + '_> {
        Box::new(self.meshes.iter().map(|m| m as _))
    }

    fn draw<'s: 'p, 'r: 'p, 'p>(
        &'s self,
        renderer: &'r Renderer,
        rpass: &mut wgpu::RenderPass<'p>,
    ) {
        for mesh in &self.meshes {
            mesh.instances
                .write_buffer(&renderer.queue, &renderer.camera);

            for primitive in &mesh.primitives {
                let material = &self.materials[primitive.material];

                rpass.set_pipeline(&material.pipeline);

                rpass.set_bind_group(0, &renderer.config.bind_group, &[]);
                rpass.set_bind_group(1, &renderer.camera.bind_group, &[]);
                rpass.set_bind_group(2, &material.bind_group, &[]);

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
