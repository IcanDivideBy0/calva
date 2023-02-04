use anyhow::{anyhow, Result};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::cell::{Ref, RefCell};
use wgpu_profiler::{GpuProfiler, GpuTimerScopeResult};

use crate::CameraManager;

pub struct Renderer {
    pub adapter: wgpu::Adapter,
    pub adapter_info: wgpu::AdapterInfo,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,

    pub msaa: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    pub depth_stencil: wgpu::TextureView,

    pub camera: CameraManager,

    profiler: RefCell<RendererProfiler>,
}

impl Renderer {
    const FEATURES: &'static [wgpu::Features] = &[
        wgpu::Features::DEPTH_CLIP_CONTROL,        // all platforms
        wgpu::Features::MULTIVIEW,                 // Vulkan
        wgpu::Features::TIMESTAMP_QUERY,           // Vulkan, DX12, web
        wgpu::Features::TEXTURE_BINDING_ARRAY,     // Vulkan, DX12, metal
        wgpu::Features::MULTI_DRAW_INDIRECT,       // Vulkan, DX12, metal
        wgpu::Features::MULTI_DRAW_INDIRECT_COUNT, // Vulkan, DX12
        wgpu::Features::INDIRECT_FIRST_INSTANCE,   // Vulkan, DX12, metal
        wgpu::Features::PUSH_CONSTANTS,
        wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY,
        wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
        wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        GpuProfiler::ALL_WGPU_TIMER_FEATURES,
    ];

    pub const MULTISAMPLE_STATE: wgpu::MultisampleState = wgpu::MultisampleState {
        count: 4,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

    pub async fn new<W>(window: &W, size: (u32, u32)) -> Result<Self>
    where
        W: HasRawWindowHandle + HasRawDisplayHandle,
    {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = unsafe { instance.create_surface(window) }?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or_else(|| anyhow!("Cannot request WebGPU adapter"))?;

        let adapter_info = adapter.get_info();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Renderer device"),
                    features: Self::FEATURES
                        .iter()
                        .copied()
                        .fold(wgpu::Features::empty(), core::ops::BitOr::bitor),
                    limits: wgpu::Limits {
                        max_sampled_textures_per_shader_stage: 256,
                        max_push_constant_size: 128,
                        max_bind_groups: 6,
                        ..Default::default()
                    },
                },
                None,
            )
            .await?;

        let surface_capabilities = surface.get_capabilities(&adapter);
        let format = surface_capabilities.formats[0].remove_srgb_suffix();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            // present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![format],
        };
        surface.configure(&device, &surface_config);

        let (msaa, depth, depth_stencil) = Self::make_textures(&device, &surface_config);

        let camera = CameraManager::new(&device);

        let mut profiler = GpuProfiler::new(4, queue.get_timestamp_period(), device.features());
        profiler.enable_debug_marker = false;
        let profiler = RefCell::new(RendererProfiler {
            inner: profiler,
            results: vec![],
        });

        Ok(Self {
            adapter,
            adapter_info,
            device,
            queue,
            surface,
            surface_config,

            msaa,
            depth,
            depth_stencil,

            camera,

            profiler,
        })
    }

    pub fn resize(&mut self, size: (u32, u32)) {
        self.surface_config.width = size.0;
        self.surface_config.height = size.1;
        self.surface.configure(&self.device, &self.surface_config);

        (self.msaa, self.depth, self.depth_stencil) =
            Self::make_textures(&self.device, &self.surface_config);
    }

    pub fn render(&self, cb: impl FnOnce(&mut RenderContext)) -> Result<()> {
        let mut encoder = self.device.create_command_encoder(&Default::default());

        let frame = self.surface.get_current_texture()?;
        let frame_view = frame.texture.create_view(&Default::default());

        let mut renderer_profiler = self.profiler.try_borrow_mut()?;
        let profiler = &mut renderer_profiler.inner;

        profiler.begin_scope("RenderFrame", &mut encoder, &self.device);

        let mut context = RenderContext {
            surface_config: &self.surface_config,
            device: &self.device,
            queue: &self.queue,
            camera: &self.camera,
            output: RenderOutput {
                view: &self.msaa,
                resolve_target: Some(&frame_view),
                depth_stencil: &self.depth_stencil,
            },
            encoder: ProfilerCommandEncoder {
                device: &self.device,
                encoder: &mut encoder,
                profiler,
            },
        };

        cb(&mut context);

        profiler.end_scope(&mut encoder);
        profiler.resolve_queries(&mut encoder);

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        profiler.end_frame().unwrap();

        if let Some(results) = profiler.process_finished_frame() {
            renderer_profiler.results = results
        }

        Ok(())
    }

    pub fn profiler_results(&self) -> impl std::ops::Deref<Target = Vec<ProfilerResult>> + '_ {
        Ref::map(self.profiler.borrow(), |p| &p.results)
    }

    fn make_textures(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::TextureView, wgpu::TextureView, wgpu::TextureView) {
        let desc = wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: Self::MULTISAMPLE_STATE.count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm, // whatever
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[wgpu::TextureFormat::R8Unorm],
        };

        let msaa = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer msaa texture"),
            format: surface_config.format,
            view_formats: &[surface_config.format],
            ..desc
        });

        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer depth texture"),
            format: Self::DEPTH_FORMAT,
            usage: desc.usage | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::DEPTH_FORMAT],
            ..desc
        });

        (
            msaa.create_view(&Default::default()),
            depth.create_view(&wgpu::TextureViewDescriptor {
                aspect: wgpu::TextureAspect::DepthOnly,
                ..Default::default()
            }),
            depth.create_view(&Default::default()),
        )
    }
}

