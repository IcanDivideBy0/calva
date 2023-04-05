use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
};

use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    CameraManager, RenderContext,
};
use glam::Vec3Swizzles;

use super::tile::Tile;

pub struct NavMesh {
    vertices: Vec<glam::Vec3>,
    indices: Vec<[u16; 3]>,
}

impl NavMesh {
    pub fn new(tile: &Tile) -> Self {
        let get_height = |x: i32, y: i32| {
            let y = y.max(0).min(Tile::TEXTURE_SIZE as i32 - 1) as usize;
            let x = x.max(0).min(Tile::TEXTURE_SIZE as i32 - 1) as usize;

            tile.height_map[y][x]
        };

        const MAX_STEP: f32 = 1.0;
        const RADIUS: f32 = 0.8;

        let iter_circle = || {
            let r: i32 = (RADIUS / Tile::PIXEL_SIZE).ceil() as _;

            (-r..=r).flat_map(move |y| {
                (-r..=r).filter_map(move |x| {
                    if x.pow(2) + y.pow(2) < r.pow(2) {
                        Some((x, y))
                    } else {
                        None
                    }
                })
            })
        };

        let mut cells = HashSet::<(i32, i32)>::new();
        for y in 0..Tile::TEXTURE_SIZE as i32 {
            'cell: for x in 0..Tile::TEXTURE_SIZE as i32 {
                // if x > 4 || y > 4 {
                //     continue;
                // }

                let height = get_height(x, y);

                for (xx, yy) in iter_circle() {
                    if (height - get_height(x + xx, y + yy)).abs() > MAX_STEP {
                        continue 'cell;
                    }
                }

                cells.insert((x, y));
            }
        }

        let mut vertices: Vec<glam::Vec3> = vec![];
        let mut vertices_map = BTreeMap::<(i32, i32), u16>::new();

