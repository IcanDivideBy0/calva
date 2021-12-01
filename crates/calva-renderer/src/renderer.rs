use anyhow::{anyhow, Result};
use winit::window::Window;

use crate::AmbientPass;
use crate::Camera;
use crate::GeometryBuffer;
use crate::PointLightsPass;
use crate::RendererConfig;
use crate::SsaoPass;

pub struct RenderContext<'a> {
    pub renderer: &'a Renderer,

    pub frame: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,

    pub config: RendererConfig,
    pub camera: Camera,

    pub depth_stencil_texture: wgpu::Texture,
    pub depth_stencil: wgpu::TextureView,

    pub gbuffer: GeometryBuffer,
    pub ssao: SsaoPass,
    pub ambient: AmbientPass,
    pub lights: PointLightsPass,
}

impl Renderer {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

    pub const DEPTH_STENCIL: wgpu::DepthStencilState = wgpu::DepthStencilState {
        format: Self::DEPTH_FORMAT,
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::Less,
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState::IGNORE,
            back: wgpu::StencilFaceState::IGNORE,
            read_mask: 0,
            write_mask: 0,
        },
        bias: wgpu::DepthBiasState {
            constant: 0,
            slope_scale: 0.0,
            clamp: 0.0,
        },
    };

    pub async fn new(window: &Window) -> Result<Self> {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or_else(|| anyhow!("Cannot request WebGPU adapter"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await?;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface
                .get_preferred_format(&adapter)
                .ok_or_else(|| anyhow!("Unable to get surface preferred format"))?,
            width: size.width as u32,
            height: size.height as u32,
            // present_mode: wgpu::PresentMode::Immediate,
            // present_mode: wgpu::PresentMode::Mailbox,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &surface_config);

        let config = RendererConfig::new(&device);
        let camera = Camera::new(&device);

        let depth_stencil_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer depth stencil texture"),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        let depth_stencil =
            depth_stencil_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let gbuffer = GeometryBuffer::new(&device, &surface_config);
        let ssao = SsaoPass::new(&device, &surface_config, &config, &camera, &gbuffer);
        let ambient = AmbientPass::new(&device, &surface_config, &config, &gbuffer, &ssao);
        let lights =
            PointLightsPass::new(&device, &surface_config, &config, &camera, &gbuffer, &ssao);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,

            config,
            camera,

            depth_stencil_texture,
            depth_stencil,

            gbuffer,
            ssao,
            ambient,
            lights,
        })
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);

        self.depth_stencil_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer depth stencil texture"),
            size: wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        self.depth_stencil = self
            .depth_stencil_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.gbuffer = GeometryBuffer::new(&self.device, &self.surface_config);
        self.ssao = SsaoPass::new(
            &self.device,
            &self.surface_config,
            &self.config,
            &self.camera,
            &self.gbuffer,
        );
        self.ambient = AmbientPass::new(
            &self.device,
            &self.surface_config,
            &self.config,
            &self.gbuffer,
            &self.ssao,
        );
        self.lights = PointLightsPass::new(
            &self.device,
            &self.surface_config,
            &self.config,
            &self.camera,
            &self.gbuffer,
            &self.ssao,
        );
    }

    pub fn update_camera(&mut self, view: glam::Mat4, proj: glam::Mat4) {
        self.camera.view = view;
        self.camera.proj = proj;

        self.camera.update_buffers(&self.queue);
    }

    pub fn begin_render_frame(&self) -> Result<RenderContext, wgpu::SurfaceError> {
        self.config.update_buffer(&self.queue);

        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render command encoder"),
            });

        Ok(RenderContext {
            renderer: self,
            frame,
            view,
            encoder,
        })
    }

    pub fn finish_render_frame(&self, ctx: RenderContext) {
        self.queue.submit(std::iter::once(ctx.encoder.finish()));
        ctx.frame.present();
    }
}
