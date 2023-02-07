use crate::{
    AnimationsManager, CameraManager, CullOutput, DirectionalLight, GeometryPass, GpuMeshInstance,
    InstancesManager, MeshesManager, RenderContext, Renderer, SkinsManager,
};

pub struct DirectionalLightPass {
    cull_output: CullOutput,

    uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,

    depth: wgpu::TextureView,
    depth_pipeline: wgpu::RenderPipeline,

    blur_pass: blur::DirectionalLightBlur,

    shadow_bind_group_layout: wgpu::BindGroupLayout,
    shadow_bind_group: wgpu::BindGroup,
    shadow_pipeline: wgpu::RenderPipeline,
}

impl DirectionalLightPass {
    const SIZE: u32 = 2048;
    const TEXTURE_SIZE: wgpu::Extent3d = wgpu::Extent3d {
        width: Self::SIZE,
        height: Self::SIZE,
        depth_or_array_layers: 1,
    };

    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(
        renderer: &Renderer,
        geometry: &GeometryPass,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
    ) -> Self {
        let cull_output = instances.create_cull_output(&renderer.device);

        let uniform = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DirectionalLight uniform"),
            size: DirectionalLightUniform::SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("DirectionalLight uniform bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(DirectionalLightUniform::SIZE),
                        },
                        count: None,
                    }],
                });

        let uniform_bind_group = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DirectionalLight bind group"),
                layout: &uniform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }],
            });

        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DirectionalLight sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let depth = Self::make_depth_texture(renderer, Some("DirectionalLight depth texture"))
            .create_view(&Default::default());

        let depth_pipeline = {
            let shader = renderer
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("DirectionalLight[depth] shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("directional_light.depth.wgsl").into(),
                    ),
                });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("DirectionalLight[depth] render pipeline layout"),
                        bind_group_layouts: &[
                            &uniform_bind_group_layout,
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
                    label: Some("DirectionalLight[depth] render pipeline"),
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
                        ],
                    },
                    fragment: None,
                    primitive: wgpu::PrimitiveState {
                        unclipped_depth: true,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: Self::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                })
        };

        let blur_pass = blur::DirectionalLightBlur::new(renderer, &depth);

        let (shadow_bind_group_layout, shadow_bind_group, shadow_pipeline) = {
            let shader = renderer
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("DirectionalLight[shadow] shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("directional_light.shadow.wgsl").into(),
                    ),
                });

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("DirectionalLight[shadow] bind group layout"),
                        entries: &[
                            // albedo + metallic
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                },
                                count: None,
                            },
                            // normal + roughness
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                },
                                count: None,
                            },
                            // depth
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Depth,
                                },
                                count: None,
                            },
                            // shadows
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Depth,
                                },
                                count: None,
                            },
                            // shadows sampler
                            wgpu::BindGroupLayoutEntry {
                                binding: 4,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    });

            let bind_group = Self::make_shadow_bind_group(
                renderer,
                geometry,
                &bind_group_layout,
                &depth,
                &sampler,
            );

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("DirectionalLight[shadow] pipeline layout"),
                        bind_group_layouts: &[
                            &renderer.camera.bind_group_layout,
                            &uniform_bind_group_layout,
                            &bind_group_layout,
                        ],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStages::FRAGMENT,
                            range: 0..(std::mem::size_of::<f32>() as _),
                        }],
                    });

            let pipeline =
                renderer
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("DirectionalLight[shadow] pipeline"),
                        layout: Some(&pipeline_layout),
                        multiview: None,
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: "vs_main",
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: "fs_main",
                            targets: &[Some(wgpu::ColorTargetState {
                                format: renderer.surface_config.format,
                                blend: Some(wgpu::BlendState {
                                    color: wgpu::BlendComponent {
                                        src_factor: wgpu::BlendFactor::One,
                                        dst_factor: wgpu::BlendFactor::One,
                                        operation: wgpu::BlendOperation::Add,
                                    },
                                    alpha: wgpu::BlendComponent {
                                        src_factor: wgpu::BlendFactor::One,
                                        dst_factor: wgpu::BlendFactor::One,
                                        operation: wgpu::BlendOperation::Max,
                                    },
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: Default::default(),
                        depth_stencil: None,
                        multisample: Renderer::MULTISAMPLE_STATE,
                    });

            (bind_group_layout, bind_group, pipeline)
        };

        Self {
            cull_output,

            uniform,
            uniform_bind_group,
            sampler,

            depth,
            depth_pipeline,

            blur_pass,

            shadow_bind_group_layout,
            shadow_bind_group,
            shadow_pipeline,
        }
    }

    pub fn rebind(&mut self, renderer: &Renderer, geometry: &GeometryPass) {
        self.shadow_bind_group = Self::make_shadow_bind_group(
            renderer,
            geometry,
            &self.shadow_bind_group_layout,
            &self.depth,
            &self.sampler,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        ctx: &mut RenderContext,
        meshes: &MeshesManager,
        skins: &SkinsManager,
        animations: &AnimationsManager,
        instances: &InstancesManager,
        gamma: f32,
        directional_light: &DirectionalLight,
    ) {
        ctx.encoder.profile_start("DirectionalLight");

        let uniform = DirectionalLightUniform::new(ctx.camera, directional_light);

        self.cull_output.update(ctx.queue, &uniform.view_proj);
        instances.cull(&mut ctx.encoder, &self.cull_output);

        ctx.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DirectionalLight[depth]"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.depth_pipeline);

        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        rpass.set_bind_group(1, &skins.bind_group, &[]);
        rpass.set_bind_group(2, &animations.bind_group, &[]);

        rpass.set_vertex_buffer(0, self.cull_output.instances.slice(..));
        rpass.set_vertex_buffer(1, meshes.vertices.slice(..));

        rpass.set_index_buffer(meshes.indices.slice(..), wgpu::IndexFormat::Uint32);

        rpass.multi_draw_indexed_indirect_count(
            &self.cull_output.indirects,
            std::mem::size_of::<u32>() as _,
            &self.cull_output.indirects,
            0,
            MeshesManager::MAX_MESHES as _,
        );

        drop(rpass);

        self.blur_pass.render(ctx, &self.depth);

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DirectionalLight[shadow]"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.output.view,
                resolve_target: ctx.output.resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&self.shadow_pipeline);

        rpass.set_bind_group(0, &ctx.camera.bind_group, &[]);
        rpass.set_bind_group(1, &self.uniform_bind_group, &[]);
        rpass.set_bind_group(2, &self.shadow_bind_group, &[]);

        rpass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::bytes_of(&gamma));

        rpass.draw(0..3, 0..1);

        drop(rpass);

        ctx.encoder.profile_end();
    }

    fn make_shadow_bind_group(
        renderer: &Renderer,
        geometry: &GeometryPass,
        layout: &wgpu::BindGroupLayout,
        depth: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DirectionalLight[shadow] bind group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            geometry.albedo_metallic_view(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            geometry.normal_roughness_view(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&renderer.depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
    }

    fn make_depth_texture(renderer: &Renderer, label: wgpu::Label<'static>) -> wgpu::Texture {
        renderer.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: Self::TEXTURE_SIZE,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::DEPTH_FORMAT],
        })
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct DirectionalLightUniform {
    color: glam::Vec4,
    direction: glam::Vec4, // camera view space
    view_proj: glam::Mat4,
}

