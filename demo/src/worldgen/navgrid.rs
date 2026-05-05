use glam::Vec3Swizzles;
use std::fmt;
use wesl::syntax::*;

use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    CameraManager, RenderContext,
};
use itertools::Itertools;

use super::tile::Tile;

pub struct NavGrid {
    pub grid: [[Option<f32>; Self::GRID_SIZE]; Self::GRID_SIZE],
}

impl NavGrid {
    const TEXTURE_GRID_RATIO: usize = 2;
    const GRID_SIZE: usize = Tile::TEXTURE_SIZE / Self::TEXTURE_GRID_RATIO;

    pub fn new(tile: &Tile) -> Self {
        let get_tex_height = |x: i32, y: i32| {
            let y = y.clamp(0, Tile::TEXTURE_SIZE as i32 - 1) as usize;
            let x = x.clamp(0, Tile::TEXTURE_SIZE as i32 - 1) as usize;

            tile.height_map[y][x]
        };

        let mut grid = [[None; Self::GRID_SIZE]; Self::GRID_SIZE];

        for (y, x) in itertools::iproduct!(0..Self::GRID_SIZE, 0..Self::GRID_SIZE) {
            let heights = itertools::iproduct!(
                -1..=Self::TEXTURE_GRID_RATIO as i32,
                -1..=Self::TEXTURE_GRID_RATIO as i32
            )
            .map(|(yy, xx)| {
                get_tex_height(
                    (x * Self::TEXTURE_GRID_RATIO) as i32 + xx,
                    (y * Self::TEXTURE_GRID_RATIO) as i32 + yy,
                )
            })
            .collect::<Vec<_>>();

            let min = heights.iter().copied().fold(f32::INFINITY, f32::min);
            let max = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);

            if max <= -Tile::MAX_HEIGHT {
                continue;
            }

            if max - min > 0.5 * Self::TEXTURE_GRID_RATIO as f32 {
                continue;
            }

            grid[y][x] = Some(1.0);
        }

        NavGrid { grid }
    }

    pub fn triangles(&self, tile: &Tile) -> Vec<[glam::Vec3; 3]> {
        let get_tex_height = |x: usize, y: usize| {
            let y = (y * Self::TEXTURE_GRID_RATIO).min(Tile::TEXTURE_SIZE - 1);
            let x = (x * Self::TEXTURE_GRID_RATIO).min(Tile::TEXTURE_SIZE - 1);

            tile.height_map[y][x]
        };

        let points: [[glam::Vec3; Self::GRID_SIZE + 1]; Self::GRID_SIZE + 1] =
            itertools::iproduct!(0..=Self::GRID_SIZE, 0..=Self::GRID_SIZE)
                .map(|(y, x)| {
                    let it = itertools::iproduct!(0..=1, 0..=1)
                        .map(|(yy, xx)| get_tex_height(x.saturating_sub(xx), y.saturating_sub(yy)));

                    let min = it.clone().fold(0.0, f32::min);

                    let height = if min > -Tile::MAX_HEIGHT {
                        it.fold(0.0, std::ops::Add::add) / 4.0
                    } else {
                        -Tile::MAX_HEIGHT
                    };

                    glam::vec3(
                        (x * Self::TEXTURE_GRID_RATIO) as f32 * Tile::PIXEL_SIZE,
                        (y * Self::TEXTURE_GRID_RATIO) as f32 * Tile::PIXEL_SIZE,
                        height,
                    )
                })
                .chunks(Self::GRID_SIZE + 1)
                .into_iter()
                .filter_map(Itertools::collect_array)
                .collect_array()
                .unwrap();

        itertools::iproduct!(0..Self::GRID_SIZE, 0..Self::GRID_SIZE)
            .flat_map(|(y, x)| {
                [
                    [points[y][x], points[y + 1][x + 1], points[y][x + 1]],
                    [points[y][x], points[y + 1][x], points[y + 1][x + 1]],
                ]
            })
            .filter(|[a, b, c]| {
                let normal = glam::Vec3::cross(b - a, a - c).normalize();
                glam::Vec3::dot(normal, glam::Vec3::Z) > 0.5
                    && a.z > -Tile::MAX_HEIGHT
                    && b.z > -Tile::MAX_HEIGHT
                    && c.z > -Tile::MAX_HEIGHT
            })
            .map(|[a, b, c]| {
                let t = |v: glam::Vec3| {
                    let tr = glam::vec3(Tile::WORLD_SIZE / 2.0, 0.0, Tile::WORLD_SIZE / 2.0);
                    v.xzy() - tr
                };

                [t(a), t(b), t(c)]
            })
            .collect()
    }
}

impl fmt::Debug for NavGrid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        dbg!(self.grid[13][40]);

        for [row_up, row_down] in self.grid.as_chunks::<2>().0 {
            for cells in std::iter::zip(row_up, row_down) {
                write!(
                    f,
                    "{}",
                    match cells {
                        (Some(_), Some(_)) => '█',
                        (Some(_), None) => '🮑',
                        (None, Some(_)) => '🮒',
                        (None, None) => '🮐',
                        // (Some(_), Some(_)) => '█',
                        // (Some(_), None) => '▀',
                        // (None, Some(_)) => '▄',
                        // (None, None) => ' ',
                    }
                )?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

pub struct NavGridDebugInput<'a> {
    pub depth: &'a wgpu::Texture,
}

pub struct NavGridDebug {
    depth_view: wgpu::TextureView,

    vertices: wgpu::Buffer,
    vertices_count: u32,
    pipeline: wgpu::RenderPipeline,
}

impl NavGridDebug {
    pub fn new(
        device: &wgpu::Device,
        camera: &CameraManager,
        triangles: &[[glam::Vec3; 3]],
        format: wgpu::TextureFormat,
        input: NavGridDebugInput,
    ) -> Self {
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("NavGridDebug vertices"),
            contents: bytemuck::cast_slice(triangles),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let vertices_count = triangles.len() as u32 * 3;

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("NavGridDebug pipeline layout"),
            bind_group_layouts: &[Some(&camera.bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("NavGridDebug shader"),
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

                    struct VertexOutput {
                        @builtin(position) position: vec4<f32>,
                        @location(0) color: vec4<f32>,
                    }

                    @vertex
                    fn vs_main(@location(0) pos: vec3<f32>) -> VertexOutput {
                        var out: VertexOutput;

                        out.position = camera.view_proj * vec4<f32>(
                            pos + vec3<f32>(0.0, 0.2, 0.0),
                            1.0,
                        );

                        out.color = vec4<f32>(
                            (pos.x / 15.0) * 0.5 + 0.5,
                            (pos.z / 15.0) * 0.5 + 0.5,
                            0.0,
                            0.3,
                        );
                        out.color = vec4<f32>(1.0, 0.0, 0.0, 1.0);

                        return out;
                    }

                    @fragment
                    fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                        return in.color;
                    }
                }
                .to_string()
                .into(),
            ),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NavGridDebug render pipeline"),
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
            depth_view: input.depth.create_view(&Default::default()),

            vertices,
            vertices_count,
            pipeline,
        }
    }

    pub fn rebind(&mut self, input: NavGridDebugInput) {
        self.depth_view = input.depth.create_view(&Default::default());
    }

    pub fn render(&self, ctx: &mut RenderContext, camera: &CameraManager) {
        if self.vertices_count == 0 {
            return;
        }

        let mut rpass = ctx.encoder.scoped_render_pass(
            "NavGridDebug",
            wgpu::RenderPassDescriptor {
                label: Some("NavGridDebug"),
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
                    view: &self.depth_view,
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
