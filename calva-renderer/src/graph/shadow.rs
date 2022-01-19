use crate::{
    Mesh, MeshInstances, RenderContext, Renderer, Skin, SkinAnimationInstances, SkinAnimations,
};

pub type DrawCallArgs<'a> = (
    &'a MeshInstances,
    &'a Mesh,
    Option<&'a Skin>,
    Option<&'a SkinAnimationInstances>,
    Option<&'a SkinAnimations>,
);

pub struct ShadowLight {
    uniform: uniform::ShadowLightUniform,
    depth_pass: depth::ShadowLightDepth,
    blur_pass: blur::ShadowLightBlur,

    render_bundle: wgpu::RenderBundle,
}

impl ShadowLight {
    const CASCADES: usize = 3;

    const TEXTURE_SIZE: wgpu::Extent3d = wgpu::Extent3d {
        width: 1024,
        height: 1024,
        depth_or_array_layers: Self::CASCADES as _,
    };

    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

    pub fn new(
        renderer: &Renderer,
        albedo_metallic: &wgpu::TextureView,
        normal_roughness: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> Self {
        let Renderer {
            device,
            surface_config,
            config,
            camera,
            ..
        } = renderer;

        let uniform = uniform::ShadowLightUniform::new(device);
        let depth_pass = depth::ShadowLightDepth::new(device, &uniform);
        let blur_pass = blur::ShadowLightBlur::new(device, &depth_pass.depth);

        let shadows_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("ShadowLight shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ShadowLight bind group layout"),
            entries: &[
                // albedo + metallic
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // normal + roughness
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: Renderer::MULTISAMPLE_STATE.count > 1,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
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
                        view_dimension: wgpu::TextureViewDimension::D2Array,
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ShadowLight bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(albedo_metallic),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(normal_roughness),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&depth_pass.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&shadows_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ShadowLight pipeline layout"),
            bind_group_layouts: &[
                &config.bind_group_layout,
                &camera.bind_group_layout,
                &uniform.bind_group_layout,
                &bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ShadowLight pipeline"),
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
                targets: &[wgpu::ColorTargetState {
                    format: surface_config.format,
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
                }],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Renderer::MULTISAMPLE_STATE,
        });

        let render_bundle = {
            let mut encoder =
                device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                    label: Some("ShadowLight render bundle encoder"),
                    color_formats: &[surface_config.format],
                    depth_stencil: None,
                    sample_count: Renderer::MULTISAMPLE_STATE.count,
                    multiview: None,
                });

            encoder.set_pipeline(&pipeline);
            encoder.set_bind_group(0, &config.bind_group, &[]);
            encoder.set_bind_group(1, &camera.bind_group, &[]);
            encoder.set_bind_group(2, &uniform.bind_group, &[]);
            encoder.set_bind_group(3, &bind_group, &[]);

            encoder.draw(0..3, 0..1);

            encoder.finish(&wgpu::RenderBundleDescriptor {
                label: Some("ShadowLight render bundle"),
            })
        };

        Self {
            uniform,
            depth_pass,
            blur_pass,
            render_bundle,
        }
    }

    pub fn render<'ctx, 'data: 'ctx>(
        &self,
        ctx: &'ctx mut RenderContext,
        splits: [f32; Self::CASCADES],
        light_dir: glam::Vec3,
        cb: impl FnOnce(&mut dyn FnMut(DrawCallArgs<'data>)),
    ) {
        ctx.encoder.push_debug_group("ShadowLight");

        self.uniform.update_buffer(
            &ctx.renderer.queue,
            ctx.renderer.camera.view,
            ctx.renderer.camera.proj,
            splits,
            light_dir,
        );

        self.depth_pass.render(ctx, &self.uniform, cb);

        self.blur_pass.render(ctx, &self.depth_pass.depth);
        self.blur_pass.render(ctx, &self.depth_pass.depth);

        ctx.encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ShadowLight lighting pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: ctx.view,
                    resolve_target: ctx.resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            })
            .execute_bundles(std::iter::once(&self.render_bundle));

        ctx.encoder.pop_debug_group();
    }
}

