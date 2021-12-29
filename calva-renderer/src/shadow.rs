use crate::{CameraUniform, Mesh, MeshInstances, RenderContext, Renderer, Skin, SkinAnimation};

pub struct ShadowLight {
    shadows: ShadowLightDepth,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl ShadowLight {
    pub fn new(
        Renderer {
            device,
            surface_config,
            config,
            camera,
            ..
        }: &Renderer,

        albedo_metallic: &wgpu::TextureView,
        normal_roughness: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> Self {
        let shadows = ShadowLightDepth::new(device);

        let shadows_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            // compare: Some(wgpu::CompareFunction::Less),
            ..Default::default()
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("ShadowLight shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.shadow.wgsl").into()),
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
                    resource: wgpu::BindingResource::TextureView(&shadows.depth),
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
                &shadows.bind_group_layout,
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

        Self {
            shadows,
            bind_group,
            pipeline,
        }
    }

    pub fn render<'ctx, 'data: 'ctx>(
        &self,
        ctx: &'ctx mut RenderContext,
        direction: glam::Vec3,
        cb: impl FnOnce(&mut dyn FnMut(DrawCallArgs<'data>)),
    ) {
        ctx.encoder.push_debug_group("ShadowLight");

        self.shadows.render(ctx, direction, cb);

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &ctx.renderer.config.bind_group, &[]);
        rpass.set_bind_group(1, &ctx.renderer.camera.bind_group, &[]);
        rpass.set_bind_group(2, &self.shadows.bind_group, &[]);
        rpass.set_bind_group(3, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);
        drop(rpass);

        ctx.encoder.pop_debug_group();
    }
}

pub type DrawCallArgs<'a> = (
    &'a MeshInstances,
    &'a Mesh,
    Option<&'a Skin>,
    Option<&'a SkinAnimation>,
);

struct ShadowLightDepth {
    depth: wgpu::TextureView,

    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,

    blur: ShadowLightBlur,
}

impl ShadowLightDepth {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
    const TEXTURE_SIZE: u32 = 1024;
    const CASCADES: usize = 4;

    pub fn new(device: &wgpu::Device) -> Self {
        let size = wgpu::Extent3d {
            width: Self::TEXTURE_SIZE,
            height: Self::TEXTURE_SIZE,
            depth_or_array_layers: Self::CASCADES as _,
        };

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ShadowLight depth texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        });

        let depth = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: core::num::NonZeroU32::new(Self::CASCADES as _),
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ShadowLight depth buffer"),
            size: std::mem::size_of::<ShadowLightUniform>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ShadowLight depth bind group layout"),
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
            label: Some("ShadowLight depth bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ShadowLight depth render pipeline layout"),
            bind_group_layouts: &[&bind_group_layout, SkinAnimation::bind_group_layout(device)],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("ShadowLight depth shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.skinned.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ShadowLight depth render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    MeshInstances::LAYOUT,
                    // Positions
                    wgpu::VertexBufferLayout {
                        array_stride: (std::mem::size_of::<f32>() * 3) as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![7 => Float32x3],
                    },
                    // Joints
                    wgpu::VertexBufferLayout {
                        array_stride: (std::mem::size_of::<u8>() * 4) as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![8 => Uint8x4],
                    },
                    // Weights
                    wgpu::VertexBufferLayout {
                        array_stride: (std::mem::size_of::<f32>() * 4) as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![9 => Float32x4],
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
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2, // corresponds to bilinear filtering
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: core::num::NonZeroU32::new(Self::CASCADES as _),
        });

        let blur = ShadowLightBlur::new(device, size, &depth);

        Self {
            depth,

            uniform_buffer,
            bind_group_layout,
            bind_group,
            pipeline,

            blur,
        }
    }

    pub fn render<'ctx, 'data: 'ctx>(
        &self,
        ctx: &'ctx mut RenderContext,
        light_dir: glam::Vec3,
        cb: impl FnOnce(&mut dyn FnMut(DrawCallArgs<'data>)),
    ) {
        ctx.renderer.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[ShadowLightUniform::new(&ctx.renderer.camera, light_dir)]),
        );

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ShadowLight depth pass"),
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

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);

        cb(
            &mut move |(instances, mesh, skin, animation): DrawCallArgs| {
                rpass.set_vertex_buffer(0, instances.buffer.slice(..));
                rpass.set_vertex_buffer(1, mesh.vertices.slice(..));

                rpass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint16);

                if let Some(skin) = skin {
                    rpass.set_bind_group(1, &animation.unwrap().bind_group, &[]);

                    rpass.set_vertex_buffer(2, skin.joint_indices.slice(..));
                    rpass.set_vertex_buffer(3, skin.joint_weights.slice(..));
                }

                rpass.draw_indexed(0..mesh.num_elements, 0, 0..instances.count());
            },
        );

        self.blur.render(ctx, &self.depth);
        self.blur.render(ctx, &self.depth);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowLightUniform {
    color: glam::Vec4,
    direction: glam::Vec4, // camera view space
    view_proj: [glam::Mat4; ShadowLightDepth::CASCADES],
    splits: [f32; ShadowLightDepth::CASCADES],
}

impl ShadowLightUniform {
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

