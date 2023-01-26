use crate::{
    AnimationState, AnimationsManager, MaterialId, MaterialsManager, MeshData, MeshId,
    MeshesManager, RenderContext, Renderer, SkinsManager, TexturesManager,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshInstance {
    pub transform: glam::Mat4,
    pub mesh: MeshId,
    pub material: MaterialId,
    pub animation: AnimationState,
}

impl MeshInstance {
    const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;
}

struct CulledMeshInstance {
    _model_matrix: [f32; 16],
    _normal_quat: [f32; 4],
    _material: MaterialId,

    // Skinning only
    _skinning_offset: i32,
    _animation: AnimationState,
}

impl CulledMeshInstance {
    const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as _,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            // Model matrix
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4,
            3 => Float32x4,
            // Normal quat
            4 => Float32x4,
            // Material
            5 => Uint32,

            // Skinning
            6 => Sint32, // Skinning offset
            7 => Uint32, // Animation ID
            8 => Float32, // Animation time
        ],
    };
}

pub struct GeometryPass {
    pub textures: TexturesManager,
    pub materials: MaterialsManager,
    pub meshes: MeshesManager,
    pub skins: SkinsManager,
    pub animations: AnimationsManager,

    pub albedo_metallic: wgpu::TextureView,
    pub normal_roughness: wgpu::TextureView,

    instances: wgpu::Buffer,
    culled_instances: wgpu::Buffer,
    indirect: wgpu::Buffer,

    cull_bind_group: wgpu::BindGroup,
    cull_init_pipeline: wgpu::ComputePipeline,
    cull_pipeline: wgpu::ComputePipeline,

    render_pipeline: wgpu::RenderPipeline,
}