mod uniform {
    const CASCADES: usize = super::ShadowLight::CASCADES;

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
    struct ShadowLightUniformRaw {
        color: [f32; 4],
        direction: [f32; 4], // camera view space
        view_proj: [[f32; 16]; CASCADES],
        splits: [f32; CASCADES],

        _padding: [f32; 4 - CASCADES % 4],
    }

    impl ShadowLightUniformRaw {
        pub fn new(
            color: glam::Vec4,
            direction: glam::Vec4,
            view_proj: Vec<glam::Mat4>,
            splits: [f32; CASCADES],
        ) -> Self {
            let view_proj = TryFrom::try_from(
                view_proj[0..CASCADES]
                    .iter()
                    .map(glam::Mat4::to_cols_array)
                    .collect::<Vec<_>>(),
            )
            .unwrap();

            Self {
                color: color.to_array(),
                direction: direction.to_array(),
                view_proj,
                splits,
                ..Default::default()
            }
        }
    }

    impl ShadowLightUniformRaw {}

    pub struct ShadowLightUniform {
        buffer: wgpu::Buffer,

        pub bind_group_layout: wgpu::BindGroupLayout,
        pub bind_group: wgpu::BindGroup,
    }

    impl ShadowLightUniform {
        pub fn new(device: &wgpu::Device) -> Self {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ShadowLightUniform buffer"),
                size: std::mem::size_of::<ShadowLightUniformRaw>() as _,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("ShadowLightUniform bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ShadowLightUniform bind group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

            Self {
                buffer,
                bind_group_layout,
                bind_group,
            }
        }

        pub fn update_buffer(
            &self,
            queue: &wgpu::Queue,
            camera_view: glam::Mat4,
            camera_proj: glam::Mat4,
            mut splits: [f32; CASCADES],
            light_dir: glam::Vec3,
        ) {
            #[rustfmt::skip]
            const CAMERA_FRUSTRUM: [glam::Vec3; 8] = [
                // near
                glam::const_vec3!([-1.0,  1.0, 0.0]), // top left
                glam::const_vec3!([ 1.0,  1.0, 0.0]), // top right
                glam::const_vec3!([-1.0, -1.0, 0.0]), // bottom left
                glam::const_vec3!([ 1.0, -1.0, 0.0]), // bottom right
                // far
                glam::const_vec3!([-1.0,  1.0, 1.0]), // top left
                glam::const_vec3!([ 1.0,  1.0, 1.0]), // top right
                glam::const_vec3!([-1.0, -1.0, 1.0]), // bottom left
                glam::const_vec3!([ 1.0, -1.0, 1.0]), // bottom right
            ];

            let light_dir = light_dir.normalize();
            let light_view = glam::Mat4::look_at_rh(glam::Vec3::ZERO, light_dir, glam::Vec3::Y);

            let transform = light_view * (camera_proj * camera_view).inverse();

            for split in &mut splits {
                let v = camera_proj * glam::vec4(0.0, 0.0, -*split, 1.0);
                *split = (v.z / v.w) * 0.5 + 0.5
            }

            let split_transforms = (0..CASCADES)
                .map(|cascade_index| {
                    let corners = CAMERA_FRUSTRUM
                        .iter()
                        .map(|v| {
                            let mut v = *v;

                            v.z = cascade_index
                                .checked_sub(if v.z <= 0.0 { 1 } else { 0 })
                                .map(|idx| splits[idx])
                                .unwrap_or(0.0);

                            let v = transform * v.extend(1.0);
                            v.truncate() / v.w
                        })
                        .collect::<Vec<_>>();

                    // Frustrum center in world space
                    let mut center = corners.iter().fold(glam::Vec3::ZERO, |acc, &v| acc + v)
                        / corners.len() as f32;

                    // Radius of the camera frustrum slice bounding sphere
                    let mut radius = corners
                        .iter()
                        .fold(0.0_f32, |acc, &v| acc.max(v.distance(center)));

                    // Avoid shadow swimming:
                    // Prevent small radius changes due to float precision
                    radius = (radius * 16.0).ceil() / 16.0;
                    // Shadow texel size in light view space
                    let texel_size = radius * 2.0 / super::ShadowLight::TEXTURE_SIZE.width as f32;
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

                    light_proj * light_view
                })
                .collect::<Vec<_>>();

            let light_dir_view_space = glam::Quat::from_mat4(&camera_view) * light_dir;

            queue.write_buffer(
                &self.buffer,
                0,
                bytemuck::bytes_of(&ShadowLightUniformRaw::new(
                    glam::Vec4::ONE,
                    light_dir_view_space.extend(1.0),
                    split_transforms,
                    splits,
                )),
            );
        }
    }
}

mod depth {
    use super::{uniform::ShadowLightUniform, DrawCallArgs, ShadowLight};
    use crate::{Instance, MeshInstance, RenderContext, SkinAnimationInstance, SkinAnimations};

