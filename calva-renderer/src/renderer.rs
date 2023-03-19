use anyhow::{anyhow, Result};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
#[cfg(feature = "profiler")]
use wgpu_profiler::{GpuProfiler, GpuTimerScopeResult};

pub struct Renderer {
    pub adapter: wgpu::Adapter,
    pub adapter_info: wgpu::AdapterInfo,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,

    pub depth: wgpu::TextureView,
    pub depth_stencil: wgpu::TextureView,

    #[cfg(feature = "profiler")]
    profiler: std::cell::RefCell<RendererProfiler>,
}

impl Renderer {
    const FEATURES: &'static [wgpu::Features] = &[
        wgpu::Features::DEPTH_CLIP_CONTROL,             // all platforms
        wgpu::Features::TEXTURE_BINDING_ARRAY,          // Vulkan, DX12, metal
        wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY, // Vulkan, metal
        wgpu::Features::MULTI_DRAW_INDIRECT,            // Vulkan, DX12, metal
        wgpu::Features::MULTI_DRAW_INDIRECT_COUNT,      // Vulkan, DX12
        wgpu::Features::INDIRECT_FIRST_INSTANCE,        // Vulkan, DX12, metal
        wgpu::Features::PUSH_CONSTANTS, // All except WebGL (DX11 & OpenGL emulated w/ uniforms)
        wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING, // Vulkan, DX12, metal
        wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES, // All except WebGL
        #[cfg(feature = "profiler")]
        GpuProfiler::ALL_WGPU_TIMER_FEATURES, // Vulkan, DX12
    ];

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
                        max_sampled_textures_per_shader_stage: 512,
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

        let (depth, depth_stencil) = Self::make_textures(&device, &surface_config);

        #[cfg(feature = "profiler")]
        let profiler = {
            let mut profiler = GpuProfiler::new(4, queue.get_timestamp_period(), device.features());
            profiler.enable_debug_marker = false;
            std::cell::RefCell::new(RendererProfiler {
                inner: profiler,
                results: vec![],
            })
        };

        Ok(Self {
            adapter,
            adapter_info,
            device,
            queue,
            surface,
            surface_config,

            depth,
            depth_stencil,

            #[cfg(feature = "profiler")]
            profiler,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    pub fn resize(&mut self, size: (u32, u32)) {
        if size == self.size() {
            return;
        }

        self.surface_config.width = size.0;
        self.surface_config.height = size.1;
        self.surface.configure(&self.device, &self.surface_config);

        (self.depth, self.depth_stencil) = Self::make_textures(&self.device, &self.surface_config);
    }

    pub fn render(&self, cb: impl FnOnce(&mut RenderContext)) -> Result<()> {
        let mut encoder = self.device.create_command_encoder(&Default::default());

        let frame = self.surface.get_current_texture()?;
        let frame_view = frame.texture.create_view(&Default::default());

        #[cfg(feature = "profiler")]
        let mut renderer_profiler = self.profiler.try_borrow_mut()?;
        #[cfg(feature = "profiler")]
        let profiler = &mut renderer_profiler.inner;

        #[cfg(feature = "profiler")]
        profiler.begin_scope("RenderFrame", &mut encoder, &self.device);

        let mut context = RenderContext {
            encoder: ProfilerCommandEncoder {
                encoder: &mut encoder,

                #[cfg(feature = "profiler")]
                device: &self.device,
                #[cfg(feature = "profiler")]
                profiler,
            },

            depth_stencil: &self.depth_stencil,
            frame: &frame_view,
        };

        cb(&mut context);

        drop(context);

        #[cfg(feature = "profiler")]
        {
            profiler.end_scope(&mut encoder);
            profiler.resolve_queries(&mut encoder);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        #[cfg(feature = "profiler")]
        {
            profiler.end_frame().unwrap();

            if let Some(results) = profiler.process_finished_frame() {
                renderer_profiler.results = results
            }
        }

        Ok(())
    }

    #[cfg(feature = "profiler")]
    pub fn profiler_results(&self) -> impl std::ops::Deref<Target = Vec<ProfilerResult>> + '_ {
        std::cell::Ref::map(self.profiler.borrow(), |p| &p.results)
    }

    fn make_textures(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::TextureView, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        };

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Renderer depth texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            format: Self::DEPTH_FORMAT,
            view_formats: &[Self::DEPTH_FORMAT],
        });

        let depth = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });
        let depth_stencil = depth_texture.create_view(&Default::default());

        (depth, depth_stencil)
    }
}

pub struct RenderContext<'a> {
    pub encoder: ProfilerCommandEncoder<'a>,

    pub depth_stencil: &'a wgpu::TextureView,
    pub frame: &'a wgpu::TextureView,
}

#[cfg(feature = "profiler")]
pub type ProfilerResult = GpuTimerScopeResult;
#[cfg(feature = "profiler")]
struct RendererProfiler {
    inner: GpuProfiler,
    pub results: Vec<ProfilerResult>,
}

pub struct ProfilerCommandEncoder<'a> {
    encoder: &'a mut wgpu::CommandEncoder,

    #[cfg(feature = "profiler")]
    device: &'a wgpu::Device,
    #[cfg(feature = "profiler")]
    profiler: &'a mut GpuProfiler,
}

impl<'a> ProfilerCommandEncoder<'a> {
    pub fn profile_start(&mut self, label: &str) {
        #[cfg(debug_assertions)]
        self.encoder.push_debug_group(label);
        #[cfg(feature = "profiler")]
        self.profiler.begin_scope(label, self.encoder, self.device);
    }

    pub fn profile_end(&mut self) {
        #[cfg(feature = "profiler")]
        self.profiler.end_scope(self.encoder);
        #[cfg(debug_assertions)]
        self.encoder.pop_debug_group();
    }

    #[cfg(feature = "profiler")]
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

    #[cfg(feature = "profiler")]
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