impl DirectionalLightUniform {
    const SIZE: wgpu::BufferAddress = std::mem::size_of::<Self>() as _;

    pub fn new(camera: &CameraManager, directional_light: &DirectionalLight) -> Self {
        let light_dir = directional_light.direction.normalize();
        let light_view = glam::Mat4::look_at_rh(glam::Vec3::ZERO, light_dir, glam::Vec3::Y);

        // Frustum bounding sphere in view space
        // https://lxjk.github.io/2017/04/15/Calculate-Minimal-Bounding-Sphere-of-Frustum.html
        // https://stackoverflow.com/questions/2194812/finding-a-minimum-bounding-sphere-for-a-frustum
        // https://stackoverflow.com/questions/56428880/how-to-extract-camera-parameters-from-projection-matrix
        let znear = camera.proj.w_axis.z / (camera.proj.z_axis.z - 1.0);
        let zfar = camera.proj.w_axis.z / (camera.proj.z_axis.z + 1.0);

        let k = f32::sqrt(1.0 + (camera.proj.x_axis.x / camera.proj.y_axis.y).powi(2))
            * camera.proj.x_axis.x.recip();
        let k2 = k.powi(2);

        let (mut center, mut radius) = if k2 >= (zfar - znear) / (zfar + znear) {
            (glam::vec3(0.0, 0.0, -zfar), zfar * k)
        } else {
            (
                glam::vec3(0.0, 0.0, -0.5 * (zfar + znear) * (1.0 + k2)),
                0.5 * f32::sqrt(
                    f32::powi(zfar - znear, 2)
                        + 2.0 * (zfar.powi(2) + znear.powi(2)) * k2
                        + f32::powi(zfar + znear, 2) * k.powi(4),
                ),
            )
        };

        // Move sphere to light view space
        center = (light_view * camera.view.inverse() * center.extend(1.0)).truncate();

        // Avoid shadow swimming:
        // Prevent small radius changes due to float precision
        radius = (radius * 16.0).ceil() / 16.0;
        // Shadow texel size in light view space
        let texel_size = radius * 2.0 / DirectionalLightPass::SIZE as f32;
        // Allow center changes only in texel size increments
        center = (center / texel_size).ceil() * texel_size;

        let min = center - glam::Vec3::splat(radius);
        let max = center + glam::Vec3::splat(radius);

        let light_proj = glam::Mat4::orthographic_rh(
            min.x,  // left
            max.x,  // right
            min.y,  // bottom
            max.y,  // top
            -max.z, // near
            -min.z, // far
        );

        Self {
            color: directional_light.color,
            direction: (glam::Quat::from_mat4(&camera.view) * light_dir).extend(0.0),
            view_proj: (light_proj * light_view),
        }
    }
}

