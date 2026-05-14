use anyhow::Result;
use bytemuck::NoUninit;
use core::{f32, fmt};
use itertools::Itertools;
use parry3d::{
    math::Vector3,
    partitioning::{Bvh, BvhBuildStrategy},
    query::{Ray, RayCast},
    shape::Triangle,
};
use renderer::{
    wgpu::{self, util::DeviceExt},
    Resource, ResourcesManager,
};

use crate::util::debug_map;

pub struct HeightMapBuilder {
    resources: ResourcesManager,

    walls_pipeline: wgpu::RenderPipeline,
    floor_pipeline: wgpu::RenderPipeline,
}

impl HeightMapBuilder {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32FloatStencil8;
    const DEPTH_BLOCK_SIZE: usize = std::mem::size_of::<f32>();

    pub const TEXTURE_SIZE: usize =
        wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize / Self::DEPTH_BLOCK_SIZE;

    pub fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HeightMapBuilder shader"),
            ..wgpu::include_wgsl!("height_map.wgsl")
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HeightMapBuilder pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: std::mem::size_of::<f32>() as _,
        });

        let walls_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HeightMapBuilder[walls] pipeline"),
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
            fragment: None,
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: None,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Replace,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Replace,
                    },
                    read_mask: 0x00,
                    write_mask: 0xFF,
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            cache: None,
        });

        let floor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HeightMapBuilder[floor] pipeline"),
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
            fragment: None,
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    read_mask: 0xFF,
                    write_mask: 0x00,
                },
                bias: Default::default(),
            }),
            multisample: Default::default(),
            cache: None,
        });

        Self {
            resources,

            walls_pipeline,
            floor_pipeline,
        }
    }

    pub fn build<A: NoUninit>(
        &self,
        world_size: f32,
        floor_triangles: &[A],
        walls_triangles: &[A],
    ) -> (HeightMap<{ Self::TEXTURE_SIZE }>, wgpu::Texture) {
        let device = self.resources.read::<wgpu::Device>();
        let queue = self.resources.read::<wgpu::Queue>();

        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HeightMapBuilder depth"),
            size: wgpu::Extent3d {
                width: Self::TEXTURE_SIZE as _,
                height: Self::TEXTURE_SIZE as _,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("HeightMapBuilder command encoder"),
        });

        if !walls_triangles.is_empty() {
            let walls_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HeightMapBuilder[walls] verts buffer"),
                contents: bytemuck::cast_slice(walls_triangles),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let walls_vertices_count = 3 * walls_triangles.len() as u32;

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HeightMapBuilder[walls]"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                ..Default::default()
            });
            rpass.set_stencil_reference(1);

            rpass.set_pipeline(&self.walls_pipeline);
            rpass.set_immediates(0, bytemuck::bytes_of(&world_size));
            rpass.set_vertex_buffer(0, walls_vertices.slice(..));
            rpass.draw(0..walls_vertices_count, 0..1);
        }

        if !floor_triangles.is_empty() {
            let floor_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HeightMapBuilder[floor] verts buffer"),
                contents: bytemuck::cast_slice(floor_triangles),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let floor_vertices_count = 3 * floor_triangles.len() as u32;

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HeightMapBuilder[floor]"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rpass.set_stencil_reference(0);

            rpass.set_pipeline(&self.floor_pipeline);
            rpass.set_immediates(0, bytemuck::bytes_of(&world_size));
            rpass.set_vertex_buffer(0, floor_vertices.slice(..));
            rpass.draw(0..floor_vertices_count, 0..1);
        }

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (depth.width() * depth.height() * Self::DEPTH_BLOCK_SIZE as u32) as _,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &depth,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(depth.width() * Self::DEPTH_BLOCK_SIZE as u32),
                    rows_per_image: Some(depth.height()),
                },
            },
            depth.size(),
        );

        let submission_index = queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, Result::unwrap);

        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission_index),
                timeout: None,
            })
            .unwrap();

        let buffer_view = buffer_slice.get_mapped_range();

        let mut height_map_data = [[0.0; Self::TEXTURE_SIZE]; Self::TEXTURE_SIZE];

        for (y, row) in bytemuck::cast_slice::<u8, f32>(&buffer_view)
            .iter()
            .map(|depth| (depth - 0.5) * -2.0 * world_size)
            .chunks(Self::TEXTURE_SIZE)
            .into_iter()
            .enumerate()
        {
            for (x, height) in row.enumerate() {
                height_map_data[y][x] = height;
            }
        }

        (
            HeightMap::new(&height_map_data, world_size / Self::TEXTURE_SIZE as f32),
            depth,
        )
    }
}

impl Resource for HeightMapBuilder {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        Ok(Self::new(resources))
    }
}

pub struct HeightMap<const SIZE: usize = { HeightMapBuilder::TEXTURE_SIZE }> {
    pub grid: [[Option<f32>; SIZE]; SIZE],
    pub triangles: Vec<[glam::Vec3; 3]>,
    bvh: Bvh,
}

impl<const SIZE: usize> HeightMap<SIZE> {
    pub const SIZE: usize = SIZE;

