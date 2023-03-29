use anyhow::{anyhow, Result};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
#[cfg(feature = "profiler")]
use wgpu_profiler::{GpuProfiler, GpuTimerScopeResult};

pub struct RendererConfig {
    present_mode: wgpu::PresentMode,
    width: u32,
    height: u32,
}

pub struct Renderer {
    pub adapter: wgpu::Adapter,
    pub adapter_info: wgpu::AdapterInfo,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,

    #[cfg(feature = "profiler")]
    profiler: std::cell::RefCell<RendererProfiler>,
}

impl Renderer {
    const FEATURES: wgpu::Features = wgpu::Features::empty()
        .union(wgpu::Features::DEPTH_CLIP_CONTROL) // all platforms
        .union(wgpu::Features::TEXTURE_BINDING_ARRAY) // Vulkan, DX12, metal
        .union(wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY) // Vulkan, metal
        .union(wgpu::Features::MULTI_DRAW_INDIRECT) // Vulkan, DX12, metal
        .union(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT) // Vulkan, DX12
        .union(wgpu::Features::INDIRECT_FIRST_INSTANCE) // Vulkan, DX12, metal
        .union(wgpu::Features::PUSH_CONSTANTS) // All except WebGL (DX11 & OpenGL emulated w/ uniforms)
        .union(wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING) // Vulkan, DX12, metal
        .union(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) // All except WebGL
        .union(
            #[cfg(feature = "profiler")]
            GpuProfiler::ALL_WGPU_TIMER_FEATURES, // Vulkan, DX12
            #[cfg(not(feature = "profiler"))]
            wgpu::Features::empty(),
        );

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
                    features: Self::FEATURES,
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

        let mut surface_config = surface
            .get_default_config(&adapter, size.0, size.1)
            .ok_or_else(|| anyhow!("Surface not compatible with adapter"))?;
        surface_config.format = surface_config.format.add_srgb_suffix();
        // surface_config.present_mode = wgpu::PresentMode::AutoNoVsync;
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;

        surface.configure(&device, &surface_config);

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

            #[cfg(feature = "profiler")]
            profiler,
        })
    }

    pub fn update(&mut self, config: &RendererConfig) {
        let mut dirty = false;

        if config.present_mode != self.surface_config.present_mode {
            self.surface_config.present_mode = config.present_mode;

            dirty = true;
        }

        let new_size = (config.width, config.height);
        if new_size != (self.surface_config.width, self.surface_config.height) {
            self.surface_config.width = config.width;
            self.surface_config.height = config.height;

            dirty = true;
        }

        if dirty {
            self.surface.configure(&self.device, &self.surface_config);
        }
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
            frame: &frame_view,
        };

        cb(&mut context);

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
}

pub struct RenderContext<'a> {
    pub encoder: ProfilerCommandEncoder<'a>,
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