    pub struct ShadowLightDepth {
        pub depth: wgpu::TextureView,

        simple_mesh_pipeline: wgpu::RenderPipeline,
        skinned_mesh_pipeline: wgpu::RenderPipeline,
    }

    impl ShadowLightDepth {
        pub fn new(device: &wgpu::Device, uniform: &ShadowLightUniform) -> Self {
            let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("ShadowLightDepth texture"),
                size: ShadowLight::TEXTURE_SIZE,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: ShadowLight::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            });

            let depth = depth_texture.create_view(&wgpu::TextureViewDescriptor {
                aspect: wgpu::TextureAspect::DepthOnly,
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: core::num::NonZeroU32::new(ShadowLight::CASCADES as _),
                ..Default::default()
            });

            let simple_mesh_pipeline = {
                let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                    label: Some("ShadowLight[simple] depth shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("shaders/shadow.simple.wgsl").into(),
                    ),
                });

                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("ShadowLight[simple] depth render pipeline layout"),
                        bind_group_layouts: &[&uniform.bind_group_layout],
                        push_constant_ranges: &[],
                    });

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("ShadowLight[simple] depth render pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[
                            MeshInstance::LAYOUT,
                            // Positions
                            wgpu::VertexBufferLayout {
                                array_stride: (std::mem::size_of::<f32>() * 3) as _,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![5 => Float32x3],
                            },
                        ],
                    },
                    fragment: None,
                    primitive: wgpu::PrimitiveState {
                        unclipped_depth: true,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: ShadowLight::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState {
                            constant: 2, // corresponds to bilinear filtering
                            slope_scale: 2.0,
                            clamp: 0.0,
                        },
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: core::num::NonZeroU32::new(ShadowLight::CASCADES as _),
                })
            };

            let skinned_mesh_pipeline = {
                let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                    label: Some("ShadowLight[skinned] depth shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("shaders/shadow.skinned.wgsl").into(),
                    ),
                });

                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("ShadowLight[skinned] depth render pipeline layout"),
                        bind_group_layouts: &[
                            &uniform.bind_group_layout,
                            &device.create_bind_group_layout(SkinAnimations::DESC),
                        ],
                        push_constant_ranges: &[],
                    });

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("ShadowLight[skinned] depth render pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[
                            MeshInstance::LAYOUT,
                            SkinAnimationInstance::LAYOUT,
                            // Positions
                            wgpu::VertexBufferLayout {
                                array_stride: (std::mem::size_of::<f32>() * 3) as _,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![6 => Float32x3],
                            },
                            // Joints
                            wgpu::VertexBufferLayout {
                                array_stride: (std::mem::size_of::<u32>()) as _,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![7 => Uint32],
                            },
                            // Weights
                            wgpu::VertexBufferLayout {
                                array_stride: (std::mem::size_of::<f32>() * 4) as _,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![8 => Float32x4],
                            },
                        ],
                    },
                    fragment: None,
                    primitive: wgpu::PrimitiveState {
                        unclipped_depth: true,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: ShadowLight::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState {
                            constant: 2, // corresponds to bilinear filtering
                            slope_scale: 2.0,
                            clamp: 0.0,
                        },
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: core::num::NonZeroU32::new(ShadowLight::CASCADES as _),
                })
            };

            Self {
                depth,

                simple_mesh_pipeline,
                skinned_mesh_pipeline,
            }
        }

        pub fn render<'ctx, 'data: 'ctx>(
            &self,
            ctx: &'ctx mut RenderContext,
            uniform: &ShadowLightUniform,
            cb: impl FnOnce(&mut dyn FnMut(DrawCallArgs<'data>)),
        ) {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ShadowLightDepth pass"),
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

            cb(
                &mut move |(mesh_instances, mesh, skin, animation_instances, animation): DrawCallArgs| {
                    if mesh_instances.count() == 0 { return; }

                    rpass.set_pipeline(match skin {
                        Some(_) => &self.skinned_mesh_pipeline,
                        None => &self.simple_mesh_pipeline,
                    });

                    rpass.set_bind_group(0, &uniform.bind_group, &[]);

                    if let Some(animation) = animation {
                        rpass.set_bind_group(1, &animation.bind_group, &[]);
                    }


                    let mut idx_iter = 0..;
                    macro_rules! idx {
                        () => {
                            idx_iter.next().unwrap()
                        };
                    }

                    rpass.set_vertex_buffer(idx!(), mesh_instances.buffer.slice(..));
                    if let Some(animation_instances) = animation_instances {
                        rpass.set_vertex_buffer(idx!(), animation_instances.buffer.slice(..));
                    }

                    rpass.set_vertex_buffer(idx!(), mesh.vertices.slice(..));

                    if let Some(skin) = skin {
                        rpass.set_vertex_buffer(idx!(), skin.joint_indices.slice(..));
                        rpass.set_vertex_buffer(idx!(), skin.joint_weights.slice(..));
                    }

                    rpass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint16);
                    rpass.draw_indexed(0..mesh.num_elements, 0, 0..mesh_instances.count());
                },
            );
        }
    }
}

