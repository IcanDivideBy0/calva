use calva::renderer::wgpu::{self, util::DeviceExt};
use itertools::Itertools;
use wesl::syntax::*;

pub struct TileBuilder {
    walls_pipeline: wgpu::RenderPipeline,
    floor_pipeline: wgpu::RenderPipeline,
}

impl TileBuilder {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32FloatStencil8;

    pub fn new(device: &wgpu::Device) -> Self {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TileBuilder pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let tile_size = Tile::WORLD_SIZE;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TileBuilder shader"),
            source: wgpu::ShaderSource::Wgsl(
                wesl_quote::quote_module! {
                    @vertex
                    fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {{
                        return vec4<f32>(
                             pos.x / #tile_size * 2.0,
                            -pos.z / #tile_size * 2.0,
                            -pos.y / #tile_size * 0.5 + 0.5,
                            1.0,
                        );
                    }}
                }
                .to_string()
                .into(),
            ),
        });

        let walls_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TileBuilder wall pipeline"),
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
            label: Some("TileBuilder floor pipeline"),
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
            walls_pipeline,
            floor_pipeline,
        }
    }

    pub fn build(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffers: &[gltf::buffer::Data],
        node: gltf::Node,
    ) -> Option<Tile> {
        let get_buffer_data = |buffer: gltf::Buffer| -> Option<&[u8]> {
            buffers.get(buffer.index()).map(std::ops::Deref::deref)
        };

        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!(
                "TileBuilder depth {}",
                node.name().unwrap_or_default()
            )),
            size: wgpu::Extent3d {
                width: Tile::TEXTURE_SIZE as _,
                height: Tile::TEXTURE_SIZE as _,
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

        let mut floor_triangles = vec![];
        let mut walls_triangles = vec![];
        calva::gltf::traverse_nodes_tree::<glam::Mat4>(
            node.children(),
            &mut |parent_transform, node| {
                let get_flag = |flag: &str| {
                    node.extras()
                        .as_ref()
                        .and_then(|extras| {
                            serde_json::from_str::<serde_json::Map<_, _>>(extras.get())
                                .ok()?
                                .get(flag)
                                .and_then(|value| value.as_bool())
                        })
                        .unwrap_or(false)
                };

                let triangles = match (get_flag("wall"), get_flag("floor")) {
                    (true, _) => &mut walls_triangles,
                    (_, true) => &mut floor_triangles,
                    _ => return None,
                };

                let transform =
                    *parent_transform * glam::Mat4::from_cols_array_2d(&node.transform().matrix());

                if let Some(mesh) = node.mesh() {
                    for primitive in mesh.primitives() {
                        let reader = primitive.reader(get_buffer_data);

                        let vertices: Vec<_> = reader
                            .read_positions()?
                            .map(glam::Vec3::from_array)
                            .collect();

                        let indices: Vec<_> = reader.read_indices()?.into_u32().collect();

                        triangles.extend(indices.chunks_exact(3).filter_map(|chunk| {
                            let [i1, i2, i3] = <[u32; 3]>::try_from(chunk).ok()?;

                            Some([
                                transform.transform_point3(*vertices.get(i1 as usize)?),
                                transform.transform_point3(*vertices.get(i2 as usize)?),
                                transform.transform_point3(*vertices.get(i3 as usize)?),
                            ])
                        }));
                    }
                }

                Some(transform)
            },
            glam::Mat4::IDENTITY,
        );

        if floor_triangles.is_empty() {
            return Some(Tile {
                node_id: node.index(),
                depth,
                height_map: [[-Tile::WORLD_SIZE; Tile::TEXTURE_SIZE]; Tile::TEXTURE_SIZE],
            });
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("TileBuilder command encoder"),
        });

        if !walls_triangles.is_empty() {
            let walls_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("TileBuilder[walls] verts buffer"),
                contents: bytemuck::cast_slice(&walls_triangles),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let walls_vertices_count = 3 * walls_triangles.len() as u32;

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!(
                    "TileBuilder[walls] {}",
                    node.name().unwrap_or_default()
                )),
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
            rpass.set_vertex_buffer(0, walls_vertices.slice(..));
            rpass.draw(0..walls_vertices_count, 0..1);
        }

        {
            let floor_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("TileBuilder[floor] verts buffer"),
                contents: bytemuck::cast_slice(&floor_triangles),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let floor_vertices_count = 3 * floor_triangles.len() as u32;

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!(
                    "TileBuilder[floor] {}",
                    node.name().unwrap_or_default()
                )),
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
            rpass.set_vertex_buffer(0, floor_vertices.slice(..));
            rpass.draw(0..floor_vertices_count, 0..1);
        }

        let depth_block_size = depth
            .format()
            .block_copy_size(Some(wgpu::TextureAspect::DepthOnly))?;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: node.name(),
            size: (depth.width() * depth.height() * depth_block_size) as _,
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
                    bytes_per_row: Some(
                        depth.width()
                            * depth
                                .format()
                                .block_copy_size(Some(wgpu::TextureAspect::DepthOnly))?,
                    ),
                    rows_per_image: None,
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

        let height_map = bytemuck::cast_slice::<u8, f32>(&buffer_view)
            .iter()
            .map(|depth| (depth - 0.5) * -2.0 * Tile::WORLD_SIZE)
            .chunks(Tile::TEXTURE_SIZE)
            .into_iter()
            .filter_map(Itertools::collect_array)
            .collect_array()?;

        Some(Tile {
            node_id: node.index(),
            depth,
            height_map,
        })
    }
}

pub struct Tile {
    pub node_id: usize,
    pub depth: wgpu::Texture,
    pub height_map: [[f32; Self::TEXTURE_SIZE]; Self::TEXTURE_SIZE],
}

impl Tile {
    pub const WORLD_SIZE: f32 = 5.0 * 6.0;

    pub const TEXTURE_SIZE: usize =
        wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize / std::mem::size_of::<f32>();

    pub const PIXEL_SIZE: f32 = Self::WORLD_SIZE / Self::TEXTURE_SIZE as f32;

    pub fn get_height(&self, pos: glam::USizeVec2) -> f32 {
        let coord = pos.min(glam::usizevec2(
            Self::TEXTURE_SIZE - 1,
            Self::TEXTURE_SIZE - 1,
        ));

        self.height_map[coord.y][coord.x]
    }

    pub fn world_get_height(&self, pos: glam::Vec2) -> f32 {
        let coord = (pos / Self::WORLD_SIZE * Self::TEXTURE_SIZE as f32).clamp(
            glam::vec2(0.0, 0.0),
            glam::vec2(
                (Self::TEXTURE_SIZE - 1) as f32,
                (Self::TEXTURE_SIZE - 1) as f32,
            ),
        );

        self.height_map[coord.y.floor() as usize][coord.x.floor() as usize]
    }

    pub fn get_grid_coord(local_coord: &glam::Vec2) -> glam::USizeVec2 {
        let coord_norm = local_coord / Self::WORLD_SIZE + 0.5;
        let grid_coord = (coord_norm * Self::TEXTURE_SIZE as f32).clamp(
            glam::Vec2::ZERO,
            glam::Vec2::splat((Self::TEXTURE_SIZE - 1) as f32),
        );

        glam::usizevec2(grid_coord.x as usize, grid_coord.y as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    North,
    East,
    South,
    West,
}

impl Face {
    pub const fn all() -> [Self; 4] {
        [Self::North, Self::East, Self::South, Self::West]
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}
