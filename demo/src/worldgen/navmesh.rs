use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    CameraManager, RenderContext,
};
use glam::Vec3Swizzles;

use super::tile::Tile;

pub struct NavMesh {
    triangles: Vec<[glam::Vec3; 3]>,
}

impl NavMesh {
    pub fn new(tile: &Tile) -> Self {
        let get_height = |x: i32, y: i32| {
            let y = y.max(0).min(Tile::TEXTURE_SIZE as i32 - 1) as usize;
            let x = x.max(0).min(Tile::TEXTURE_SIZE as i32 - 1) as usize;

            tile.height_map[y][x]
        };

        let triangles = (0..Tile::TEXTURE_SIZE as i32)
            .flat_map(|y| {
                (0..Tile::TEXTURE_SIZE as i32)
                    .filter_map(move |x| {
                        const MAX_STEP: f32 = 0.5;

                        let height = get_height(x, y);

                        if height < 0.0 {
                            return None;
                        }

                        let mut accept = 0;
                        let r: i32 = 3;
                        for yy in -r..=r {
                            for xx in -r..=r {
                                if (get_height(x + xx, y + yy) - height).abs() > MAX_STEP {
                                    continue;
                                }

                                accept += 1;
                            }
                        }

                        let threshold = (2 * r + 1) * (r + 1);
                        if accept < threshold {
                            return None;
                        }

                        let mut c = 0;
                        for xx in -r..=r {
                            let a = get_height(x + xx, y);
                            let b = height; // get_height(x + xx - xx.signum(), y);
                            if (a - b).abs() < MAX_STEP {
                                c += 1;
                            }
                        }
                        if c < 2 * r - 1 {
                            return None;
                        }

                        let mut c = 0;
                        for yy in -r..=r {
                            let a = get_height(x, y + yy);
                            let b = height; // get_height(x, y + yy - yy.signum());
                            if (a - b).abs() < MAX_STEP {
                                c += 1;
                            }
                        }
                        if c < 2 * r - 1 {
                            return None;
                        }

                        let mut tl = glam::vec2(x as f32, y as f32);
                        let mut tr = tl + glam::Vec2::X;
                        let mut bl = tl + glam::Vec2::Y;
                        let mut br = bl + glam::Vec2::X;

                        tl = tl * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0;
                        tr = tr * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0;
                        bl = bl * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0;
                        br = br * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0;

                        let tlh = height
                            .max(get_height(x - 1, y - 1))
                            .max(get_height(x, y - 1))
                            .max(get_height(x - 1, y));
                        let trh = height
                            .max(get_height(x + 1, y - 1))
                            .max(get_height(x, y - 1))
                            .max(get_height(x + 1, y));
                        let blh = height
                            .max(get_height(x - 1, y + 1))
                            .max(get_height(x, y + 1))
                            .max(get_height(x - 1, y));
                        let brh = height
                            .max(get_height(x + 1, y + 1))
                            .max(get_height(x, y + 1))
                            .max(get_height(x + 1, y));

                        let tlh = ((tlh - height).abs() < MAX_STEP).then_some(tlh);
                        let trh = ((trh - height).abs() < MAX_STEP).then_some(trh);
                        let blh = ((blh - height).abs() < MAX_STEP).then_some(blh);
                        let brh = ((brh - height).abs() < MAX_STEP).then_some(brh);
                        // let tlh = Some(tlh);
                        // let trh = Some(trh);
                        // let blh = Some(blh);
                        // let brh = Some(brh);

                        let tl = tlh.map(|tlh| tl.extend(tlh).xzy());
                        let tr = trh.map(|trh| tr.extend(trh).xzy());
                        let bl = blh.map(|blh| bl.extend(blh).xzy());
                        let br = brh.map(|brh| br.extend(brh).xzy());

                        let corners = [tl, tr, bl, br]
                            .iter()
                            .filter_map(|v| *v)
                            .collect::<Vec<_>>();

                        match corners.len() {
                            4 => {
                                let diag1 = corners[1] - corners[2];
                                let diag2 = corners[3] - corners[0];

                                if diag1.dot(glam::Vec3::Y).abs() > diag2.dot(glam::Vec3::Y).abs() {
                                    Some(vec![
                                        [corners[3], corners[1], corners[0]],
                                        [corners[0], corners[2], corners[3]],
                                    ])
                                } else {
                                    Some(vec![
                                        [corners[1], corners[2], corners[3]],
                                        [corners[2], corners[1], corners[0]],
                                    ])
                                }
                            }
                            3 => Some(vec![[corners[0], corners[1], corners[2]]]),
                            _ => None,
                        }
                    })
                    .flatten()
            })
            .collect::<Vec<_>>();

        Self { triangles }
    }
}

pub struct NavMeshDebugInput<'a> {
    pub depth: &'a wgpu::Texture,
}

pub struct NavMeshDebug {
    depth_view: wgpu::TextureView,

    vertices: wgpu::Buffer,
    vertices_count: u32,
    pipeline: wgpu::RenderPipeline,
}

impl NavMeshDebug {
    pub fn new(
        device: &wgpu::Device,
        camera: &CameraManager,
        navmesh: &NavMesh,
        format: wgpu::TextureFormat,
        input: NavMeshDebugInput,
    ) -> Self {
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("NavMeshDebug vertices"),
            contents: bytemuck::cast_slice(&navmesh.triangles),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let vertices_count = navmesh.triangles.len() as u32 * 3;

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("NavMeshDebug pipeline layout"),
            bind_group_layouts: &[&camera.bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("NavMeshDebug shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                    struct Camera {
                        view: mat4x4<f32>,
                        proj: mat4x4<f32>,
                        view_proj: mat4x4<f32>,
                        inv_view: mat4x4<f32>,
                        inv_proj: mat4x4<f32>,
                        frustum: array<vec4<f32>, 6>,
                    }
                    @group(0) @binding(0) var<uniform> camera: Camera;

                    struct VertexOutput {
                        @builtin(position) position: vec4<f32>,
                        @location(0) color: vec4<f32>,
                    }

                    @vertex
                    fn vs_main(@location(0) pos: vec3<f32>) -> VertexOutput {
                        var out: VertexOutput;

                        out.position = camera.view_proj * vec4<f32>(pos, 1.0);

                        out.color = vec4<f32>(
                            (pos.x / 15.0) * 0.5 + 0.5,
                            (pos.z / 15.0) * 0.5 + 0.5,
                            0.0,
                            0.3,
                        );

                        return out;
                    }

                    @fragment
                    fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                        return in.color;
                    }
                "#
                .into(),
            ),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NavMeshDebug render pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
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
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                polygon_mode: wgpu::PolygonMode::Line,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
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
            cache: None,
        });

        Self {
            depth_view: input.depth.create_view(&Default::default()),

            vertices,
            vertices_count,
            pipeline,
        }
    }

    pub fn rebind(&mut self, input: NavMeshDebugInput) {
        self.depth_view = input.depth.create_view(&Default::default());
    }

    pub fn render(&self, ctx: &mut RenderContext, camera: &CameraManager) {
        let color_attachments = [Some(wgpu::RenderPassColorAttachment {
            view: ctx.frame,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("NavMeshDebug"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: None,
                stencil_ops: None,
            }),
            ..Default::default()
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &camera.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.vertices.slice(..));

        rpass.draw(0..self.vertices_count, 0..1);
    }
}