mod blur {
    use super::ShadowLight;
    use crate::RenderContext;

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

    pub struct ShadowLightBlur {
        temp: wgpu::TextureView,

        h_render_bundle: wgpu::RenderBundle,
        v_render_bundle: wgpu::RenderBundle,
    }

    impl ShadowLightBlur {
        pub fn new(device: &wgpu::Device, output: &wgpu::TextureView) -> Self {
            let temp = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("ShadowLightBlur temp texture"),
                    size: ShadowLight::TEXTURE_SIZE,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: ShadowLight::DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                })
                .create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    array_layer_count: core::num::NonZeroU32::new(ShadowLight::CASCADES as _),
                    ..Default::default()
                });

            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("ShadowLightBlur shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.blur.wgsl").into()),
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("ShadowLightBlur bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            sample_type: wgpu::TextureSampleType::Depth,
                        },
                        count: None,
                    }],
                });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ShadowLightBlur pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let make_render_bundle = |direction: Direction| {
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(format!("ShadowLightBlur {} bind group", direction).as_str()),
                    layout: &bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(match direction {
                            Direction::Horizontal => output,
                            Direction::Vertical => &temp,
                        }),
                    }],
                });

                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(format!("ShadowLightBlur {} pipeline", direction).as_str()),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: format!("fs_main_{}", direction).as_str(),
                        targets: &[],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: ShadowLight::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Always,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: core::num::NonZeroU32::new(ShadowLight::CASCADES as _),
                });

                let mut encoder =
                    device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                        label: Some(
                            format!("ShadowLightBlur {} render bundle encoder", direction).as_str(),
                        ),
                        color_formats: &[],
                        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                            format: ShadowLight::DEPTH_FORMAT,
                            depth_read_only: false,
                            stencil_read_only: false,
                        }),
                        sample_count: 1,
                        multiview: core::num::NonZeroU32::new(ShadowLight::CASCADES as _),
                    });

                encoder.set_pipeline(&pipeline);
                encoder.set_bind_group(0, &bind_group, &[]);

                encoder.draw(0..3, 0..1);

                encoder.finish(&wgpu::RenderBundleDescriptor {
                    label: Some(format!("ShadowLightBlur {} render bundle", direction).as_str()),
                })
            };

            let h_render_bundle = make_render_bundle(Direction::Horizontal);
            let v_render_bundle = make_render_bundle(Direction::Vertical);

            Self {
                temp,

                h_render_bundle,
                v_render_bundle,
            }
        }

        pub fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ShadowLightBlur horizontal pass"),
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
                .execute_bundles(std::iter::once(&self.h_render_bundle));

            ctx.encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ShadowLightBlur vertical pass"),
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
                .execute_bundles(std::iter::once(&self.v_render_bundle));
        }
    }
}