    // https://github.com/SaschaWillems/Vulkan/blob/master/examples/shadowmappingcascade/shadowmappingcascade.cpp#L639-L716
    fn new(camera: &CameraUniform, light_dir: glam::Vec3) -> Self {
        let light_dir = light_dir.normalize();
        let light_view = glam::Mat4::look_at_rh(glam::Vec3::ZERO, light_dir, glam::Vec3::Y);

        let inv_proj = camera.proj.inverse();
        let near = inv_proj * glam::Vec3::ZERO.extend(1.0);
        let near = -near.z / near.w;
        let far = inv_proj * glam::Vec3::Z.extend(1.0);
        let far = -far.z / far.w;

        // let ratio = far / near;
        // let lambda = 0.95;
        // let mut splits = (0..ShadowLightDepth::CASCADES)
        //     .map(|cascade| {
        //         let p = (cascade + 1) as f32 / ShadowLightDepth::CASCADES as f32;
        //         let log = near * ratio.powf(p);
        //         let uniform = near + (far - near) * p;
        //         let d = lambda * (log - uniform) + uniform;
        //         1.0 - (d - near) / (far - near) / 2.0
        //     })
        //     .collect::<Vec<_>>();

        // splits.insert(0, 0.0);
        // dbg!(&splits);

        let splits = (0..=ShadowLightDepth::CASCADES)
            .map(|cascade| {
                if cascade == 0 {
                    return 0.0;
                };

                let z = cascade as f32 / ShadowLightDepth::CASCADES as f32 * (far - near);
                let v = camera.proj * glam::vec4(0.0, 0.0, -z, 1.0);
                v.z / v.w
            })
            .collect::<Vec<_>>();

        // let mut ratio = 1.0;
        // let mut splits = (0..ShadowLightDepth::CASCADES)
        //     .map(|_| {
        //         let z = ratio * (far - near);
        //         let v = camera.proj * glam::vec4(0.0, 0.0, -z, 1.0);
        //         ratio /= 2.0;

        //         v.z / v.w
        //     })
        //     .collect::<Vec<_>>();
        // splits.push(0.0);
        // splits.reverse();

        let transform = light_view * (camera.proj * camera.view).inverse();

        let split_transforms = (0..ShadowLightDepth::CASCADES)
            .map(|cascade_index| {
                let corners = Self::CAMERA_FRUSTRUM
                    .iter()
                    .map(|v| {
                        let mut v = *v;
                        v.z = splits[cascade_index + v.z as usize];
                        let v = transform * v.extend(1.0);
                        v.truncate() / v.w
                    })
                    .collect::<Vec<_>>();

                // Frustrum center in world space
                let mut center =
                    corners.iter().fold(glam::Vec3::ZERO, |acc, &v| acc + v) / corners.len() as f32;

                // Radius of the camera frustrum slice bounding sphere
                let mut radius = corners
                    .iter()
                    .fold(0.0_f32, |acc, &v| acc.max(v.distance(center)));

                // Avoid shadow swimming
                // Prevent small radius changes due to float precision
                radius = (radius * 16.0).ceil() / 16.0;
                // Shadow texel size in light view space
                let texel_size = radius * 2.0 / ShadowLightDepth::TEXTURE_SIZE as f32;
                // Center can only change in texel size increments
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

        Self {
            color: glam::Vec4::ONE,
            direction: (glam::Quat::from_mat4(&camera.view) * light_dir).extend(1.0), // use only rotation component from camera view
            view_proj: TryFrom::try_from(split_transforms).unwrap(),
            splits: TryFrom::try_from(&splits[0..ShadowLightDepth::CASCADES]).unwrap(),
        }
    }
}

struct ShadowLightBlur {
    depth: wgpu::TextureView,

    h_bind_group: wgpu::BindGroup,
    h_pipeline: wgpu::RenderPipeline,

    v_bind_group: wgpu::BindGroup,
    v_pipeline: wgpu::RenderPipeline,
}

impl ShadowLightBlur {
    fn new(device: &wgpu::Device, size: wgpu::Extent3d, output: &wgpu::TextureView) -> Self {
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("ShadowLight blur depth temp texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: ShadowLightDepth::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                array_layer_count: core::num::NonZeroU32::new(ShadowLightDepth::CASCADES as _),
                ..Default::default()
            });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("ShadowLight blur shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.blur.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ShadowLight blur bind group layout"),
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

        let (h_bind_group, h_pipeline) = {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ShadowLight blur horizontal bind group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(output),
                }],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ShadowLight blur horizontal pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ShadowLight blur horizontal pipeline"),
                layout: Some(&pipeline_layout),
                multiview: core::num::NonZeroU32::new(ShadowLightDepth::CASCADES as _),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main_horizontal",
                    targets: &[],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ShadowLightDepth::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            });

            (bind_group, pipeline)
        };

        let (v_bind_group, v_pipeline) = {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ShadowLight blur vertical bind group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&depth),
                }],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ShadowLight blur vertical pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ShadowLight blur vertical pipeline"),
                layout: Some(&pipeline_layout),
                multiview: core::num::NonZeroU32::new(ShadowLightDepth::CASCADES as _),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main_vertical",
                    targets: &[],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ShadowLightDepth::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            });

            (bind_group, pipeline)
        };

        Self {
            depth,

            h_bind_group,
            h_pipeline,

            v_bind_group,
            v_pipeline,
        }
    }

    fn render(&self, ctx: &mut RenderContext, output: &wgpu::TextureView) {
        // horizontal pass
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ShadowLight blur horizontal pass"),
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

            rpass.set_pipeline(&self.h_pipeline);
            rpass.set_bind_group(0, &self.h_bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }

        // vertical pass
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ShadowLight blur vertical pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: output,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            rpass.set_pipeline(&self.v_pipeline);
            rpass.set_bind_group(0, &self.v_bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }
    }
}
