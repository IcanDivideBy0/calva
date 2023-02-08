use crate::{
    AnimationsManager, CullOutput, GpuMeshInstance, InstancesManager, MaterialsManager,
    MeshesManager, RenderContext, Renderer, SkinsManager, TexturesManager,
};

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
    albedo_metallic: GeometryOutput,
    normal_roughness: GeometryOutput,
    cull_output: CullOutput,

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

    pub fn new(
        renderer: &Renderer,
        textures: &TexturesManager,
        materials: &MaterialsManager,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
    ) -> Self {
        let (albedo_metallic, normal_roughness) = Self::make_textures(renderer);

        let cull_output = instances.create_cull_output(&renderer.device);

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
                        &renderer.camera.bind_group_layout,
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
                        GpuMeshInstance::LAYOUT,
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
            albedo_metallic,
            normal_roughness,
            cull_output,

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
    pub fn render<'e, 'data: 'e>(
        &self,
        ctx: &mut RenderContext,
        textures: &TexturesManager,
        materials: &MaterialsManager,
        meshes: &MeshesManager,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
    ) {
        ctx.encoder.profile_start("Geometry");

        self.cull_output
            .update(ctx.queue, ctx.camera.proj * ctx.camera.view);
        instances.cull(&mut ctx.encoder, &self.cull_output, 0);

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Geometry[render]"),
            color_attachments: &[
                Some(self.albedo_metallic.attachment()),
                Some(self.normal_roughness.attachment()),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: ctx.output.depth_stencil,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.pipeline);

        rpass.set_bind_group(0, &ctx.camera.bind_group, &[]);
        rpass.set_bind_group(1, &textures.bind_group, &[]);
        rpass.set_bind_group(2, &materials.bind_group, &[]);
        rpass.set_bind_group(3, &skins.bind_group, &[]);
        rpass.set_bind_group(4, &animations.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.cull_output.instances.slice(..));
        rpass.set_vertex_buffer(1, meshes.vertices.slice(..));
        rpass.set_vertex_buffer(2, meshes.normals.slice(..));
        rpass.set_vertex_buffer(3, meshes.tangents.slice(..));
        rpass.set_vertex_buffer(4, meshes.tex_coords0.slice(..));

        rpass.set_index_buffer(meshes.indices.slice(..), wgpu::IndexFormat::Uint32);

        rpass.multi_draw_indexed_indirect_count(
            &self.cull_output.indirects,
            std::mem::size_of::<u32>() as _,
            &self.cull_output.indirects,
            0,
            MeshesManager::MAX_MESHES as _,
        );

        drop(rpass);

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
