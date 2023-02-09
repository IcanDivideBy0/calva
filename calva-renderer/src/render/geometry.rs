use crate::{
    AnimationState, AnimationsManager, CameraManager, InstancesManager, MaterialId,
    MaterialsManager, MeshesManager, RenderContext, Renderer, SkinsManager, TexturesManager,
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

struct GeometryOutput {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    resolve_target: Option<wgpu::TextureView>,
}

impl GeometryOutput {
    fn new(renderer: &Renderer, desc: wgpu::TextureDescriptor) -> Self {
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            sample_count: 1,
            ..desc
        });

        let mut view = texture.create_view(&Default::default());
        let mut resolve_target = None;

        if desc.sample_count > 1 {
            resolve_target = Some(view);
            view = renderer
                .device
                .create_texture(&desc)
                .create_view(&Default::default())
        }

        Self {
            texture,
            view,
            resolve_target,
        }
    }

    fn attachment(&self) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: &self.view,
            resolve_target: self.resolve_target.as_ref(),
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: true,
            },
        }
    }

    fn resolve_view(&self) -> &wgpu::TextureView {
        self.resolve_target.as_ref().unwrap_or(&self.view)
    }

    fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }
}

pub struct GeometryPass {
    cull: GeometryCull,
    hiz: GeometryHiZ,

    albedo_metallic: GeometryOutput,
    normal_roughness: GeometryOutput,

    pipeline: wgpu::RenderPipeline,
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

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        renderer: &Renderer,
        camera: &CameraManager,
        textures: &TexturesManager,
        materials: &MaterialsManager,
        meshes: &MeshesManager,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
    ) -> Self {
        let cull = GeometryCull::new(renderer, camera, meshes, instances);
        let hiz = GeometryHiZ::new(renderer);

        let (albedo_metallic, normal_roughness) = Self::make_textures(renderer);

        let shader = renderer
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Geometry[render] shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("geometry.wgsl").into()),
            });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Geometry[render] render pipeline layout"),
                    bind_group_layouts: &[
                        &camera.bind_group_layout,
                        &textures.bind_group_layout,
                        &materials.bind_group_layout,
                        &skins.bind_group_layout,
                        &animations.bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            });

        GeometryPass {
            cull,
            hiz,

            albedo_metallic,
            normal_roughness,

            pipeline,
        }
    }

    pub fn albedo_metallic(&self) -> &wgpu::Texture {
        &self.albedo_metallic.texture
    }
    pub fn albedo_metallic_view(&self) -> &wgpu::TextureView {
        self.albedo_metallic.resolve_view()
    }

    pub fn normal_roughness(&self) -> &wgpu::Texture {
        &self.normal_roughness.texture
    }
    pub fn normal_roughness_view(&self) -> &wgpu::TextureView {
        self.normal_roughness.resolve_view()
    }

    pub fn size(&self) -> (u32, u32) {
        self.albedo_metallic.size()
    }

    pub fn resize(&mut self, renderer: &Renderer) {
        (self.albedo_metallic, self.normal_roughness) = Self::make_textures(renderer);
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
                Some(self.albedo_metallic.attachment()),
                Some(self.normal_roughness.attachment()),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: ctx.depth_stencil,
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

        self.hiz.hiz(ctx);

        ctx.encoder.profile_end();
    }

    fn make_textures(renderer: &Renderer) -> (GeometryOutput, GeometryOutput) {
        let size = wgpu::Extent3d {
            width: renderer.surface_config.width,
            height: renderer.surface_config.height,
            depth_or_array_layers: 1,
        };

        let albedo_metallic = GeometryOutput::new(
            renderer,
            wgpu::TextureDescriptor {
                label: Some("GBuffer albedo/metallic texture"),
                size,
                mip_level_count: 1,
                sample_count: Renderer::MULTISAMPLE_STATE.count,
                dimension: wgpu::TextureDimension::D2,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                format: Self::ALBEDO_METALLIC_FORMAT,
                view_formats: &[Self::ALBEDO_METALLIC_FORMAT],
            },
        );

        let normal_roughness = GeometryOutput::new(
            renderer,
            wgpu::TextureDescriptor {
                label: Some("Geometry normal/roughness texture"),
                size,
                mip_level_count: 1,
                sample_count: Renderer::MULTISAMPLE_STATE.count,
                dimension: wgpu::TextureDimension::D2,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                format: Self::NORMAL_ROUGHNESS_FORMAT,
                view_formats: &[Self::NORMAL_ROUGHNESS_FORMAT],
            },
        );

        (albedo_metallic, normal_roughness)
    }
}

