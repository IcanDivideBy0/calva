use anyhow::{anyhow, Result};
use winit::window::Window;

use crate::{CameraUniform, RendererConfig};

// pub struct TraceEncoder<'a> {
//     profiler: &'a mut GpuProfiler,
//     device: &'a wgpu::Device,

//     inner: &'a mut wgpu::CommandEncoder,
// }

// impl<'a> TraceEncoder<'a> {
//     pub fn begin_render_pass<'s: 'd, 'd>(
//         &'s mut self,
//         desc: &wgpu::RenderPassDescriptor<'d, '_>,
//     ) -> impl std::ops::DerefMut<Target = wgpu::RenderPass<'d>> {
//         wgpu_profiler::scope::OwningScope::start(
//             desc.label.unwrap_or("unnamed"),
//             self.profiler,
//             self.inner.begin_render_pass(desc),
//             self.device,
//         )
//     }
// }

// impl<'a> std::ops::Deref for TraceEncoder<'a> {
//     type Target = wgpu::CommandEncoder;

//     fn deref(&self) -> &Self::Target {
//         self.inner
//     }
// }

// impl<'a> std::ops::DerefMut for TraceEncoder<'a> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         self.inner
//     }
// }

pub struct RenderContext<'a> {
    pub renderer: &'a Renderer,

    pub view: &'a wgpu::TextureView,
    pub resolve_target: Option<&'a wgpu::TextureView>,
    pub encoder: wgpu::CommandEncoder,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,

    pub msaa_texture: wgpu::Texture,
    pub msaa: wgpu::TextureView,
    pub depth_stencil_texture: wgpu::Texture,
    pub depth_stencil: wgpu::TextureView,

    pub config: RendererConfig,
    pub camera: CameraUniform,
}

impl Renderer {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

    pub const MULTISAMPLE_STATE: wgpu::MultisampleState = wgpu::MultisampleState {
        count: 4,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    pub async fn new(window: &Window) -> Result<Self> {
        let size = window.inner_size();

        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        // let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let instance = wgpu::Instance::new(wgpu::Backends::VULKAN);
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
                    // features: wgpu::Features::empty(),
                    // features: wgpu::Features::CLEAR_COMMANDS,
                    features: wgpu::Features::DEPTH_CLIP_CONTROL // all platforms
                        | wgpu::Features::MULTIVIEW // Vulkan
                        | wgpu::Features::TIMESTAMP_QUERY, // Vulkan, DX12, web
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

        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };

        let msaa_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer msaa texture"),
            size,
            mip_level_count: 1,
            sample_count: Self::MULTISAMPLE_STATE.count,
            dimension: wgpu::TextureDimension::D2,
            format: surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        });

        let msaa = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_stencil_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer depth stencil texture"),
            size,
            mip_level_count: 1,
            sample_count: Self::MULTISAMPLE_STATE.count,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
        });

        let depth_stencil =
            depth_stencil_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let config = RendererConfig::new(&device);
        let camera = CameraUniform::new(&device);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,

            msaa_texture,
            msaa,
            depth_stencil_texture,
            depth_stencil,

            config,
            camera,
        })
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);

        self.msaa_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer msaa texture"),
            format: self.surface_config.format,
            size: wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: Self::MULTISAMPLE_STATE.count,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        });

        self.msaa = self
            .msaa_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.depth_stencil_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer depth stencil texture"),
            size: wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: Self::MULTISAMPLE_STATE.count,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
        });
        self.depth_stencil = self
            .depth_stencil_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
    }

    pub fn update_camera(&mut self, view: glam::Mat4, proj: glam::Mat4) {
        self.camera.view = view;
        self.camera.proj = proj;

        self.camera.update_buffer(&self.queue);
    }

    pub fn render(&self, cb: impl FnOnce(&mut RenderContext)) -> Result<(), wgpu::SurfaceError> {
        self.config.update_buffer(&self.queue);

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Renderer command encoder"),
            });

        let frame = self.surface.get_current_texture()?;
        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut ctx = RenderContext {
            renderer: self,

            encoder,
            view: &self.msaa,
            resolve_target: Some(&frame_view),
        };

        cb(&mut ctx);

        self.queue.submit(std::iter::once(ctx.encoder.finish()));
        frame.present();

        Ok(())
    }
}
