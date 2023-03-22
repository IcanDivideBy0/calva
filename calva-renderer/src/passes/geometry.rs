use crate::{
    AnimationState, AnimationsManager, CameraManager, InstancesManager, MaterialId,
    MaterialsManager, MeshesManager, RenderContext, SkinsManager, TexturesManager,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawInstance {
    _model_matrix: [f32; 16],
    _normal_quat: [f32; 4],
    _material: MaterialId,
    _skin_offset: i32,
    _animation: AnimationState,
}

impl DrawInstance {
    pub(crate) const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    pub(crate) const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE,
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
            6 => Sint32, // Skin offset
            7 => Uint32, // Animation ID
            8 => Float32, // Animation time
        ],
    };
}

pub struct GeometryPassOutputs {
    pub albedo_metallic: wgpu::Texture,
    pub normal_roughness: wgpu::Texture,
    pub emissive: wgpu::Texture,
    pub depth: wgpu::Texture,
}

pub struct GeometryPass {
    pub outputs: GeometryPassOutputs,

    albedo_metallic_view: wgpu::TextureView,
    normal_roughness_view: wgpu::TextureView,
    emissive_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,

    cull: GeometryCull,

    pipeline: wgpu::RenderPipeline,
}

impl GeometryPass {
    pub const FEATURES: &'static [wgpu::Features] = &[
        wgpu::Features::TEXTURE_BINDING_ARRAY,
        wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
        wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY,
        wgpu::Features::MULTI_DRAW_INDIRECT,
    ];

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,

        camera: &CameraManager,
        textures: &TexturesManager,
        materials: &MaterialsManager,
        meshes: &MeshesManager,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
    ) -> Self {
        let outputs = Self::make_outputs(device, surface_config);

        let albedo_metallic_view = outputs.albedo_metallic.create_view(&Default::default());
        let normal_roughness_view = outputs.normal_roughness.create_view(&Default::default());
        let emissive_view = outputs.emissive.create_view(&Default::default());
        let depth_view = outputs.depth.create_view(&Default::default());

        let cull = GeometryCull::new(device, camera, meshes, instances);

        let shader = device.create_shader_module(wgpu::include_wgsl!("geometry.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry[render] pipeline layout"),
            bind_group_layouts: &[
                &camera.bind_group_layout,
                &textures.bind_group_layout,
                &materials.bind_group_layout,
                &skins.bind_group_layout,
                &animations.bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry[render] render pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    DrawInstance::LAYOUT,
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
                        format: outputs.albedo_metallic.format(),
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: outputs.normal_roughness.format(),
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: outputs.emissive.format(),
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
                format: outputs.depth.format(),
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
        });

        GeometryPass {
            outputs,

            cull,

            albedo_metallic_view,
            normal_roughness_view,
            emissive_view,
            depth_view,

            pipeline,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) {
        self.outputs = Self::make_outputs(device, surface_config);

        self.albedo_metallic_view = self
            .outputs
            .albedo_metallic
            .create_view(&Default::default());
        self.normal_roughness_view = self
            .outputs
            .normal_roughness
            .create_view(&Default::default());
        self.emissive_view = self.outputs.emissive.create_view(&Default::default());
        self.depth_view = self.outputs.depth.create_view(&Default::default());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        ctx: &mut RenderContext,
        camera: &CameraManager,
        textures: &TexturesManager,
        materials: &MaterialsManager,
        meshes: &MeshesManager,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
    ) {
        ctx.encoder.profile_start("Geometry");

        self.cull.cull(ctx, camera, meshes, instances);

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Geometry[render]"),
            color_attachments: &[
                &self.albedo_metallic_view,
                &self.normal_roughness_view,
                &self.emissive_view,
            ]
            .map(|view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })
            }),
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

        rpass.set_bind_group(0, &camera.bind_group, &[]);
        rpass.set_bind_group(1, &textures.bind_group, &[]);
        rpass.set_bind_group(2, &materials.bind_group, &[]);
        rpass.set_bind_group(3, &skins.bind_group, &[]);
        rpass.set_bind_group(4, &animations.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.cull.draw_instances.slice(..));
        rpass.set_vertex_buffer(1, meshes.vertices.slice(..));
        rpass.set_vertex_buffer(2, meshes.normals.slice(..));
        rpass.set_vertex_buffer(3, meshes.tangents.slice(..));
        rpass.set_vertex_buffer(4, meshes.tex_coords0.slice(..));

        rpass.set_index_buffer(meshes.indices.slice(..), wgpu::IndexFormat::Uint32);

        rpass.multi_draw_indexed_indirect_count(
            &self.cull.draw_indirects,
            std::mem::size_of::<u32>() as _,
            &self.cull.draw_indirects,
            0,
            MeshesManager::MAX_MESHES as _,
        );

        drop(rpass);

        ctx.encoder.profile_end();
    }

    fn make_outputs(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> GeometryPassOutputs {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };

        let albedo_metallic = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GBuffer albedo/metallic texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            format: wgpu::TextureFormat::Bgra8Unorm,
            view_formats: &[wgpu::TextureFormat::Bgra8Unorm],
        });

        let normal_roughness = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Geometry normal/roughness texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            format: wgpu::TextureFormat::Rgba16Float,
            view_formats: &[wgpu::TextureFormat::Rgba16Float],
        });

        let emissive = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GBuffer albedo/metallic texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            format: wgpu::TextureFormat::Bgra8Unorm,
            view_formats: &[wgpu::TextureFormat::Bgra8Unorm],
        });

        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GBuffer depth texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            view_formats: &[wgpu::TextureFormat::Depth24PlusStencil8],
        });

        GeometryPassOutputs {
            albedo_metallic,
            normal_roughness,
            emissive,
            depth,
        }
    }
}

