use crate::RenderContext;
use crate::Renderer;

pub struct GeometryBuffer {
    pub albedo: wgpu::TextureView,
    pub position: wgpu::TextureView,
    pub normal: wgpu::TextureView,
    pub depth: wgpu::TextureView,

    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl GeometryBuffer {
    pub fn new(renderer: &Renderer) -> Self {
        let Renderer {
            device,
            surface_config,
            ..
        } = renderer;

        macro_rules! make_view {
            ($format: expr, $label: expr) => {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some($label),
                        size: wgpu::Extent3d {
                            width: surface_config.width,
                            height: surface_config.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: $format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                    })
                    .create_view(&wgpu::TextureViewDescriptor::default())
            };
        }

        let albedo = make_view!(Renderer::ALBEDO_FORMAT, "GBuffer albedo texture");
        let position = make_view!(Renderer::POSITION_FORMAT, "GBuffer position texture");
        let normal = make_view!(Renderer::NORMAL_FORMAT, "GBuffer normal texture");
        let depth = make_view!(Renderer::DEPTH_FORMAT, "GBuffer depth texture");

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GBuffer bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
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
                    resource: wgpu::BindingResource::TextureView(&albedo),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&position),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&normal),
                },
            ],
        });

        Self {
            albedo,
            position,
            normal,
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
        let mut rpass = {
            ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Geometry pass"),
                color_attachments: &[
                    wgpu::RenderPassColorAttachment {
                        view: &self.albedo,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
                    },
                    wgpu::RenderPassColorAttachment {
                        view: &self.position,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
                    },
                    wgpu::RenderPassColorAttachment {
                        view: &self.normal,
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

pub trait DrawModel {
    fn draw<'ctx: 'pass, 'pass>(
        &'ctx self,
        renderer: &'ctx Renderer,
        rpass: &mut wgpu::RenderPass<'pass>,
    );
}
