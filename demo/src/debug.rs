use bytemuck::NoUninit;
use wesl::syntax::*;

use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    Camera, GeometryPassOutputs, RenderContext, ResourcesManager, UniformBuffer,
};

pub struct Debug {
    resources: ResourcesManager,

    vertices: wgpu::Buffer,
    vertices_count: u32,
    pipeline: wgpu::RenderPipeline,
}

impl Debug {
    pub fn new<A: NoUninit>(resources: &ResourcesManager, triangles: &[A]) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();
        let camera = resources.read::<UniformBuffer<Camera>>();
        let geometry_outputs = resources.read::<GeometryPassOutputs>();

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Debug vertices"),
            contents: bytemuck::cast_slice(triangles),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let vertices_count = triangles.len() as u32 * 3;

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Debug pipeline layout"),
            bind_group_layouts: &[Some(&camera.bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Debug shader"),
            source: wgpu::ShaderSource::Wgsl(
                wesl_quote::quote_module! {
                    struct Camera {
                        view: mat4x4<f32>,
                        proj: mat4x4<f32>,
                        view_proj: mat4x4<f32>,
                        inv_view: mat4x4<f32>,
                        inv_proj: mat4x4<f32>,
                        frustum: array<vec4<f32>, 6>,
                    }
                    @group(0) @binding(0) var<uniform> camera: Camera;

                    @vertex
                    fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
                        return camera.view_proj * vec4<f32>(pos, 1.0);
                    }

                    @fragment
                    fn fs_main() -> @location(0) vec4<f32> {
                        return vec4<f32>(0.0, 1.0, 0.0, 0.2);
                    }
                }
                .to_string()
                .into(),
            ),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug render pipeline"),
            layout: Some(&pipeline_layout),
            multiview_mask: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 3]>() as _,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                // polygon_mode: wgpu::PolygonMode::Line,
                polygon_mode: wgpu::PolygonMode::Fill,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: geometry_outputs.depth.format(),
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: -10,
                    ..Default::default()
                },
            }),
            multisample: Default::default(),
            cache: None,
        });

        Self {
            resources,

            vertices,
            vertices_count,
            pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        if self.vertices_count == 0 {
            return;
        }

        let camera = self.resources.read::<UniformBuffer<Camera>>();
        let geometry_outputs = self.resources.read::<GeometryPassOutputs>();

        let mut rpass = ctx.encoder.scoped_render_pass(
            "Debug",
            wgpu::RenderPassDescriptor {
                label: Some("Debug"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.frame,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &geometry_outputs.depth_view,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                ..Default::default()
            },
        );

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &camera.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.vertices.slice(..));

        rpass.draw(0..self.vertices_count, 0..1);
    }
}
