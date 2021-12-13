use glam::swizzles::*;

use crate::CameraUniform;
use crate::DrawModel;
use crate::MeshInstances;
use crate::RenderContext;
use crate::Renderer;

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
        ssao: &wgpu::TextureView,
    ) -> Self {
        let shadows = ShadowLightDepth::new(device);

        let shadows_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::Less),
            ..Default::default()
        });

        let vsm_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
                // ssao
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // shadows
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                // vsm
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // vsm sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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
                    resource: wgpu::BindingResource::TextureView(ssao),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&shadows.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&shadows_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&shadows.variance),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&vsm_sampler),
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

    pub fn render<'m>(
        &self,
        ctx: &mut RenderContext,
        direction: glam::Vec3,
        models: impl IntoIterator<Item = &'m Box<dyn DrawModel>>,
    ) {
        ctx.encoder.push_debug_group("ShadowLight");

        self.shadows.render(ctx, direction, models);

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

struct ShadowLightDepth {
    depth: wgpu::TextureView,
    variance: wgpu::TextureView,

    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,

    blur: ShadowLightBlur,
}

impl ShadowLightDepth {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
    const VARIANCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;
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

        let variance_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ShadowLight variance texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::VARIANCE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        });

        let variance = variance_texture.create_view(&wgpu::TextureViewDescriptor {
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
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("ShadowLight depth shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ShadowLight depth render pipeline"),
            layout: Some(&pipeline_layout),
            multiview: core::num::NonZeroU32::new(Self::CASCADES as _),
            // multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    MeshInstances::DESC,
                    // Positions
                    wgpu::VertexBufferLayout {
                        array_stride: (std::mem::size_of::<f32>() * 3) as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![7 => Float32x3],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: Self::VARIANCE_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                // cull_mode: Some(wgpu::Face::Front),
                unclipped_depth: true,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
                // bias: wgpu::DepthBiasState {
                //     constant: 2, // corresponds to bilinear filtering
                //     slope_scale: 2.0,
                //     clamp: 0.0,
                // },
            }),
            multisample: wgpu::MultisampleState::default(),
        });

        let blur = ShadowLightBlur::new(device, size, &variance);

        Self {
            depth,
            variance,

            uniform_buffer,
            bind_group_layout,
            bind_group,
            pipeline,

            blur,
        }
    }

    pub fn render<'m>(
        &self,
        ctx: &mut RenderContext,
        light_dir: glam::Vec3,
        models: impl IntoIterator<Item = &'m Box<dyn DrawModel>>,
    ) {
        ctx.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[ShadowLightUniform::new(&ctx.renderer.camera, light_dir)]),
        );

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ShadowLight depth pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &self.variance,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: true,
                },
            }],
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

        for model in models {
            for mesh in model.meshes() {
                let instances = mesh.instances();
                let count = mesh.instances().count();

                rpass.set_vertex_buffer(0, instances.buffer.slice(..));

                for p in mesh.primitives() {
                    rpass.set_vertex_buffer(1, p.vertices().slice(..));
                    rpass.set_index_buffer(p.indices().slice(..), wgpu::IndexFormat::Uint16);

                    rpass.draw_indexed(0..p.num_elements(), 0, 0..count)
                }
            }
        }

        drop(rpass);

        self.blur.render(ctx, &self.variance);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowLightUniform {
    color: glam::Vec4,
    direction: glam::Vec4, // camera view space
    view: [glam::Mat4; ShadowLightDepth::CASCADES],
    proj: [glam::Mat4; ShadowLightDepth::CASCADES],
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

        let cam_inv = (camera.proj * camera.view).inverse();

        let mut view = Vec::with_capacity(ShadowLightDepth::CASCADES);
        let mut proj = Vec::with_capacity(ShadowLightDepth::CASCADES);

        // From world to view space (rotation only)
        let light_transform = glam::Mat4::look_at_rh(glam::Vec3::ZERO, light_dir, glam::Vec3::Y);

        for cascade_index in 0..ShadowLightDepth::CASCADES {
            let corners = Self::CAMERA_FRUSTRUM
                .iter()
                .map(|v| {
                    let mut v = *v;
                    v.z = splits[cascade_index + v.z as usize];
                    let v = cam_inv * v.extend(1.0);
                    v.xyz() / v.w
                })
                .collect::<Vec<_>>();

            // Frustrum center in world space
            let center =
                corners.iter().fold(glam::Vec3::ZERO, |acc, &v| acc + v) / corners.len() as f32;

            // Radius of the camera frustrum slice bounding sphere
            let mut radius = corners
                .iter()
                .fold(0.0_f32, |acc, &v| acc.max(v.distance(center)));

            // Avoid shadow swimming
            let translation = {
                // Prevent small radius changes due to float precision
                radius = (radius * 16.0).ceil() / 16.0;

                // Move frustum center to view space (orientation only)
                let center = light_transform * center.extend(1.0);
                // Shadow texel size in light view space
                let texel_size = (radius * 2.0) / ShadowLightDepth::TEXTURE_SIZE as f32;
                // Center can only change in texel size increments
                let mut center = (center / texel_size).ceil() * texel_size;
                // Move center back so the near ortho plane is at 0
                center.z += radius;

                glam::Mat4::from_translation(-center.xyz())
            };

            view.push(translation * light_transform);

            // We want to keep the near plane at 0, so it’s easy to get the distance in
            // fragment shader during lighting phase. Usiing distance instead of depth
            // prevent a lot of visual articacts due to float precision.
            proj.push(glam::Mat4::orthographic_rh(
                -radius,      // left
                radius,       // right
                -radius,      // bottom
                radius,       // top
                0.0,          // near
                2.0 * radius, // far
            ));
        }

        Self {
            color: glam::Vec3::ONE.extend(0.2),
            direction: (glam::Quat::from_mat4(&camera.view) * light_dir).extend(1.0), // use only rotation component from camera view
            view: TryFrom::try_from(view).unwrap(),
            proj: TryFrom::try_from(proj).unwrap(),
            splits: TryFrom::try_from(&splits[0..ShadowLightDepth::CASCADES]).unwrap(),
        }
    }
}

struct ShadowLightBlur {
    view: wgpu::TextureView,

    h_bind_group: wgpu::BindGroup,
    h_pipeline: wgpu::RenderPipeline,

    v_bind_group: wgpu::BindGroup,
    v_pipeline: wgpu::RenderPipeline,
}

impl ShadowLightBlur {
    fn new(device: &wgpu::Device, size: wgpu::Extent3d, output: &wgpu::TextureView) -> Self {
        let view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("ShadowLight blur temp texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: ShadowLightDepth::VARIANCE_FORMAT,
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
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
                    targets: &[wgpu::ColorTargetState {
                        format: ShadowLightDepth::VARIANCE_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
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
                    resource: wgpu::BindingResource::TextureView(&view),
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
                    targets: &[wgpu::ColorTargetState {
                        format: ShadowLightDepth::VARIANCE_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            });

            (bind_group, pipeline)
        };

        Self {
            view,

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
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&self.h_pipeline);
            rpass.set_bind_group(0, &self.h_bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }

        // vertical pass
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ShadowLight blur vertical pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: output,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&self.v_pipeline);
            rpass.set_bind_group(0, &self.v_bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }
    }
}
