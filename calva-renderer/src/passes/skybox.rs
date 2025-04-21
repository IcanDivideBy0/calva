use crate::{CameraManager, RenderContext, RessourceRef, RessourcesManager, SkyboxManager};

pub struct SkyboxPassInputs<'a> {
    pub depth: &'a wgpu::Texture,
    pub output: &'a wgpu::Texture,
}

pub struct SkyboxPass {
    camera: RessourceRef<CameraManager>,
    skybox: RessourceRef<SkyboxManager>,

    depth_view: wgpu::TextureView,
    output_view: wgpu::TextureView,

    pipeline: wgpu::RenderPipeline,
}

impl SkyboxPass {
    pub fn new(
        device: &wgpu::Device,
        ressources: &RessourcesManager,
        inputs: SkyboxPassInputs,
    ) -> Self {
        let camera = ressources.get::<CameraManager>();
        let skybox = ressources.get::<SkyboxManager>();

        let output_view = inputs.output.create_view(&Default::default());
        let depth_view = inputs.depth.create_view(&Default::default());

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox render pipeline layout"),
            bind_group_layouts: &[
                &camera.get().bind_group_layout,
                &skybox.get().bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("skybox.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox render pipeline"),
            layout: Some(&pipeline_layout),
            multiview: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: inputs.output.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            cache: None,
        });

        Self {
            camera,
            skybox,

            output_view,
            depth_view,

            pipeline,
        }
    }

    pub fn rebind(&mut self, inputs: SkyboxPassInputs) {
        self.output_view = inputs.output.create_view(&Default::default());
        self.depth_view = inputs.depth.create_view(&Default::default());
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        if let Some(skybox_bind_group) = self.skybox.get().bind_group.as_ref() {
            let camera = self.camera.get();

            let mut rpass = ctx.encoder.scoped_render_pass(
                "Skybox",
                wgpu::RenderPassDescriptor {
                    label: Some("Skybox"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: None,
                        stencil_ops: None,
                    }),
                    ..Default::default()
                },
            );

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &camera.bind_group, &[]);
            rpass.set_bind_group(1, skybox_bind_group, &[]);

            rpass.draw(0..3, 0..1);
        }
    }
}
