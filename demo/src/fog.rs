use core::{f32, f64};

use anyhow::Result;
use calva::renderer::{
    wgpu::{self, util::DeviceExt},
    Camera, GeometryPassOutputs, RenderContext, ResourcesManager, Time, UniformBuffer,
};
use noise::NoiseFn;

pub struct FogPass {
    resources: ResourcesManager,

    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl FogPass {
    const NOISE_SIZE: usize = 128;
    // const NOISE_SCALE: f64 = 1.0;
    const NOISE_SCALE: f64 = 1.0 / (Self::NOISE_SIZE as f64 / 4.0);
    // const NOISE_SCALE: f64 = 1.0 / 8.0;

    pub fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let device = resources.read::<wgpu::Device>();
        let queue = resources.read::<wgpu::Queue>();
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();
        let camera = resources.read::<UniformBuffer<Camera>>();
        let time = resources.read::<UniformBuffer<Time>>();
        let geometry_outputs = resources.read::<GeometryPassOutputs>();

        let seed = 0;
        // let noise = noise::Simplex::new(seed);
        // let noise = noise::Billow::<noise::Perlin>::new(seed);
        // let noise = noise::BasicMulti::<noise::Perlin>::new(seed);

        let noise_2 = noise::Billow::<noise::Perlin>::new(seed);
        let noise_2 = noise::ScaleBias::<_, _, 2>::new(noise_2)
            .set_scale(0.5)
            .set_bias(0.5);

        let noise_3 = noise::Billow::<noise::Perlin>::new(seed);
        let noise_3 = noise::ScaleBias::<_, _, 3>::new(noise_3)
            .set_scale(0.5)
            .set_bias(0.5);

        let noise_4 = noise::Billow::<noise::Perlin>::new(seed);
        let noise_4 = noise::ScaleBias::<_, _, 4>::new(noise_4)
            .set_scale(0.5)
            .set_bias(0.5);
        // let noise = noise::ScalePoint::new(noise).set_scale(Self::NOISE_SCALE);

        // let noise_a = ScalePoint::new(noise.clone()).set_scale(Self::NOISE_SCALE);
        // let noise_b = ScalePoint::new(noise.clone()).set_scale(-Self::NOISE_SCALE);
        // let noise_b = Constant::new(0.0);

        // let noise_b = TranslatePoint::new(noise.clone()).set_x_translation(0.5);
        // let noise_b = ScalePoint::new(noise).set_scale(Self::NOISE_SCALE);

        let noise_data = itertools::iproduct!(
            0..Self::NOISE_SIZE,
            0..Self::NOISE_SIZE,
            0..Self::NOISE_SIZE,
        )
        .map(|(z, y, x)| -> f32 {
            let x = x as f64;
            let y = y as f64;
            let z = z as f64;

            let xx = f64::cos(x / Self::NOISE_SIZE as f64 * f64::consts::TAU);
            let xy = f64::sin(x / Self::NOISE_SIZE as f64 * f64::consts::TAU);
            // let nx = noise_2.get([xx, xy]);

            let yx = f64::cos(y / Self::NOISE_SIZE as f64 * f64::consts::TAU);
            let yy = f64::sin(y / Self::NOISE_SIZE as f64 * f64::consts::TAU);
            // let ny = noise_3.get([yx, yy, nx]);

            let zx = f64::cos(z / Self::NOISE_SIZE as f64 * f64::consts::TAU);
            let zy = f64::sin(z / Self::NOISE_SIZE as f64 * f64::consts::TAU);
            // let nz = noise_4.get([zx, zy, nx, ny]);

            let q = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                x as f32 / Self::NOISE_SIZE as f32 * f32::consts::TAU,
                y as f32 / Self::NOISE_SIZE as f32 * f32::consts::TAU,
                0.0, // z as f32 / Self::NOISE_SIZE as f32 * f32::consts::TAU,
            );

            let p = glam::Vec3::splat(1.0).normalize();
            let p = q * p;

            noise_3.get([
                p.x as f64, //
                p.y as f64, //
                p.z as f64, //
            ]) as _

            // if z == 0.0 && y == 0.0 {
            //     dbg!(x);
            // }

            // let n = noise.get([x, y, z]) as f32;

            // nz as f32

            // let xx = (x + (Self::NOISE_SIZE as f64 / 2.0)) % Self::NOISE_SIZE as f64;
            // let yy = (y + (Self::NOISE_SIZE as f64 / 2.0)) % Self::NOISE_SIZE as f64;
            // let zz = (z + (Self::NOISE_SIZE as f64 / 2.0)) % Self::NOISE_SIZE as f64;

            // let nx = noise_a.get([xx, y, z]) as f32;
            // let ny = noise_a.get([x, yy, z]) as f32;

            // let nz = noise_a.get([x, y, zz]) as f32;

            // let nxy = noise_a.get([xx, yy, z]) as f32;

            // let get_t = |n: f64| {
            //     let t = n as f32 / Self::NOISE_SIZE as f32;
            //     f32::cos(2.0 * t * f32::consts::PI) / 2.0 + 0.5
            // };

            // let t = get_t(x);
            // let n_x = (1.0 - t) * n + t * nx;

            // let t = get_t(y);
            // let n_y = (1.0 - t) * n + t * ny;

            // // n_x
            // let n_xy = (1.0 - t) * n_x + t * n_y;

            // let t = f32::min(get_t(x), get_t(y));
            // let t = f32::cos(t * f32::consts::PI) / 2.0 + 0.5;
            // (1.0 - t) * n_y + t * n_x

            // let na = 0.0f32;
            // let nb = 1.0f32;
            // let t = f32::cos(f32::max(get_t(x), get_t(y)) * f32::consts::PI) / 2.0 + 0.5;
            // (1.0 - t) * na + t * nb
        })
        .collect::<Vec<_>>();

        let noise_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("Fog perlin noise"),
                size: wgpu::Extent3d {
                    width: Self::NOISE_SIZE as _,
                    height: Self::NOISE_SIZE as _,
                    depth_or_array_layers: Self::NOISE_SIZE as _,
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Fog sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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

        let shader = device.create_shader_module(wgpu::include_wgsl!("fog.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fog pipeline layout"),
            bind_group_layouts: &[
                Some(&camera.bind_group_layout),
                Some(&time.bind_group_layout),
                Some(&bind_group_layout),
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format: surface_config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: geometry_outputs.depth.format(),
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            resources,

            bind_group,
            pipeline,
        }
    }

    pub fn render(&self, ctx: &mut RenderContext) -> Result<()> {
        let camera = self.resources.read::<UniformBuffer<Camera>>();
        let time = self.resources.read::<UniformBuffer<Time>>();
        let geometry_outputs = self.resources.read::<GeometryPassOutputs>();

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
                    view: &geometry_outputs.depth_view,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                ..Default::default()
            },
        );

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &camera.bind_group, &[]);
        rpass.set_bind_group(1, &time.bind_group, &[]);
        rpass.set_bind_group(2, &self.bind_group, &[]);

        rpass.draw(0..3, 0..1);

        Ok(())
    }
}