impl GeometryPass {
    pub const FEATURES: &'static [wgpu::Features] = &[
        wgpu::Features::TEXTURE_BINDING_ARRAY,
        wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
        wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY,
        wgpu::Features::MULTI_DRAW_INDIRECT,
    ];

    const ALBEDO_METALLIC_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
    const NORMAL_ROUGHNESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    const INDIRECT_SIZE: wgpu::BufferAddress =
        std::mem::size_of::<wgpu::util::DrawIndexedIndirect>() as _;

    const MAX_INSTANCES: usize = 1000;

    pub fn new(renderer: &Renderer) -> Self {
        let textures = TexturesManager::new(&renderer.device);
        let materials = MaterialsManager::new(&renderer.device);
        let meshes = MeshesManager::new(&renderer.device);
        let skins = SkinsManager::new(&renderer.device);
        let animations = AnimationsManager::new(&renderer.device);

        let (albedo_metallic, normal_roughness) = Self::make_textures(renderer);

        let instances = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Geometry meshes instances"),
            size: (std::mem::size_of::<[MeshInstance; Self::MAX_INSTANCES]>()) as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let culled_instances = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Geometry culled meshes instances"),
            size: (std::mem::size_of::<[CulledMeshInstance; Self::MAX_INSTANCES]>()) as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let indirect = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Geometry draw indirect"),
            size: (std::mem::size_of::<wgpu::util::DrawIndexedIndirect>()
                * MeshesManager::MAX_MESHES) as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::INDIRECT,
            mapped_at_creation: false,
        });

        let (cull_bind_group, cull_init_pipeline, cull_pipeline) = {
            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("shaders/geometry.cull.wgsl"));

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("Geometry[cull] bind group layout"),
                        entries: &[
                            // Mesh data
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: wgpu::BufferSize::new(MeshData::SIZE),
                                },
                                count: None,
                            },
                            // Mesh instances
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: wgpu::BufferSize::new(MeshInstance::SIZE),
                                },
                                count: None,
                            },
                            // Culled instances
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                                    has_dynamic_offset: false,
                                    min_binding_size: wgpu::BufferSize::new(
                                        CulledMeshInstance::SIZE,
                                    ),
                                },
                                count: None,
                            },
                            // Indirect draws
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                                    has_dynamic_offset: false,
                                    min_binding_size: wgpu::BufferSize::new(Self::INDIRECT_SIZE),
                                },
                                count: None,
                            },
                        ],
                    });

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Geometry[cull] bind group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: meshes.meshes_data.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: instances.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: culled_instances.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: indirect.as_entire_binding(),
                        },
                    ],
                });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Geometry[cull] pipeline layout"),
                        bind_group_layouts: &[
                            &renderer.camera.bind_group_layout,
                            &bind_group_layout,
                        ],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStages::COMPUTE,
                            range: 0..(std::mem::size_of::<u32>() as _),
                        }],
                    });

            let init_pipeline =
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("Geometry[cull] init pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: "init",
                    });

            let pipeline =
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("Geometry[cull] pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: "cull",
                    });

            (bind_group, init_pipeline, pipeline)
        };

        let render_pipeline = {
            let shader = renderer
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Geometry[render] shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("shaders/geometry.wgsl").into()),
                });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Geometry[render] render pipeline layout"),
                        bind_group_layouts: &[
                            &renderer.camera.bind_group_layout,
                            &textures.bind_group_layout,
                            &materials.bind_group_layout,
                            &skins.bind_group_layout,
                            &animations.bind_group_layout,
                        ],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStages::VERTEX,
                            range: 0..(std::mem::size_of::<f32>() as _),
                        }],
                    });

            renderer
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Geometry[render] render pipeline"),
                    layout: Some(&pipeline_layout),
                    multiview: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[
                            CulledMeshInstance::LAYOUT,
                            // Positions
                            wgpu::VertexBufferLayout {
                                array_stride: MeshesManager::VERTEX_SIZE as _,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![10 => Float32x3],
                            },
                            // Normals
                            wgpu::VertexBufferLayout {
                                array_stride: MeshesManager::NORMAL_SIZE as _,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![11 => Float32x3],
                            },
                            // Tangents
                            wgpu::VertexBufferLayout {
                                array_stride: MeshesManager::TANGENT_SIZE as _,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![12 => Float32x4],
                            },
                            // UV
                            wgpu::VertexBufferLayout {
                                array_stride: MeshesManager::TEX_COORD_SIZE as _,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![13 => Float32x2],
                            },
                        ],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_main",
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: Self::ALBEDO_METALLIC_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: Self::NORMAL_ROUGHNESS_FORMAT,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                    }),
                    primitive: wgpu::PrimitiveState {
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: Renderer::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Renderer::MULTISAMPLE_STATE,
                })
        };

        GeometryPass {
            textures,
            materials,
            meshes,
            skins,
            animations,

            albedo_metallic,
            normal_roughness,

            instances,
            culled_instances,
            indirect,

            cull_bind_group,
            cull_init_pipeline,
            cull_pipeline,

            render_pipeline,
        }
    }

    pub fn resize(&mut self, renderer: &Renderer) {
        (self.albedo_metallic, self.normal_roughness) = Self::make_textures(renderer);
    }

    pub fn render<'e, 'data: 'e>(
        &self,
        ctx: &mut RenderContext,
        instances: &[MeshInstance], // cb: impl FnOnce(&mut dyn FnMut(&'data MeshId, &'data [MeshInstance])),
    ) {
        ctx.encoder.profile_start("Geometry");

        ctx.queue
            .write_buffer(&self.instances, 0, bytemuck::cast_slice(instances));

        let mut cpass = ctx
            .encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Geometry[cull]"),
            });

        let instances_count = instances.len() as u32;

        cpass.set_pipeline(&self.cull_init_pipeline);
        cpass.set_bind_group(0, &ctx.camera.bind_group, &[]);
        cpass.set_bind_group(1, &self.cull_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::bytes_of(&instances_count));
        cpass.dispatch_workgroups(self.meshes.count() as _, 1, 1);

        cpass.set_pipeline(&self.cull_pipeline);
        cpass.set_bind_group(0, &ctx.camera.bind_group, &[]);
        cpass.set_bind_group(1, &self.cull_bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::bytes_of(&instances_count));
        cpass.dispatch_workgroups(instances.len() as _, 1, 1);

        drop(cpass);

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Geometry[render]"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &self.albedo_metallic,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &self.normal_roughness,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &ctx.output.depth_stencil,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.render_pipeline);

        rpass.set_bind_group(0, &ctx.camera.bind_group, &[]);
        rpass.set_bind_group(1, &self.textures.bind_group, &[]);
        rpass.set_bind_group(2, &self.materials.bind_group, &[]);
        rpass.set_bind_group(3, &self.skins.bind_group, &[]);
        rpass.set_bind_group(4, &self.animations.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.culled_instances.slice(..));
        rpass.set_vertex_buffer(1, self.meshes.vertices.slice(..));
        rpass.set_vertex_buffer(2, self.meshes.normals.slice(..));
        rpass.set_vertex_buffer(3, self.meshes.tangents.slice(..));
        rpass.set_vertex_buffer(4, self.meshes.tex_coords0.slice(..));

        rpass.set_index_buffer(self.meshes.indices.slice(..), wgpu::IndexFormat::Uint32);

        rpass.multi_draw_indexed_indirect(&self.indirect, 0, MeshesManager::MAX_MESHES as _);

        drop(rpass);

        ctx.encoder.profile_end();
    }

    fn make_textures(renderer: &Renderer) -> (wgpu::TextureView, wgpu::TextureView) {
        let desc = wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: renderer.surface_config.width,
                height: renderer.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: Renderer::MULTISAMPLE_STATE.count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm, // whatever
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        };

        let albedo_metallic = renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("GBuffer albedo/metallic texture"),
                format: Self::ALBEDO_METALLIC_FORMAT,
                ..desc
            })
            .create_view(&Default::default());

        let normal_roughness = renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Geometry normal/roughness texture"),
                format: Self::NORMAL_ROUGHNESS_FORMAT,
                ..desc
            })
            .create_view(&Default::default());

        (albedo_metallic, normal_roughness)
    }
}
