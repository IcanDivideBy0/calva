use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    CameraManager, RenderContext, Renderer,
};
use noise::NoiseFn;

pub struct FogPassInput<'a> {
    pub depth: &'a wgpu::Texture,
}

pub struct FogPass {
    depth_view: wgpu::TextureView,

    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl FogPass {
    const NOISE_SIZE: u32 = 128;
    const NOISE_SCALE: f64 = 1.0 / Self::NOISE_SIZE as f64;

    pub fn new(renderer: &Renderer, camera: &CameraManager, input: FogPassInput) -> Self {
        let fbm = noise::Fbm::<noise::Perlin>::new(rand::random());
        let perlin = noise::Perlin::new(rand::random());

        let cx = (rand::random::<f64>(), rand::random::<f64>());
        let cy = (rand::random::<f64>(), rand::random::<f64>());
        let cz = (rand::random::<f64>(), rand::random::<f64>());

        let noise_data = itertools::iproduct!(
            0..Self::NOISE_SIZE,
            0..Self::NOISE_SIZE,
            0..Self::NOISE_SIZE
        )
        .map(|(x, y, z)| {
            let x = x as f64;
            let xx = x * Self::NOISE_SCALE * std::f64::consts::TAU;

            let y = y as f64;
            let yy = y * Self::NOISE_SCALE * std::f64::consts::TAU;

            let z = z as f64;
            let zz = z * Self::NOISE_SCALE * std::f64::consts::TAU;

            // let n = perlin.get([
            //     f64::from(x) * Self::NOISE_SCALE,
            //     f64::from(y) * Self::NOISE_SCALE,
            //     f64::from(z) * Self::NOISE_SCALE,
            // ]);

            let nx = perlin.get([
                cx.0 + 3.0 * xx.cos(),
                cx.1 + 3.0 * yy.sin(), //
                3.0 * yy.cos() * xx.sin(),
            ]);
            let ny = perlin.get([
                cy.0 + 3.0 * yy.cos(),
                cy.1 + 3.0 * zz.sin(), //
                3.0 * zz.cos() * yy.sin(),
            ]);
            let nz = perlin.get([
                cz.0 + 3.0 * zz.cos(),
                cz.1 + 3.0 * xx.sin(), //
                3.0 * xx.cos() * zz.sin(),
            ]);

            // let n = (nx + ny + nz) / 3.0;
            // let n1 = (nx + ny) / 2.0;
            // let n2 = (nx + nz) / 2.0;
            // let n3 = (nz + ny) / 2.0;

            let n = fbm.get([nx, ny, nz]);
            // let n = nx;

            // let n = perlin.get([nx, ny, nz]);
            // let n = perlin.get([
            //     cx.0 + 5.0 * (xx.cos() * xx.sin()),
            //     cy.0 + 5.0 * (yy.cos() * yy.sin()), //
            //     cz.0 + 5.0 * (zz.cos() * zz.sin()),
            //     0.34,
            // ]);

            // let n = nx;
            // nx = nx * std::f64::consts::PI;
            // ny = ny * std::f64::consts::PI;

            // let n = perlin.get([
            //     cx.0 + nx.cos(),
            //     cy.0 + ny.sin(),
            //     cz.0 + nx.sin() * ny.cos(),
            // ]);

            // let n = nx;
            // let mut nz = perlin.get([
            //     cz.0 + zz.cos(),
            //     cz.1 + zz.sin(), //
            //     (cz.0 + cz.1) + xx.cos() + yy.sin(),
            // ]);

            // let n = ny;

            // let mut nx = perlin.get([
            //     cx.0 + yy.cos(),
            //     cx.1 + zz.sin(), //
            //     (cx.0 + cx.1) + yy.sin() * zz.cos(),
            // ]);

            // let mut ny = perlin.get([
            //     cy.0 + zz.cos(),
            //     cy.1 + xx.sin(), //
            //     (cy.0 + cy.1) + zz.sin() * xx.cos(),
            // ]);

            // let mut nz = perlin.get([
            //     cz.0 + xx.cos(),
            //     cz.1 + yy.sin(), //
            //     (cz.0 + cz.1) + xx.sin() * yy.cos(),
            // ]);

            // let n = ny;

            // nx = (nx * 0.5 + 0.5) * std::f64::consts::TAU;
            // ny = (ny * 0.5 + 0.5) * std::f64::consts::TAU;
            // nz = (nz * 0.5 + 0.5) * std::f64::consts::TAU;

            // let n = perlin.get([nx.cos(), nz.cos(), ny.cos()]);
            // let n = perlin.get([nx, ny]);

            // let mut nx = perlin.get([xx.cos(), yy.sin(), xx.sin() * yy.cos()]);
            // let mut ny = perlin.get([yy.cos(), yy.sin(), nx]);
            // let mut nz = perlin.get([zz.cos(), zz.sin(), ny]);
            // nx = (nx * 0.5 + 0.5) * std::f64::consts::TAU;
            // ny = (ny * 0.5 + 0.5) * std::f64::consts::TAU;
            // nz = (nz * 0.5 + 0.5) * std::f64::consts::TAU;

            // let n = perlin.get([nx, ny, nz]);
            // let n = nx;

            // let l = (nx.powf(2.0) + ny.powf(2.0) + nz.powf(2.0)).sqrt();
            // let n = perlin.get([nx / l, ny / l, nz / l]);

            // let mut nx = perlin.get([cx.0 + xx.sin(), cx.1 + xx.cos()]);
            // let mut ny = perlin.get([cy.0 + yy.sin(), cy.1 + yy.cos()]);
            // let mut nz = perlin.get([cz.0 + zz.sin(), cz.1 + zz.cos()]);

            // nx = (nx * 0.5 + 0.5) * std::f64::consts::TAU;
            // ny = (ny * 0.5 + 0.5) * std::f64::consts::TAU;
            // nz = (nz * 0.5 + 0.5) * std::f64::consts::TAU;

            // let l = (nx.powf(2.0) + ny.powf(2.0) + nz.powf(2.0)).sqrt();

            // let n = perlin.get([nx / l, ny / l, nz / l]);

            // let ny = perlin.get([yy.sin() + 3.978, yy.cos()]);
            // let nz = perlin.get([zz.sin() - 3.978, zz.cos()]);

            // let n = (nx + ny) / 2.0;
            // let n = nx;

            // let p = [
            //     xx.cos(),
            //     xx.sin() * yy.cos(),
            //     xx.sin() * yy.sin() * zz.cos(),
            //     xx.sin() * yy.sin() * zz.sin(),
            // ];

            // let n1 = perlin.get([
            //     p[0].sin(),
            //     p[1].cos(), //
            //     p[0].cos() * p[1].sin(),
            // ]);

            // let n2 = perlin.get([
            //     p[2].sin(),
            //     p[3].cos(), //
            //     p[2].cos() * p[3].sin(),
            // ]);

            // let n = (n1 + n2) / 2.0;

            // let n = perlin.get([
            //     xx.sin(),
            //     zz.cos(), //
            //     xx.cos() * zz.sin(),
            //     // 100.0, // f64::from(z) * Self::NOISE_SCALE,
            //     // 100.0, // f64::from(z) * Self::NOISE_SCALE,
            //     // 100.0, // f64::from(z) * Self::NOISE_SCALE,
            // ]);

            // let n = perlin.get([
            //     f64::from(x) * Self::NOISE_SCALE + 0.5,
            //     f64::from(y) * Self::NOISE_SCALE + 0.5,
            //     f64::from(z) * Self::NOISE_SCALE + 0.5,
            // ]);

            // let n = perlin.get([xx.cos(), yy.cos(), zz.cos()]);
            // let n = perlin.get([
            //     xx.sin(),
            //     yy.cos(), //
            //               // xx.cos() * yy.sin(),
            //               // zz.cos(),
            // ]);

            // let nxy = perlin.get([
            //     xx.sin(),
            //     yy.cos(), //
            //     xx.cos() * yy.sin(),
            // ]);
            // let nyz = perlin.get([
            //     yy.sin(),
            //     zz.cos(), //
            //     yy.cos() * zz.sin(),
            // ]);
            // let nzx = perlin.get([
            //     zz.sin(),
            //     xx.cos(), //
            //     zz.cos() * xx.sin(),
            // ]);

            // let nxy = (nxy * 0.5 + 0.5) * std::f64::consts::TAU;
            // let nyz = (nyz * 0.5 + 0.5) * std::f64::consts::TAU;
            // let nzx = (nzx * 0.5 + 0.5) * std::f64::consts::TAU;

            // let n = perlin.get([xx.cos(), yy.cos(), xx.sin(), yy.sin()]);

            // let n = (nx + ny + nz) / 3.0;

            // let n = perlin.get([
            //     xx.cos(),            //
            //     yy.sin(),            //
            //     xx.sin() * yy.cos(), //
            // ]);

            // let nn = (n * 0.5 + 0.5) * std::f64::consts::TAU;

            // let nz = perlin.get([
            //     zz.sin(), //
            //     zz.cos(), //
            // ]);

            // let n = (2.0 * n + nz) / 3.0;

            // let p1 = perlin.get([xx.cos(), yy.sin(), xx.sin() * yy.cos()]);
            // let p2 = perlin2.get([zz.cos(), xx.sin(), zz.sin() * xx.cos()]);
            // let p3 = perlin.get([yy.cos(), zz.sin(), yy.sin() * zz.cos()]);
            // let p2 = perlin.get([xx.cos(), zz.sin(), xx.sin() * zz.cos()]);

            // let n = (p1 + p2 + p3) / 3.0;
            // let n = (p1 + p2) / 2.0;

            // let mut xy = perlin.get([
            //     xx.cos() / std::f64::consts::TAU, //
            //     yy.cos() / std::f64::consts::TAU, //
            //     xx.sin() / std::f64::consts::TAU, //
            //     yy.sin() / std::f64::consts::TAU, //
            // ]);
            // let xy = perlin.get([
            //     xx.cos(), //
            //     yy.sin(), //

            // ]);

            // let rad_xy = (xy * 0.5 + 0.5) * std::f64::consts::TAU;

            // let xyzz = perlin.get([
            //     rad_xy.cos(),            //
            //     zz.sin(),                //
            //     rad_xy.sin() * zz.cos(), //
            // ]);

            // let n = xy;

            // let p = [xx.cos(), yy.cos(), xx.sin(), yy.sin()];

            // let q = glam::Quat::from_array([
            //     (xx.cos()) as f32,
            //     (xx.sin() * yy.cos()) as f32,
            //     (xx.sin() * yy.sin() * zz.cos()) as f32,
            //     (xx.sin() * yy.sin() * zz.sin()) as f32,
            // ]);
            // let p = [
            //     xx.cos(),
            //     xx.sin() * yy.cos(),
            //     xx.sin() * yy.sin() * zz.cos(),
            //     xx.sin() * yy.sin() * zz.sin(),
            // ];

            // let p = (q * glam::Vec3::Y).normalize();

            // let n = perlin.get([p.x as f64, p.y as f64, p.z as f64]);
            // let n = perlin.get([q.x as f64, q.y as f64, q.z as f64, q.w as f64]);

            n as f32 * 0.5 + 0.5
        })
        .collect::<Vec<_>>();

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
            wgpu::util::TextureDataOrder::LayerMajor,
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
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: renderer.surface_config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: input.depth.format(),
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            });

        Self {
            depth_view: input.depth.create_view(&Default::default()),

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
        let mut rpass = ctx.encoder.scoped_render_pass(
            "Fog",
            wgpu::RenderPassDescriptor {
                label: Some("Fog"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.frame,
                    depth_slice: None,
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
        rpass.set_bind_group(1, &self.bind_group, &[]);

        rpass.set_push_constants(
            wgpu::ShaderStages::FRAGMENT,
            0,
            bytemuck::bytes_of(&time.elapsed().as_secs_f32()),
        );

        rpass.draw(0..3, 0..1);
    }
}
