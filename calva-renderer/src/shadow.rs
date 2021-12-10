use wgpu::util::DeviceExt;

use crate::Camera;
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
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                entry_point: "main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "main",
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
        self.shadows.render(ctx, direction, models);

        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ShadowLight render pass"),
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
    }
}

struct ShadowLightDepth {
    depth: wgpu::TextureView,

    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl ShadowLightDepth {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

    const SIZE: u32 = 512;

    #[rustfmt::skip]
    const CAMERA_FRUSTRUM: [glam::Vec4; 8] = [
        // near
        glam::const_vec4!([-1.0,  1.0, 0.0, 1.0]), // top left
        glam::const_vec4!([ 1.0,  1.0, 0.0, 1.0]), // top right
        glam::const_vec4!([-1.0, -1.0, 0.0, 1.0]), // bottom left
        glam::const_vec4!([ 1.0, -1.0, 0.0, 1.0]), // bottom right
        // far
        glam::const_vec4!([-1.0,  1.0, 1.0, 1.0]), // top left
        glam::const_vec4!([ 1.0,  1.0, 1.0, 1.0]), // top right
        glam::const_vec4!([-1.0, -1.0, 1.0, 1.0]), // bottom left
        glam::const_vec4!([ 1.0, -1.0, 1.0, 1.0]), // bottom right
    ];

    pub fn new(device: &wgpu::Device) -> Self {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ShadowLight depth texture"),
            size: wgpu::Extent3d {
                width: Self::SIZE,
                height: Self::SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        });

        let depth = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ShadowLight depth buffer"),
            contents: bytemuck::cast_slice(&[glam::Mat4::default(); 3]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "main",
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
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Front),
                unclipped_depth: true,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                // bias: wgpu::DepthBiasState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2, // corresponds to bilinear filtering
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
        });

        Self {
            depth,

            uniform_buffer,
            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn render<'m>(
        &self,
        ctx: &mut RenderContext,
        light_dir: glam::Vec3,
        models: impl IntoIterator<Item = &'m Box<dyn DrawModel>>,
    ) {
        let (light_view, light_proj) =
            Self::get_frustrum_light_bounds(&ctx.renderer.camera, light_dir);

        ctx.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[light_view, light_proj, light_proj * light_view]),
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
    }

    fn get_frustrum_light_bounds(
        camera: &Camera,
        light_dir: glam::Vec3,
    ) -> (glam::Mat4, glam::Mat4) {
        use glam::swizzles::*;

        // https://github.com/SaschaWillems/Vulkan/blob/master/examples/shadowmappingcascade/shadowmappingcascade.cpp#L666-L709
        let ndc_to_world = (camera.proj * camera.view).inverse();

        let frustrum_corners = Self::CAMERA_FRUSTRUM
            .iter()
            .map(|&v| {
                let v = ndc_to_world * v;
                v.xyz() / v.w
            })
            .collect::<Vec<_>>();

        let frustrum_center = frustrum_corners
            .iter()
            .fold(glam::Vec3::ZERO, |acc, &v| acc + v)
            / frustrum_corners.len() as f32;

        let mut radius = frustrum_corners
            .iter()
            .fold(0.0_f32, |acc, &v| acc.max((v - frustrum_center).length()));
        radius = (radius * 16.0).ceil() / 16.0;

        let light_view = glam::Mat4::look_at_rh(
            frustrum_center - light_dir.normalize() * radius,
            frustrum_center,
            glam::Vec3::Y,
        );

        let light_proj = glam::Mat4::orthographic_rh(
            -radius,      // left
            radius,       // right
            -radius,      // bottom
            radius,       // top
            0.0,          // near
            2.0 * radius, // far
        );

        (light_view, light_proj)
    }
}
