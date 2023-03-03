use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    CameraManager, RenderContext, Renderer,
};
use noise::NoiseFn;

pub struct FogPass {
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl FogPass {
    const NOISE_SIZE: u32 = 128;
    const NOISE_SCALE: f64 = 1.0 / Self::NOISE_SIZE as f64;

    pub fn new(renderer: &Renderer, camera: &CameraManager) -> Self {
        // let perlin = noise::Fbm::<noise::Perlin>::new(rand::random());
        let perlin = noise::Perlin::new(rand::random());

        let noise_data = simdnoise::NoiseBuilder::fbm_3d_offset(
            (Self::NOISE_SIZE / 2) as _,
            Self::NOISE_SIZE as _,
            (Self::NOISE_SIZE / 2) as _,
            Self::NOISE_SIZE as _,
            (Self::NOISE_SIZE / 2) as _,
            Self::NOISE_SIZE as _,
        )
        // .with_freq(0.05)
        // .with_octaves(5)
        // .with_gain(2.0)
        // .with_seed(1337)
        // .with_lacunarity(0.5)
        .generate_scaled(0.0, 1.0);

        // let noise_data = (0..Self::NOISE_SIZE)
        //     .flat_map(|z| {
        //         let zz = f64::from(z) * Self::NOISE_SCALE * std::f64::consts::TAU;
        //         (0..Self::NOISE_SIZE)
        //             .flat_map(|y| {
        //                 let yy = f64::from(y) * Self::NOISE_SCALE * std::f64::consts::TAU;
        //                 (0..Self::NOISE_SIZE)
        //                     .map(|x| {
        //                         let xx = f64::from(x) * Self::NOISE_SCALE * std::f64::consts::TAU;

        //                         // let p = [
        //                         //     xx.cos(),
        //                         //     xx.sin() * yy.cos(),
        //                         //     xx.sin() * yy.sin() * zz.cos(),
        //                         //     xx.sin() * yy.sin() * zz.sin(),
        //                         // ];

        //                         // let n1 = perlin.get([
        //                         //     p[0].sin(),
        //                         //     p[1].cos(), //
        //                         //     p[0].cos() * p[1].sin(),
        //                         // ]);

        //                         // let n2 = perlin.get([
        //                         //     p[2].sin(),
        //                         //     p[3].cos(), //
        //                         //     p[2].cos() * p[3].sin(),
        //                         // ]);

        //                         // let n = (n1 + n2) / 2.0;

        //                         // let n = perlin.get([
        //                         //     xx.sin(),
        //                         //     zz.cos(), //
        //                         //     xx.cos() * zz.sin(),
        //                         //     // 100.0, // f64::from(z) * Self::NOISE_SCALE,
        //                         //     // 100.0, // f64::from(z) * Self::NOISE_SCALE,
        //                         //     // 100.0, // f64::from(z) * Self::NOISE_SCALE,
        //                         // ]);

        //                         // let n = perlin.get([
        //                         //     f64::from(x) * Self::NOISE_SCALE,
        //                         //     f64::from(y) * Self::NOISE_SCALE,
        //                         //     f64::from(z) * Self::NOISE_SCALE,
        //                         // ]);

        //                         // let n = perlin.get([
        //                         //     f64::from(x) * Self::NOISE_SCALE + 0.5,
        //                         //     f64::from(y) * Self::NOISE_SCALE + 0.5,
        //                         //     f64::from(z) * Self::NOISE_SCALE + 0.5,
        //                         // ]);

        //                         // let n = perlin.get([xx.cos(), yy.cos(), zz.cos()]);
        //                         // let n = perlin.get([
        //                         //     xx.sin(),
        //                         //     yy.cos(), //
        //                         //               // xx.cos() * yy.sin(),
        //                         //               // zz.cos(),
        //                         // ]);

        //                         // let nxy = perlin.get([
        //                         //     xx.sin(),
        //                         //     yy.cos(), //
        //                         //     xx.cos() * yy.sin(),
        //                         // ]);
        //                         // let nyz = perlin.get([
        //                         //     yy.sin(),
        //                         //     zz.cos(), //
        //                         //     yy.cos() * zz.sin(),
        //                         // ]);
        //                         // let nzx = perlin.get([
        //                         //     zz.sin(),
        //                         //     xx.cos(), //
        //                         //     zz.cos() * xx.sin(),
        //                         // ]);

        //                         // let nxy = (nxy * 0.5 + 0.5) * std::f64::consts::TAU;
        //                         // let nyz = (nyz * 0.5 + 0.5) * std::f64::consts::TAU;
        //                         // let nzx = (nzx * 0.5 + 0.5) * std::f64::consts::TAU;

        //                         let n = perlin.get([xx.cos(), yy.cos(), xx.sin(), yy.sin()]);

        //                         // let n = (nx + ny + nz) / 3.0;

        //                         // let n = perlin.get([
        //                         //     xx.cos(),            //
        //                         //     yy.sin(),            //
        //                         //     xx.sin() * yy.cos(), //
        //                         // ]);

        //                         // let nn = (n * 0.5 + 0.5) * std::f64::consts::TAU;

        //                         // let nz = perlin.get([
        //                         //     zz.sin(), //
        //                         //     zz.cos(), //
        //                         // ]);

        //                         // let n = (2.0 * n + nz) / 3.0;

        //                         // let p1 = perlin.get([xx.cos(), yy.sin(), xx.sin() * yy.cos()]);
        //                         // let p2 = perlin2.get([zz.cos(), xx.sin(), zz.sin() * xx.cos()]);
        //                         // let p3 = perlin.get([yy.cos(), zz.sin(), yy.sin() * zz.cos()]);
        //                         // let p2 = perlin.get([xx.cos(), zz.sin(), xx.sin() * zz.cos()]);

        //                         // let n = (p1 + p2 + p3) / 3.0;
        //                         // let n = (p1 + p2) / 2.0;

        //                         // let mut xy = perlin.get([
        //                         //     xx.cos() / std::f64::consts::TAU, //
        //                         //     yy.cos() / std::f64::consts::TAU, //
        //                         //     xx.sin() / std::f64::consts::TAU, //
        //                         //     yy.sin() / std::f64::consts::TAU, //
        //                         // ]);
        //                         // let xy = perlin.get([
        //                         //     xx.cos(), //
        //                         //     yy.sin(), //

        //                         // ]);

        //                         // let rad_xy = (xy * 0.5 + 0.5) * std::f64::consts::TAU;

        //                         // let xyzz = perlin.get([
        //                         //     rad_xy.cos(),            //
        //                         //     zz.sin(),                //
        //                         //     rad_xy.sin() * zz.cos(), //
        //                         // ]);

        //                         // let n = xy;

        //                         // let p = [xx.cos(), yy.cos(), xx.sin(), yy.sin()];

        //                         // let q = glam::Quat::from_array([
        //                         //     (xx.cos()) as f32,
        //                         //     (xx.sin() * yy.cos()) as f32,
        //                         //     (xx.sin() * yy.sin() * zz.cos()) as f32,
        //                         //     (xx.sin() * yy.sin() * zz.sin()) as f32,
        //                         // ]);
        //                         // let p = [
        //                         //     xx.cos(),
        //                         //     xx.sin() * yy.cos(),
        //                         //     xx.sin() * yy.sin() * zz.cos(),
        //                         //     xx.sin() * yy.sin() * zz.sin(),
        //                         // ];

        //                         // let p = (q * glam::Vec3::Y).normalize();

        //                         // let n = perlin.get([p.x as f64, p.y as f64, p.z as f64]);
        //                         // let n = perlin.get([q.x as f64, q.y as f64, q.z as f64, q.w as f64]);

        //                         n as f32 * 0.5 + 0.5
        //                     })
        //                     .collect::<Vec<_>>()
        //             })
        //             .collect::<Vec<_>>()
        //     })
        //     .collect::<Vec<_>>();

        let noise_texture = renderer.device.create_texture_with_data(
            &renderer.queue,
            &wgpu::TextureDescriptor {
                label: Some("Fog perlin noise"),
                size: wgpu::Extent3d {
                    width: Self::NOISE_SIZE,
                    height: Self::NOISE_SIZE,
                    depth_or_array_layers: Self::NOISE_SIZE,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[wgpu::TextureFormat::R32Float],
            },
            bytemuck::cast_slice(&noise_data),
        );

        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Fog sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout =
            renderer
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Fog bind group layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D3,
                                multisampled: false,
                            },
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            count: None,
                        },
                    ],
                });

        let bind_group = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Fog bind group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &noise_texture.create_view(&Default::default()),
                        ),
                    },
                ],
            });

        let shader = renderer
            .device
            .create_shader_module(wgpu::include_wgsl!("fog.wgsl"));

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Fog pipeline layout"),
                    bind_group_layouts: &[&camera.bind_group_layout, &bind_group_layout],
                    push_constant_ranges: &[wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::FRAGMENT,
                        range: 0..(std::mem::size_of::<f32>() as _),
                    }],
                });

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Fog pipeline"),
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
                        format: renderer.surface_config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Renderer::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
            });

        Self {
            bind_group,
            pipeline,
        }
    }

    pub fn render(
        &self,
        ctx: &mut RenderContext,
        camera: &CameraManager,
        time: &std::time::Instant,
    ) {
        let mut rpass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Fog"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.frame,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: ctx.depth_stencil,
                depth_ops: None,
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &camera.bind_group, &[]);
        rpass.set_bind_group(1, &self.bind_group, &[]);

        rpass.set_push_constants(
            wgpu::ShaderStages::FRAGMENT,
            0,
            bytemuck::bytes_of(&time.elapsed().as_secs_f32()),
        );

        rpass.draw(0..3, 0..1);
    }
}
