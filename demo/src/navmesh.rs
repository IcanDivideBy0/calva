#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Triangle(glam::Vec3, glam::Vec3, glam::Vec3);

use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    CameraManager, RenderContext, Renderer,
};

pub struct NavMesh {
    triangles: Vec<Triangle>,
    lines: Vec<glam::Vec3>,
    vertices: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
}

impl NavMesh {
    pub fn new(
        renderer: &Renderer,
        camera: &CameraManager,
        doc: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Self {
        let module1 = doc
            .nodes()
            .find(|node| Some("module01") == node.name())
            .unwrap();

        let get_buffer_data = |buffer: gltf::Buffer| -> Option<&[u8]> {
            buffers.get(buffer.index()).map(std::ops::Deref::deref)
        };

        let mut triangles = vec![];
        traverse_nodes_tree::<glam::Mat4>(
            module1.children(),
            &mut |parent_transform, node| {
                let local_transform = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
                let global_transform = *parent_transform * local_transform;

                if let Some(mesh) = node.mesh() {
                    for primitive in mesh.primitives() {
                        let reader = primitive.reader(get_buffer_data);

                        let vertices = reader
                            .read_positions()
                            .unwrap()
                            .map(glam::Vec3::from_array)
                            .collect::<Vec<_>>();

                        let indices = reader
                            .read_indices()
                            .unwrap()
                            .into_u32()
                            .collect::<Vec<_>>();

                        triangles.extend(indices.chunks_exact(3).filter_map(|chunk| {
                            let [i1, i2, i3] = <[u32; 3]>::try_from(chunk).ok()?;

                            Some(Triangle(
                                global_transform.transform_point3(*vertices.get(i1 as usize)?),
                                global_transform.transform_point3(*vertices.get(i2 as usize)?),
                                global_transform.transform_point3(*vertices.get(i3 as usize)?),
                            ))
                        }));
                    }
                }

                global_transform
            },
            glam::Mat4::IDENTITY,
        );

        let lines = triangles
            .iter()
            .flat_map(|&Triangle(v1, v2, v3)| [v1, v2, v2, v3, v3, v1])
            .collect::<Vec<_>>();

        let vertices = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("NavMesh verts buffer"),
                contents: bytemuck::cast_slice(&lines),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("NavMesh pipeline layout"),
                    bind_group_layouts: &[&camera.bind_group_layout],
                    push_constant_ranges: &[],
                });

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("navmesh.wgsl"));

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("NavMesh render pipeline"),
                layout: Some(&pipeline_layout),
                multiview: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 3]>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: renderer.surface_config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
            });

        Self {
            triangles,
            lines,
            vertices,
            pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext, camera: &CameraManager) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("NavMesh"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.frame,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: ctx.depth_stencil,
                depth_ops: None,
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &camera.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.vertices.slice(..));

        let count = self.lines.len() as u32;
        rpass.draw(0..count, 0..1);
    }
}

fn traverse_nodes_tree<'a, T>(
    nodes: impl Iterator<Item = gltf::Node<'a>>,
    cb: &mut dyn FnMut(&T, &gltf::Node) -> T,
    acc: T,
) {
    for node in nodes {
        let res = cb(&acc, &node);
        traverse_nodes_tree(node.children(), cb, res);
    }
}