use cull::*;
mod cull {
    use crate::{
        CameraManager, Instance, InstancesManager, MeshInfo, MeshesManager, RenderContext, Renderer,
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
            renderer: &Renderer,
            camera: &CameraManager,
            meshes: &MeshesManager,
            instances: &InstancesManager,
        ) -> Self {
            let draw_instances = renderer.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Geometry[cull] draw instances"),
                size: (std::mem::size_of::<[DrawInstance; InstancesManager::MAX_INSTANCES]>()) as _,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });

            let draw_indirects = renderer.device.create_buffer(&wgpu::BufferDescriptor {
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
                                        std::mem::size_of::<u32>() as _,
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

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
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

            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("geometry.cull.wgsl"));

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Geometry[cull] pipeline layout"),
                        bind_group_layouts: &[&camera.bind_group_layout, &bind_group_layout],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStages::COMPUTE,
                            range: 0..(std::mem::size_of::<u32>() as _),
                        }],
                    });

            let pipelines = (
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("Geometry[cull] reset pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: "reset",
                    }),
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("Geometry[cull] cull pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: "cull",
                    }),
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
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

            let meshes_count: u32 = meshes.count();
            let instances_count: u32 = instances.count();

            cpass.set_pipeline(&self.pipelines.0);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &self.bind_group, &[]);
            cpass.dispatch_workgroups(meshes_count, 1, 1);

            cpass.set_pipeline(&self.pipelines.1);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &self.bind_group, &[]);
            cpass.dispatch_workgroups(instances_count, 1, 1);

            cpass.set_pipeline(&self.pipelines.2);
            cpass.set_bind_group(0, &camera.bind_group, &[]);
            cpass.set_bind_group(1, &self.bind_group, &[]);
            cpass.dispatch_workgroups(meshes_count, 1, 1);
        }
    }
}

use hiz::*;
mod hiz {
    use crate::{RenderContext, Renderer};

    pub struct GeometryHiZ {
        bind_group: wgpu::BindGroup,
        pipeline: wgpu::ComputePipeline,
    }

    impl GeometryHiZ {
        pub fn new(renderer: &Renderer) -> Self {
            let depth = renderer
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("Geometry[hi-z] depth"),
                    size: wgpu::Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::Depth32Float],
                })
                .create_view(&Default::default());

            let output = renderer
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("Geometry[hi-z] output"),
                    size: wgpu::Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::STORAGE_BINDING,
                    view_formats: &[wgpu::TextureFormat::R32Float],
                })
                .create_view(&Default::default());

            let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Geometry[hi-z] sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("Geometry[hi-z] bind group layout"),
                        entries: &[
                            // Depth input
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Depth,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            // Sampler
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                            // Output
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::WriteOnly,
                                    format: wgpu::TextureFormat::R32Float,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                },
                                count: None,
                            },
                        ],
                    });

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Geometry[hi-z] bind group"),
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&depth),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&output),
                        },
                    ],
                });

            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("geometry.hi-z.wgsl"));

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Geometry[hi-z] pipeline layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStages::COMPUTE,
                            range: 0..(std::mem::size_of::<f32>() as _),
                        }],
                    });

            let pipeline =
                renderer
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("Geometry[hi-z] pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: "main",
                    });

            Self {
                bind_group,
                pipeline,
            }
        }

        pub fn hiz(&self, ctx: &mut RenderContext) {
            let mut cpass = ctx
                .encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Geometry[hi-z]"),
                });

            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &self.bind_group, &[]);
            cpass.dispatch_workgroups(256, 256, 1);
        }
    }
}
