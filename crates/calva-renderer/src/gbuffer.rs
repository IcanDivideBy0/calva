use crate::RenderContext;
use crate::Renderer;

pub struct GeometryBuffer {
    pub albedo_metallic: wgpu::TextureView,
    pub normal_roughness: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth: wgpu::TextureView,

    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl GeometryBuffer {
    pub const ALBEDO_METALLIC_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    pub const NORMAL_ROUGHNESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub const RENDER_TARGETS: &'static [wgpu::ColorTargetState] = &[
        wgpu::ColorTargetState {
            format: Self::ALBEDO_METALLIC_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        },
        wgpu::ColorTargetState {
            format: Self::NORMAL_ROUGHNESS_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        },
    ];

    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        macro_rules! texture {
            ($label: expr, $format: expr $(, $usages: expr)?) => {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some($label),
                    size: wgpu::Extent3d {
                        width: surface_config.width,
                        height: surface_config.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: Renderer::MULTISAMPLE_STATE.count,
                    dimension: wgpu::TextureDimension::D2,
                    format: $format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        $(| $usages)?
                })
            };
        }

        let albedo_metallic = texture!(
            "GBuffer albedo/metallic texture",
            Self::ALBEDO_METALLIC_FORMAT
        )
        .create_view(&wgpu::TextureViewDescriptor::default());

        let normal_roughness = texture!(
            "GBuffer normal/roughness texture",
            Self::NORMAL_ROUGHNESS_FORMAT
        )
        .create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = texture!(
            "GBuffer depth texture",
            Renderer::DEPTH_FORMAT,
            wgpu::TextureUsages::COPY_SRC
        );

        let depth = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GBuffer bind group layout"),
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
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GBuffer bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&albedo_metallic),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&normal_roughness),
                },
            ],
        });

        Self {
            albedo_metallic,
            normal_roughness,

            depth_texture,
            depth,

            bind_group_layout,
            bind_group,
        }
    }

    pub fn render<'a>(
        &self,
        ctx: &mut RenderContext,
        models: impl IntoIterator<Item = &'a Box<dyn DrawModel>>,
    ) {
        {
            let mut rpass = {
                ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("GeometryPass"),
                    color_attachments: &[
                        wgpu::RenderPassColorAttachment {
                            view: &self.albedo_metallic,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: true,
                            },
                        },
                        wgpu::RenderPassColorAttachment {
                            view: &self.normal_roughness,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: true,
                            },
                        },
                    ],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                })
            };

            for model in models {
                model.draw(ctx.renderer, &mut rpass);
            }
        }
    }

    pub fn blit_depth(&self, ctx: &mut RenderContext, dst: &wgpu::Texture) {
        ctx.encoder.copy_texture_to_texture(
            self.depth_texture.as_image_copy(),
            dst.as_image_copy(),
            wgpu::Extent3d {
                width: ctx.renderer.surface_config.width,
                height: ctx.renderer.surface_config.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

pub trait DrawModel {
    fn draw<'ctx: 'pass, 'pass>(
        &'ctx self,
        renderer: &'ctx Renderer,
        rpass: &mut wgpu::RenderPass<'pass>,
    );
}