mod blur {
    use crate::{RenderContext, Renderer};

    use super::DirectionalLightPass;

    #[derive(Clone, Copy)]
    enum Direction {
        Horizontal,
        Vertical,
    }

    impl std::fmt::Display for Direction {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self {
                Direction::Horizontal => "horizontal",
                Direction::Vertical => "vertical",
            })
        }
    }

    pub struct DirectionalLightBlur {
        temp: wgpu::TextureView,

        h_pass: wgpu::RenderBundle,
        v_pass: wgpu::RenderBundle,
    }

    impl DirectionalLightBlur {
        pub fn new(renderer: &Renderer, output: &wgpu::TextureView) -> Self {
            let temp = DirectionalLightPass::make_depth_texture(
                renderer,
                Some("DirectionalLightBlur temp texture"),
            )
            .create_view(&Default::default());

            let bind_group_layout =
                renderer
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("DirectionalLightBlur bind group layout"),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Depth,
                            },
                            count: None,
                        }],
                    });

            let pipeline_layout =
                renderer
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("DirectionalLightBlur pipeline layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let shader = renderer
                .device
                .create_shader_module(wgpu::include_wgsl!("directional_light.blur.wgsl"));

            let make_render_bundle = |direction: Direction| {
                let bind_group = renderer
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(
                            format!("DirectionalLightBlur[{direction}] bind group").as_str(),
                        ),
                        layout: &bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(match direction {
                                Direction::Horizontal => output,
                                Direction::Vertical => &temp,
                            }),
                        }],
                    });

                let pipeline =
                    renderer
                        .device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some(
                                format!("DirectionalLightBlur[{direction}] pipeline").as_str(),
                            ),
                            layout: Some(&pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &shader,
                                entry_point: "vs_main",
                                buffers: &[],
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &shader,
                                entry_point: format!("fs_main_{direction}").as_str(),
                                targets: &[],
                            }),
                            primitive: Default::default(),
                            depth_stencil: Some(wgpu::DepthStencilState {
                                format: DirectionalLightPass::DEPTH_FORMAT,
                                depth_write_enabled: true,
                                depth_compare: wgpu::CompareFunction::Always,
                                stencil: wgpu::StencilState::default(),
                                bias: wgpu::DepthBiasState::default(),
                            }),
                            multisample: Default::default(),
                            multiview: None,
                        });

                let mut encoder = renderer.device.create_render_bundle_encoder(
                    &wgpu::RenderBundleEncoderDescriptor {
                        label: Some(
                            format!("DirectionalLightBlur[{direction}] render bundle").as_str(),
                        ),
                        color_formats: &[],
                        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                            format: DirectionalLightPass::DEPTH_FORMAT,
                            depth_read_only: false,
                            stencil_read_only: false,
                        }),
                        sample_count: 1,
                        multiview: None,
                    },
                );

                encoder.set_pipeline(&pipeline);
                encoder.set_bind_group(0, &bind_group, &[]);

                encoder.draw(0..3, 0..1);

                encoder.finish(&Default::default())
            };

            let h_pass = make_render_bundle(Direction::Horizontal);
            let v_pass = make_render_bundle(Direction::Vertical);

            Self {
                temp,

                h_pass,
                v_pass,
            }
        }

        pub fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
            ctx.encoder.profile_start("DirectionalLight[blur]");

            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("DirectionalLight[blur][horizontal]"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.temp,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                })
                .execute_bundles(std::iter::once(&self.h_pass));

            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("DirectionalLight[blur][vertical]"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: output,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                })
                .execute_bundles(std::iter::once(&self.v_pass));

            ctx.encoder.profile_end();
        }
    }
}