    pub fn new(height_map: &[[f32; SIZE]; SIZE], sample_size: f32) -> Self {
        let get_height = |x: usize, y: usize| {
            let y = y.min(SIZE - 1);
            let x = x.min(SIZE - 1);

            height_map[y][x]
        };

        let tile_world_size = SIZE as f32 * sample_size;
        let min_height = -tile_world_size;

        let grid = std::array::from_fn(|y| {
            std::array::from_fn(|x| {
                let height = get_height(x, y);

                if height <= min_height {
                    return None;
                }

                let valid_neighbours = itertools::iproduct!(
                    y.saturating_sub(1)..=y.saturating_add(1),
                    x.saturating_sub(1)..=x.saturating_add(1),
                )
                .all(|(yy, xx)| {
                    let dist = if xx != x && yy != y {
                        f32::consts::SQRT_2
                    } else {
                        1.0
                    };

                    // This 2x factor on threshold is not logic to me, but omitting it
                    // produce different results than the triangles list where we're
                    // checking the dot product of the normal and the up axis.
                    (height - get_height(xx, yy)).abs() < dist * sample_size * 2.0
                });

                if valid_neighbours {
                    Some(height)
                } else {
                    None
                }
            })
        });

        let transform = glam::Mat4::from_translation(glam::vec3(
            -tile_world_size / 2.0,
            0.0,
            -tile_world_size / 2.0,
        ));

        let points: Vec<Vec<glam::Vec3>> = itertools::iproduct!(0..=SIZE, 0..=SIZE)
            .map(|(y, x)| {
                let it = itertools::iproduct!(0..=1, 0..=1,)
                    .map(|(yy, xx)| get_height(x.saturating_sub(xx), y.saturating_sub(yy)));

                let min = it.clone().fold(f32::INFINITY, f32::min);

                let height = if min > min_height {
                    it.fold(0.0, std::ops::Add::add) / 4.0
                } else {
                    min_height
                };

                transform.transform_point3(glam::vec3(
                    x as f32 * sample_size,
                    height, // engine is Y-up
                    y as f32 * sample_size,
                ))
            })
            .chunks(SIZE + 1)
            .into_iter()
            .map(std::iter::Iterator::collect)
            .collect();

        let triangles: Vec<_> = itertools::iproduct!(0..SIZE, 0..SIZE)
            .map(|(y, x)| {
                [
                    [points[y][x], points[y + 1][x + 1], points[y][x + 1]],
                    [points[y][x], points[y + 1][x], points[y + 1][x + 1]],
                ]
            })
            .filter(|[t1, t2]| {
                let check_triangle = |[a, b, c]: &[glam::Vec3; 3]| -> bool {
                    let normal = glam::Vec3::cross(b - a, a - c).normalize();

                    glam::Vec3::dot(normal, glam::Vec3::Y).abs() > f32::consts::SQRT_2 / 2.0
                        && a.y > min_height
                        && b.y > min_height
                        && c.y > min_height
                };

                check_triangle(t1) && check_triangle(t2)
            })
            .flatten()
            .collect();

        let bvh = Bvh::from_iter(
            BvhBuildStrategy::default(),
            triangles
                .iter()
                .map(|[a, b, c]| {
                    Triangle::new(
                        Vector3::from_array(a.to_array()),
                        Vector3::from_array(b.to_array()),
                        Vector3::from_array(c.to_array()),
                    )
                    .local_aabb()
                })
                .enumerate(),
        );

        Self {
            grid,
            triangles,
            bvh,
        }
    }

    pub fn get_height(&self, coord: &glam::USizeVec2) -> &Option<f32> {
        &self.grid[coord.y.min(SIZE)][coord.x.min(SIZE)]
    }

    pub fn ray_cast(&self, ro: glam::Vec3, rd: glam::Vec3) -> Option<f32> {
        let ray = Ray::new(
            Vector3::from_array(ro.to_array()),
            Vector3::from_array(rd.to_array()),
        );

        self.bvh
            .cast_ray(&ray, f32::MAX, |i, _| {
                let [a, b, c] = self.triangles[i as usize];
                Triangle::new(
                    Vector3::from_array(a.to_array()),
                    Vector3::from_array(b.to_array()),
                    Vector3::from_array(c.to_array()),
                )
                .cast_local_ray(&ray, f32::MAX, true)
            })
            .map(|(_, t)| t)
    }
}

impl<const SIZE: usize> fmt::Debug for HeightMap<SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let min = self
            .grid
            .iter()
            .flatten()
            .filter_map(|h| *h)
            .fold(f32::MAX, f32::min);

        let max = self
            .grid
            .iter()
            .flatten()
            .filter_map(|h| *h)
            .fold(f32::MIN, f32::max);

        write!(
            f,
            "{SIZE}×{SIZE} min={min:.1} max={max:.1}\n{}",
            debug_map(&self.grid, |height| match height {
                Some(height) => {
                    let height_norm = (height / max + 0.5) * (u8::MAX / 2) as f32;
                    let value = height_norm as u8;

                    (value, value, value)
                }
                None => (255, 0, 0),
            })
        )
    }
}
