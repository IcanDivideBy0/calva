use crate::{
    AnimationState, AnimationsManager, Camera, MaterialsManager, MeshesManager, RenderContext,
    Resource, ResourcesManager, SkinsManager, TexturesManager, UniformBuffer,
};
use anyhow::Result;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawInstance {
    pub model_matrix: [f32; 16],
    pub normal_quat: [f32; 4],
    pub material: u32,
    // pub material: MaterialHandle,
    // pub __padding1__: u8,
    // pub __padding2__: u16,
    pub skin_offset: i32,
    pub animation: AnimationState,
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

pub struct GeometryPass {
    resources: ResourcesManager,

    cull: GeometryCull,

    pipeline: wgpu::RenderPipeline,
}

impl GeometryPass {
    pub fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();
        let camera = resources.read::<UniformBuffer<Camera>>();
        let textures = resources.read::<TexturesManager>();
        let materials = resources.read::<MaterialsManager>();
        let skins = resources.read::<SkinsManager>();
        let animations = resources.read::<AnimationsManager>();
        let outputs = resources.read::<GeometryPassOutputs>();

        let cull = GeometryCull::new(&resources);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Geometry shader"),
            source: wgpu::ShaderSource::Wgsl(wesl::include_wesl!("passes::geometry").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry[render] pipeline layout"),
            bind_group_layouts: &[
                Some(&camera.bind_group_layout),
                Some(&textures.bind_group_layout),
                Some(&materials.bind_group_layout),
                Some(&skins.bind_group_layout),
                Some(&animations.bind_group_layout),
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry[render] render pipeline"),
            layout: Some(&pipeline_layout),
            multiview_mask: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
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
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            cache: None,
        });

        Self {
            resources,

            cull,

            pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let mut encoder = ctx.encoder.scope("Geometry");

        self.cull.cull(&mut encoder);

        let camera = self.resources.read::<UniformBuffer<Camera>>();
        let textures = self.resources.read::<TexturesManager>();
        let materials = self.resources.read::<MaterialsManager>();
        let skins = self.resources.read::<SkinsManager>();
        let animations = self.resources.read::<AnimationsManager>();
        let meshes = self.resources.read::<MeshesManager>();
        let outputs = self.resources.read::<GeometryPassOutputs>();

        let color_attachments = [
            &outputs.albedo_metallic_view,
            &outputs.normal_roughness_view,
            &outputs.emissive_view,
        ]
        .map(|view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })
        });

        let mut rpass = encoder.scoped_render_pass(
            "Geometry[render]",
            wgpu::RenderPassDescriptor {
                label: Some("Geometry[render]"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &outputs.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            },
        );

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
    }
}

pub struct GeometryPassOutputs {
    pub albedo_metallic: wgpu::Texture,
    pub albedo_metallic_view: wgpu::TextureView,

    pub normal_roughness: wgpu::Texture,
    pub normal_roughness_view: wgpu::TextureView,

    pub emissive: wgpu::Texture,
    pub emissive_view: wgpu::TextureView,

    pub depth: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
}

impl Resource for GeometryPassOutputs {
    fn instanciate(resources: &ResourcesManager) -> Self {
        let device = resources.read::<wgpu::Device>();
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();

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
        let albedo_metallic_view = albedo_metallic.create_view(&Default::default());

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
        let normal_roughness_view = normal_roughness.create_view(&Default::default());

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
        let emissive_view = emissive.create_view(&Default::default());

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
        let depth_view = depth.create_view(&Default::default());

        GeometryPassOutputs {
            albedo_metallic,
            albedo_metallic_view,

            normal_roughness,
            normal_roughness_view,

            emissive,
            emissive_view,

            depth,
            depth_view,
        }
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();

        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };

        if self.depth.size() != size {
            *self = Self::instanciate(resources);
        }

        Ok(())
    }
}

use cull::*;
mod cull {
    use crate::{
        Camera, GpuMeshInstance, MeshInfo, MeshInstancesManager, MeshesManager,
        ProfilerCommandEncoder, ResourcesManager, UniformBuffer,
    };

    use super::DrawInstance;

    pub struct GeometryCull {
        resources: ResourcesManager,

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
        pub fn new(resources: &ResourcesManager) -> Self {
            let resources = resources.clone();
            let device = resources.read::<wgpu::Device>();
            let camera = resources.read::<UniformBuffer<Camera>>();
            let meshes = resources.read::<MeshesManager>();
            let mesh_instances = resources.read::<MeshInstancesManager>();

            let draw_instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Geometry[cull] draw instances"),
                size: (std::mem::size_of::<[DrawInstance; MeshInstancesManager::MAX_INSTANCES]>())
                    as _,
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
                        [wgpu::util::DrawIndexedIndirectArgs; MeshesManager::MAX_MESHES],
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
                                        + GpuMeshInstance::SIZE,
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
                                        + std::mem::size_of::<wgpu::util::DrawIndexedIndirectArgs>()
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
                        resource: mesh_instances.base_instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: mesh_instances.instances.as_entire_binding(),
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

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Geometry[cull] shader"),
                source: wgpu::ShaderSource::Wgsl(
                    wesl::include_wesl!("passes::geometry[cull]").into(),
                ),
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Geometry[cull] pipeline layout"),
                bind_group_layouts: &[Some(&camera.bind_group_layout), Some(&bind_group_layout)],
                immediate_size: 0,
            });

            let pipelines = (
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Geometry[cull] reset pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("reset"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Geometry[cull] cull pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("cull"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Geometry[cull] count pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("count"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            );

            Self {
                resources,

                draw_instances,
                draw_indirects,

                bind_group,
                pipelines,
            }
        }

        pub fn cull(&self, encoder: &mut ProfilerCommandEncoder) {
            let camera = self.resources.read::<UniformBuffer<Camera>>();

            let mut cpass = encoder.scoped_compute_pass("Geometry[cull]");

            const WORKGROUP_SIZE: u32 = 32;

            let meshes_count = self.resources.read::<MeshesManager>().count();
            let meshes_workgroups_count =
                (meshes_count as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

            let instances_count = self.resources.read::<MeshInstancesManager>().count();
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
