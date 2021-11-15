use crate::{Mesh, Renderer};

pub struct Material {
    pub pipeline: wgpu::RenderPipeline,
}

impl Material {
    pub fn new(renderer: &Renderer, name: &str) -> Self {
        let shader = renderer
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some(&format!("Material Shader: {}", name)),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/gbuffer.wgsl").into()),
            });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(&format!("Material Render Pipeline Layout: {}", name)),
                    bind_group_layouts: &[&renderer.camera_uniforms.bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("Material Render Pipeline: {}", name)),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[
                        Mesh::DESC,
                        wgpu::VertexBufferLayout {
                            array_stride: (std::mem::size_of::<glam::Vec3>())
                                as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![
                                4 => Float32x3,
                            ],
                        },
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "main",
                    targets: &[
                        wgpu::ColorTargetState {
                            format: renderer.gbuffer.albedo.format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        },
                        wgpu::ColorTargetState {
                            format: renderer.gbuffer.position.format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        },
                        wgpu::ColorTargetState {
                            format: renderer.gbuffer.normal.format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        },
                    ],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    clamp_depth: false,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: renderer.gbuffer.depth.format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
            });

        Self { pipeline }
    }
}