use cull::*;
mod cull {
    use crate::{
        CameraManager, Instance, InstancesManager, MeshInfo, MeshesManager, RenderContext,
    };

    use super::DrawInstance;

    pub struct GeometryCull {
        pub(crate) draw_instances: wgpu::Buffer,
        pub(crate) draw_indirects: wgpu::Buffer,

        bind_group: wgpu::BindGroup,
        pipelines: (
            wgpu::ComputePipeline, // reset
            wgpu::ComputePipeline, // cull
            wgpu::ComputePipeline, // count
        ),
    }

    impl GeometryCull {
        pub fn new(
            device: &wgpu::Device,
            camera: &CameraManager,
            meshes: &MeshesManager,
            instances: &InstancesManager,
        ) -> Self {
            let draw_instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Geometry[cull] draw instances"),
                size: (std::mem::size_of::<[DrawInstance; InstancesManager::MAX_INSTANCES]>()) as _,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });

            let draw_indirects = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Geometry[cull] draw indirects"),
                size: {
                    let count_size = std::mem::size_of::<u32>();
                    let indirects_size = std::mem::size_of::<
                        [wgpu::util::DrawIndexedIndirect; MeshesManager::MAX_MESHES],
                    >();

                    count_size + indirects_size
                } as _,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::INDIRECT,
                mapped_at_creation: false,
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Geometry[cull] bind group layout"),
                    entries: &[
                        // Mesh data
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(MeshInfo::SIZE),
                            },
                            count: None,
                        },
                        // Base instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<u32>() as _
                                ),
                            },
                            count: None,
                        },
                        // Cull instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<[u32; 4]>() as wgpu::BufferAddress
                                        + Instance::SIZE,
                                ),
                            },
                            count: None,
                        },
                        // Draw instances
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(DrawInstance::SIZE),
                            },
                            count: None,
                        },
                        // Draw indirects
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<u32>() as u64
                                        + std::mem::size_of::<wgpu::util::DrawIndexedIndirect>()
                                            as u64,
                                ),
                            },
                            count: None,
                        },
                    ],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Geometry[cull] bind group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: meshes.meshes_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: instances.base_instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: instances.instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: draw_instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: draw_indirects.as_entire_binding(),
                    },
                ],
            });

            let shader = device.create_shader_module(wgpu::include_wgsl!("geometry.cull.wgsl"));

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Geometry[cull] pipeline layout"),
                bind_group_layouts: &[&camera.bind_group_layout, &bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipelines = (
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Geometry[cull] reset pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: "reset",
                }),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Geometry[cull] cull pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: "cull",
                }),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Geometry[cull] count pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: "count",
                }),
            );

            Self {
                draw_instances,
                draw_indirects,

                bind_group,
                pipelines,
            }
        }

        pub fn cull(
            &self,
            ctx: &mut RenderContext,
            camera: &CameraManager,
            meshes: &MeshesManager,
            instances: &InstancesManager,
        ) {
            let mut cpass = ctx
                .encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Geometry[cull]"),
                });

            const WORKGROUP_SIZE: u32 = 32;

            let meshes_count: u32 = meshes.count();
            let meshes_workgroups_count =
                (meshes_count as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

            let instances_count: u32 = instances.count();
            let instances_workgroups_count =
                (instances_count as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

            cpass.set_pipeline(&self.pipelines.0);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &self.bind_group, &[]);
            cpass.dispatch_workgroups(meshes_workgroups_count, 1, 1);

            cpass.set_pipeline(&self.pipelines.1);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &self.bind_group, &[]);
            cpass.dispatch_workgroups(instances_workgroups_count, 1, 1);

            cpass.set_pipeline(&self.pipelines.2);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &self.bind_group, &[]);
            cpass.dispatch_workgroups(meshes_workgroups_count, 1, 1);
        }
    }
}
