#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Triangle(glam::Vec3, glam::Vec3, glam::Vec3);

use calva::{
    wgpu::{self, util::DeviceExt},
    RenderContext,
};

pub struct NavMesh {
    vertices: wgpu::Buffer,
    count: u32,

    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
}

impl NavMesh {
    pub fn new(
        device: &wgpu::Device,
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
        calva::gltf::traverse_nodes_tree::<glam::Mat4>(
            module1.children(),
            &mut |parent_transform, node| {
                let transform =
                    *parent_transform * glam::Mat4::from_cols_array_2d(&node.transform().matrix());

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
                                transform.transform_point3(*vertices.get(i1 as usize)?),
                                transform.transform_point3(*vertices.get(i2 as usize)?),
                                transform.transform_point3(*vertices.get(i3 as usize)?),
                            ))
                        }));
                    }
                }

                transform
            },
            glam::Mat4::IDENTITY,
        );

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("NavMesh verts buffer"),
            contents: bytemuck::cast_slice(&triangles),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let count = 3 * triangles.len() as u32;

        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("NavMesh depth"),
            size: wgpu::Extent3d {
                width: 12 * 5,
                height: 12 * 5,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Depth32Float],
        });
        let depth_view = depth.create_view(&Default::default());

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("NavMesh pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("NavMesh shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                    @vertex
                    fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
                        return vec4<f32>(
                            pos.xz / (5.0 * 3.0),
                            (20.0 - pos.y) / 30.0,
                            1.0,
                        );
                    }
                "#
                .into(),
            ),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            fragment: None,
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth.format(),
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
        });

        Self {
            vertices,
            count,

            depth,
            depth_view,
            pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut rpass = ctx.encoder.scoped_render_pass(
            "NavMesh",
            wgpu::RenderPassDescriptor {
                label: Some("NavMesh"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            },
        );

        rpass.set_pipeline(&self.pipeline);

        rpass.set_vertex_buffer(0, self.vertices.slice(..));

        rpass.draw(0..self.count, 0..1);
    }
}