pub type ProfilerResult = GpuTimerScopeResult;
struct RendererProfiler {
    inner: GpuProfiler,
    pub results: Vec<ProfilerResult>,
}

pub struct RenderContext<'a> {
    pub surface_config: &'a wgpu::SurfaceConfiguration,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub camera: &'a CameraManager,
    pub output: RenderOutput<'a>,
    pub encoder: ProfilerCommandEncoder<'a>,
}

pub struct RenderOutput<'a> {
    pub view: &'a wgpu::TextureView,
    pub resolve_target: Option<&'a wgpu::TextureView>,
    pub depth_stencil: &'a wgpu::TextureView,
}

pub struct ProfilerCommandEncoder<'a> {
    device: &'a wgpu::Device,
    encoder: &'a mut wgpu::CommandEncoder,
    profiler: &'a mut GpuProfiler,
}

impl<'a> ProfilerCommandEncoder<'a> {
    pub fn profile_start(&mut self, label: &str) {
        self.encoder.push_debug_group(label);
        self.profiler.begin_scope(label, self.encoder, self.device);
    }

    pub fn profile_end(&mut self) {
        self.profiler.end_scope(self.encoder);
        self.encoder.pop_debug_group();
    }

    pub fn begin_compute_pass(
        &mut self,
        desc: &wgpu::ComputePassDescriptor,
    ) -> wgpu_profiler::scope::OwningScope<wgpu::ComputePass> {
        wgpu_profiler::scope::OwningScope::start(
            desc.label.unwrap_or("???"),
            self.profiler,
            self.encoder.begin_compute_pass(desc),
            self.device,
        )
    }

    pub fn begin_render_pass<'pass>(
        &'pass mut self,
        desc: &wgpu::RenderPassDescriptor<'pass, '_>,
    ) -> wgpu_profiler::scope::OwningScope<wgpu::RenderPass<'pass>> {
        wgpu_profiler::scope::OwningScope::start(
            desc.label.unwrap_or("???"),
            self.profiler,
            self.encoder.begin_render_pass(desc),
            self.device,
        )
    }
}

impl<'a> std::ops::Deref for ProfilerCommandEncoder<'a> {
    type Target = wgpu::CommandEncoder;

    fn deref(&self) -> &Self::Target {
        self.encoder
    }
}
impl<'a> std::ops::DerefMut for ProfilerCommandEncoder<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.encoder
    }
}
