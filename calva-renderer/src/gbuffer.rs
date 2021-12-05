use crate::RenderContext;
use crate::Renderer;

pub struct GeometryBuffer {
    pub albedo_metallic: wgpu::TextureView,
    pub normal_roughness: wgpu::TextureView,

    pub depth_texture: wgpu::Texture,
    pub depth: wgpu::TextureView,
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

    pub fn new(renderer: &Renderer) -> Self {
        let size = wgpu::Extent3d {
            width: renderer.surface_config.width,
            height: renderer.surface_config.height,
            depth_or_array_layers: 1,
        };

        let albedo_metallic = renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("GBuffer albedo/metallic texture"),
                size,
                mip_level_count: 1,
                sample_count: Renderer::MULTISAMPLE_STATE.count,
                dimension: wgpu::TextureDimension::D2,
                format: Self::ALBEDO_METALLIC_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let normal_roughness = renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("GBuffer normal/roughness texture"),
                size,
                mip_level_count: 1,
                sample_count: Renderer::MULTISAMPLE_STATE.count,
                dimension: wgpu::TextureDimension::D2,
                format: Self::NORMAL_ROUGHNESS_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GBuffer depth texture"),
            size,
            mip_level_count: 1,
            sample_count: Renderer::MULTISAMPLE_STATE.count,
            dimension: wgpu::TextureDimension::D2,
            format: Renderer::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        });

        let depth = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });

        Self {
            albedo_metallic,
            normal_roughness,

            depth_texture,
            depth,
        }
    }

    pub fn render<'m>(
        &self,
        ctx: &mut RenderContext,
        models: impl IntoIterator<Item = &'m Box<dyn DrawModel>>,
    ) {
        {
            let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            });

            for model in models {
                model.draw(ctx.renderer, &mut rpass);
            }
        }

        ctx.encoder.copy_texture_to_texture(
            self.depth_texture.as_image_copy(),
            ctx.renderer.depth_stencil_texture.as_image_copy(),
            wgpu::Extent3d {
                width: ctx.renderer.surface_config.width,
                height: ctx.renderer.surface_config.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

pub trait DrawModel {
    fn draw<'s: 'p, 'r: 'p, 'p>(&'s self, renderer: &'r Renderer, rpass: &mut wgpu::RenderPass<'p>);
}
