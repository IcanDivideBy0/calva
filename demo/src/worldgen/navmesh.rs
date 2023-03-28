use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    CameraManager, RenderContext,
};

use super::tile::Tile;

pub struct NavMesh {
    points: Vec<glam::Vec3>,
}

impl NavMesh {
    pub fn new(tile: &Tile) -> Self {
        let points = (0..Tile::TEXTURE_SIZE)
            .flat_map(|y| {
                (0..Tile::TEXTURE_SIZE).filter_map(move |x| {
                    let height = tile.height_map[y][x];

                    if height < 0.0 {
                        None
                    } else {
                        Some(glam::vec3(
                            (x as f32 + 0.5) * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0,
                            height,
                            (y as f32 + 0.5) * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0,
                        ))
                    }
                })
            })
            .collect::<Vec<_>>();

        Self { points }
    }
}

pub struct NavMeshDebugInput<'a> {
    pub depth: &'a wgpu::Texture,
}

pub struct NavMeshDebug {
    depth_view: wgpu::TextureView,

    instances_count: u32,
    instances: wgpu::Buffer,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
}

impl NavMeshDebug {
    #[rustfmt::skip]
    const POSITIONS: [glam::Vec3; 4] = [
        glam::vec3(-1.0, 0.0, -1.0),
        glam::vec3( 1.0, 0.0, -1.0),
        glam::vec3( 1.0, 0.0,  1.0),
        glam::vec3(-1.0, 0.0,  1.0),
    ];

    #[rustfmt::skip]
    const INDICES: [u16; 6] = [
        0, 1, 2,
        2, 3, 0,
    ];

    const VERTICES_COUNT: u32 = Self::INDICES.len() as _;

    pub fn new(
        device: &wgpu::Device,
        camera: &CameraManager,
        navmesh: &NavMesh,
        format: wgpu::TextureFormat,
        input: NavMeshDebugInput,
    ) -> Self {
        let instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("NavMeshDebug instances"),
            contents: bytemuck::cast_slice(&navmesh.points),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("NavMeshDebug vertices"),
            contents: bytemuck::cast_slice(&Self::POSITIONS),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("NavMeshDebug indices"),
            contents: bytemuck::cast_slice(&Self::INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("NavMeshDebug pipeline layout"),
            bind_group_layouts: &[&camera.bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("NavMeshDebug shader"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    r#"
                        struct Camera {{
                            view: mat4x4<f32>,
                            proj: mat4x4<f32>,
                            view_proj: mat4x4<f32>,
                            inv_view: mat4x4<f32>,
                            inv_proj: mat4x4<f32>,
                            frustum: array<vec4<f32>, 6>,
                        }}
                        @group(0) @binding(0) var<uniform> camera: Camera;

                        struct VertexInput {{
                            @location(0) instance_position: vec3<f32>,
                            @location(1) vertex_position: vec3<f32>
                        }}

                        struct VertexOutput {{
                            @builtin(position) position: vec4<f32>,
                            @location(0) color: vec4<f32>,
                        }}

                        @vertex
                        fn vs_main(in: VertexInput) -> VertexOutput {{
                            var out: VertexOutput;

                            out.position = camera.view_proj * vec4<f32>(
                                {scale:.8} * in.vertex_position + in.instance_position,
                                1.0,
                            );

                            out.color = vec4<f32>(
                                (in.instance_position.x / 15.0) * 0.5 + 0.5,
                                (in.instance_position.z / 15.0) * 0.5 + 0.5,
                                0.0,
                                1.0,
                            );

                            return out;
                        }}

                        @fragment
                        fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {{
                            return in.color;
                        }}
                    "#,
                    scale = Tile::PIXEL_SIZE / 2.0 / 2.0
                )
                .into(),
            ),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NavMeshDebug render pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 3]>() as _,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 3]>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![1 => Float32x3],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: input.depth.format(),
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: -10,
                    ..Default::default()
                },
            }),
            multisample: Default::default(),
        });

        Self {
            depth_view: input.depth.create_view(&Default::default()),

            instances_count: navmesh.points.len() as _,
            instances,
            vertices,
            indices,
            pipeline,
        }
    }

    pub fn rebind(&mut self, input: NavMeshDebugInput) {
        self.depth_view = input.depth.create_view(&Default::default());
    }

    pub fn render(&self, ctx: &mut RenderContext, camera: &CameraManager) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("NavMeshDebug"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ctx.frame,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: None,
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &camera.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.instances.slice(..));
        rpass.set_vertex_buffer(1, self.vertices.slice(..));
        rpass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);

        rpass.draw_indexed(0..Self::VERTICES_COUNT, 0, 0..self.instances_count);
    }
}