        let indices = cells
            .drain()
            .filter_map(|(x, y)| {
                let mut tl = glam::vec2(x as f32, y as f32);
                let mut tr = tl + glam::Vec2::X;
                let mut bl = tl + glam::Vec2::Y;
                let mut br = bl + glam::Vec2::X;

                tl = tl * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0;
                tr = tr * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0;
                bl = bl * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0;
                br = br * Tile::PIXEL_SIZE - Tile::WORLD_SIZE / 2.0;

                let height = get_height(x, y);

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

                let tl = tlh.map(|tlh| tl.extend(tlh).xzy());
                let tr = trh.map(|trh| tr.extend(trh).xzy());
                let bl = blh.map(|blh| bl.extend(blh).xzy());
                let br = brh.map(|brh| br.extend(brh).xzy());

                let tl = tl.map(|tl| {
                    let idx = vertices.len();
                    *vertices_map.entry((x, y)).or_insert_with(|| {
                        vertices.push(tl);
                        idx as _
                    })
                });
                let tr = tr.map(|tr| {
                    let idx = vertices.len();
                    *vertices_map.entry((x + 1, y)).or_insert_with(|| {
                        vertices.push(tr);
                        idx as _
                    })
                });
                let bl = bl.map(|bl| {
                    let idx = vertices.len();
                    *vertices_map.entry((x, y + 1)).or_insert_with(|| {
                        vertices.push(bl);
                        idx as _
                    })
                });
                let br = br.map(|br| {
                    let idx = vertices.len();
                    *vertices_map.entry((x + 1, y + 1)).or_insert_with(|| {
                        vertices.push(br);
                        idx as _
                    })
                });

                let corners = [tl, tr, bl, br]
                    .iter()
                    .filter_map(|v| *v)
                    .collect::<Vec<_>>();

                match corners.len() {
                    4 => {
                        let diag1 = vertices[corners[1] as usize] - vertices[corners[2] as usize];
                        let diag2 = vertices[corners[3] as usize] - vertices[corners[0] as usize];

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
            .collect::<Vec<_>>();

        let vertices = RefCell::new(vertices);
        let indices = RefCell::new(indices);

        let merge_verts = |i1: u16, i2: u16| {
            let mut vertices = vertices.borrow_mut();
            let mut indices = indices.borrow_mut();

            // let [i1, i2] = [i1.min(i2), i1.max(i2)];

            indices.retain_mut(|tri| {
                if tri.contains(&i1) && tri.contains(&i2) {
                    return false;
                }

                for i in tri.iter_mut() {
                    *i = match std::cmp::Ord::cmp(i, &i2) {
                        std::cmp::Ordering::Less => *i,
                        std::cmp::Ordering::Equal => i1,
                        std::cmp::Ordering::Greater => *i - 1,
                    }
                }

                true
            });

            // let middle = (vertices[i1 as usize] + vertices[i2 as usize]) / 2.0;
            // vertices[i1 as usize] = middle;
            vertices.remove(i2 as usize);
        };

        // merge_verts(
        //     *vertices_map.get(&(62, 62)).unwrap(),
        //     *vertices_map.get(&(62, 63)).unwrap(),
        // );

        // let get_vert_triangles = |i: u16| {
        //     indices
        //         .borrow()
        //         .iter()
        //         .filter(|t| t.contains(&i))
        //         .copied()
        //         .collect::<Vec<_>>()
        // };

        // let get_triangle_verts = |t: &[u16; 3]| {
        //     let vertices = vertices.borrow();

        //     (
        //         vertices[t[0] as usize],
        //         vertices[t[1] as usize],
        //         vertices[t[2] as usize],
        //     )
        // };

        // let get_vert_normal = |i: u16| {
        //     let tris = get_vert_triangles(i);

        //     tris.iter()
        //         .map(|t| {
        //             let tri = get_triangle_verts(t);
        //             glam::Vec3::cross(tri.0 - tri.1, tri.0 - tri.2)
        //         })
        //         .sum::<glam::Vec3>()
        //         .normalize()
        // };

        // let find_mergables = || {
        //     let vertices = vertices.borrow();

        //     for (i, _) in vertices.iter().enumerate() {
        //         let i = i as u16;

        //         let n = get_vert_normal(i);

        //         let tris = get_vert_triangles(i);
        //         if tris.len() != 6 {
        //             continue;
        //         }

        //         let neighbours = tris
        //             .iter()
        //             .copied()
        //             .flatten()
        //             .filter(|v| *v != i)
        //             .collect::<HashSet<_>>();

        //         if neighbours.len() != tris.len() {
        //             continue;
        //         }

        //         for neighbour in &neighbours {
        //             let tris = get_vert_triangles(*neighbour);
        //             if tris.len() != 6 {
        //                 continue;
        //             }

        //             let neighbour_neighbours = tris
        //                 .iter()
        //                 .copied()
        //                 .flatten()
        //                 .filter(|v| *v != *neighbour)
        //                 .collect::<HashSet<_>>();

        //             if tris.len() != neighbour_neighbours.len() {
        //                 continue;
        //             }

        //             let neighbour_normal = get_vert_normal(*neighbour);
        //             if n == neighbour_normal {
        //                 return Some((i, *neighbour));
        //             }
        //         }
        //     }

        //     None
        // };

        // let mut count = 0;
        // while let Some((i1, i2)) = find_mergables() {
        //     count += 1;
        //     println!("merge {i1} {i2}");
        //     merge_verts(i1, i2);
        //     if count > 100 {
        //         break;
        //     }
        // }

        // for &[i1, i2, i3] in &indices {
        //     let triangle = (
        //         vertices[i1 as usize],
        //         vertices[i2 as usize],
        //         vertices[i3 as usize],
        //     );
        // }

        Self {
            indices: indices.into_inner(),
            vertices: vertices.into_inner(),
        }
    }
}

pub struct NavMeshDebugInput<'a> {
    pub depth: &'a wgpu::Texture,
}

pub struct NavMeshDebug {
    depth_view: wgpu::TextureView,

    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
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
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("NavMeshDebug indices"),
            contents: bytemuck::cast_slice(&navmesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("NavMeshDebug vertices"),
            contents: bytemuck::cast_slice(&navmesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let vertices_count = navmesh.indices.len() as u32 * 3;

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
                            (pos.x / 15.0) * 0.25 + 0.5,
                            (pos.z / 15.0) * 0.25 + 0.5,
                            0.0,
                            1.0,
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
        });

        Self {
            depth_view: input.depth.create_view(&Default::default()),

            indices,
            vertices,
            vertices_count,
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
                view: ctx.frame,
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

        rpass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        rpass.set_vertex_buffer(0, self.vertices.slice(..));

        rpass.draw_indexed(0..self.vertices_count, 0, 0..1);
    }
}
