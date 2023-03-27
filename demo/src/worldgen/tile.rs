#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Triangle(glam::Vec3, glam::Vec3, glam::Vec3);

use calva::renderer::wgpu::{self, util::DeviceExt};

pub struct TileBuilder<'a> {
    doc: &'a gltf::Document,
    buffers: &'a [gltf::buffer::Data],

    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
}

impl<'a> TileBuilder<'a> {
    pub fn new(
        device: &wgpu::Device,
        doc: &'a gltf::Document,
        buffers: &'a [gltf::buffer::Data],
    ) -> Self {
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TileBuilder depth"),
            size: wgpu::Extent3d {
                width: Tile::TEXTURE_SIZE as _,
                height: Tile::TEXTURE_SIZE as _,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth16Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TileBuilder pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TileBuilder shader"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    r#"
                        @vertex
                        fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {{
                            return vec4<f32>(
                                pos.x / {tile_half_size:.1},
                                -pos.z / {tile_half_size:.1},
                                -(pos.y / {tile_max_height:.1}) * 0.5 + 0.5,
                                1.0,
                            );
                        }}
                    "#,
                    tile_half_size = Tile::WORLD_SIZE as f32 / 2.0,
                    tile_max_height = Tile::MAX_HEIGHT,
                )
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
            doc,
            buffers,

            depth,
            depth_view,
            pipeline,
        }
    }

    pub fn build(&self, device: &wgpu::Device, queue: &wgpu::Queue, tile_name: &str) -> Tile {
        let root_node = self
            .doc
            .nodes()
            .find(|node| Some(tile_name) == node.name())
            .expect(&format!("Unable to find node: {}", tile_name));

        let get_buffer_data = |buffer: gltf::Buffer| -> Option<&[u8]> {
            self.buffers.get(buffer.index()).map(std::ops::Deref::deref)
        };

        fn traverse_nodes_tree<'a, T>(
            nodes: impl Iterator<Item = gltf::Node<'a>>,
            visitor: &mut dyn FnMut(&T, &gltf::Node) -> Option<T>,
            acc: T,
        ) {
            for node in nodes {
                if let Some(res) = visitor(&acc, &node) {
                    traverse_nodes_tree(node.children(), visitor, res);
                }
            }
        }

        let mut triangles = vec![];
        traverse_nodes_tree::<glam::Mat4>(
            root_node.children(),
            &mut |parent_transform, node| {
                let skip = node
                    .extras()
                    .as_ref()
                    .and_then(|extras| {
                        let extras =
                            serde_json::from_str::<serde_json::Map<_, _>>(extras.get()).ok()?;

                        Some(["partly_hidden", "prop"].iter().any(|&name| {
                            extras
                                .get(name.into())
                                .and_then(|value| value.as_u64())
                                .map(|i| i > 0)
                                .unwrap_or(false)
                        }))
                    })
                    .unwrap_or(false);

                if skip {
                    return None;
                }

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

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TileBuilder verts buffer"),
            contents: bytemuck::cast_slice(&triangles),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let vertices_count = 3 * triangles.len() as u32;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{}", tile_name)),
            size: (self.depth.width()
                * self.depth.height()
                * self.depth.format().describe().block_size as u32) as _,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("TileBuilder command encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("TileBuilder"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_vertex_buffer(0, vertices.slice(..));
            rpass.draw(0..vertices_count, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.depth,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: std::num::NonZeroU32::new(
                        self.depth.width() * self.depth.format().describe().block_size as u32,
                    ),
                    rows_per_image: None,
                },
            },
            self.depth.size(),
        );

        let submission_index = queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, Result::unwrap);

        device.poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));

        let buffer_view = buffer_slice.get_mapped_range();
        let data: [u16; Tile::TEXTURE_SIZE.pow(2)] = bytemuck::cast_slice::<u8, u16>(&*buffer_view)
            .try_into()
            .unwrap();

        Tile {
            node_id: root_node.index(),

            // wait for https://doc.rust-lang.org/nightly/core/primitive.slice.html#method.as_chunks
            height_map: unsafe { std::mem::transmute(data) },
        }
    }
}

pub struct Tile {
    pub node_id: usize,
    pub height_map: [[u16; Self::TEXTURE_SIZE]; Self::TEXTURE_SIZE],
}

impl Tile {
    pub const WORLD_SIZE: f32 = 5.0 * 6.0;
    pub const WORLD_FLOOR_HEIGHT: f32 = 4.0;

    pub const WORLD_POS: glam::Vec3 =
        glam::vec3(Self::WORLD_SIZE, Self::WORLD_FLOOR_HEIGHT, Self::WORLD_SIZE);

    pub const MAX_HEIGHT: f32 = 40.0;

    pub const TEXTURE_SIZE: usize =
        wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize / std::mem::size_of::<u16>();
    pub const WFC_SAMPLES: usize = 5;

    pub fn iter_face(&self, face: Face) -> Box<dyn Iterator<Item = &u16> + '_> {
        match face {
            Face::North => Box::new(self.height_map[0].iter()),
            Face::East => Box::new(
                self.height_map
                    .iter()
                    .map(|row| &row[Tile::TEXTURE_SIZE - 1]),
            ),
            Face::South => Box::new(self.height_map[Tile::TEXTURE_SIZE - 1].iter().rev()),
            Face::West => Box::new(self.height_map.iter().map(|row| &row[0]).rev()),
        }
    }

    pub fn wfc_constraints(&self) -> [[Option<u8>; Tile::WFC_SAMPLES]; 4] {
        Face::all().map(|face: Face| -> [Option<u8>; Tile::WFC_SAMPLES] {
            (0..Tile::WFC_SAMPLES)
                .map(|i| {
                    const STEP: f32 = Tile::TEXTURE_SIZE as f32 / Tile::WFC_SAMPLES as f32;
                    ((i as f32 + 0.5) * STEP) as usize
                })
                .map(|i| {
                    let height = *self.iter_face(face).skip(i).next().unwrap();

                    let world_height = ((height as f32 / u16::MAX as f32) - 0.5) * -2.0 * 40.0;
                    ((world_height / Tile::WORLD_FLOOR_HEIGHT).round() as i32)
                        .try_into()
                        .ok()
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        })
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
