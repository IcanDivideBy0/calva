use crate::{GeometryPass, MaterialsManager, RenderContext, Renderer, TexturesManager};

pub struct AmbientLightPass {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl AmbientLightPass {
    pub fn new(
        renderer: &Renderer,
        textures: &TexturesManager,
        materials: &MaterialsManager,
        geometry: &GeometryPass,
    ) -> Self {
        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("AmbientLight bind group layout"),
                    entries: &[
                        // albedo
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
                    ],
                });

        let bind_group = Self::make_bind_group(renderer, geometry, &bind_group_layout);

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("ambient_light.wgsl"));

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("AmbientLight pipeline layout"),
                    bind_group_layouts: &[
                        &textures.bind_group_layout,
                        &materials.bind_group_layout,
                        &bind_group_layout,
                    ],
                    push_constant_ranges: &[wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::FRAGMENT,
                        range: 0..(std::mem::size_of::<[f32; 2]>() as _),
                    }],
                });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("AmbientLight pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Renderer::OUTPUT_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
            });

        Self {
            bind_group_layout,
            bind_group,
            pipeline,
        }
    }

    pub fn rebind(&mut self, renderer: &Renderer, geometry: &GeometryPass) {
        self.bind_group = Self::make_bind_group(renderer, geometry, &self.bind_group_layout);
    }

    pub fn render(
        &self,
        ctx: &mut RenderContext,
        textures: &TexturesManager,
        materials: &MaterialsManager,
        gamma: f32,
        factor: f32,
    ) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("AmbientLight"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &textures.bind_group, &[]);
        rpass.set_bind_group(1, &materials.bind_group, &[]);
        rpass.set_bind_group(2, &self.bind_group, &[]);
        rpass.set_push_constants(
            wgpu::ShaderStages::FRAGMENT,
            0,
            bytemuck::bytes_of(&[gamma, factor]),
        );

        rpass.draw(0..3, 0..1);
    }

    fn make_bind_group(
        renderer: &Renderer,
        geometry: &GeometryPass,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("AmbientLight bind group"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&geometry.albedo_metallic),
                }],
            })
    }
}
