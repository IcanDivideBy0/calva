use std::sync::Arc;

use anyhow::{anyhow, Result};

#[cfg(feature = "profiler")]
use wgpu_profiler::{
    GpuProfiler, GpuProfilerSettings, GpuTimerQueryResult, ManualOwningScope, OwningScope,
};

pub struct Renderer<'window> {
    pub surface: wgpu::Surface<'window>,
    pub surface_config: wgpu::SurfaceConfiguration,

    pub adapter: wgpu::Adapter,
    pub adapter_info: wgpu::AdapterInfo,

    pub device: Arc<wgpu::Device>,
    pub queue: wgpu::Queue,

    #[cfg(feature = "profiler")]
    pub profiler: std::cell::RefCell<RendererProfiler>,
}

impl<'window> Renderer<'window> {
    const FEATURES: wgpu::Features = wgpu::Features::empty()
        .union(wgpu::Features::DEPTH_CLIP_CONTROL) // all platforms
        .union(wgpu::Features::MULTI_DRAW_INDIRECT) // Vulkan, DX12, Metal
        .union(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT) // Vulkan, DX12
        .union(wgpu::Features::INDIRECT_FIRST_INSTANCE) // Vulkan, DX12, Metal
        .union(wgpu::Features::TEXTURE_BINDING_ARRAY) // Vulkan, DX12, Metal
        .union(wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY) // Vulkan, Metal
        .union(wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING) // Vulkan, DX12, Metal
        .union(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) // All except WebGL
        .union(wgpu::Features::POLYGON_MODE_LINE) // Vulkan, DX12, Metal
        .union(wgpu::Features::FLOAT32_FILTERABLE) // Vulkan, DX12, Metal
        .union(
            #[cfg(feature = "profiler")]
            GpuProfiler::ALL_WGPU_TIMER_FEATURES, // Vulkan, DX12
            #[cfg(not(feature = "profiler"))]
            wgpu::Features::empty(),
        );

    pub async fn new(
        window: impl Into<wgpu::SurfaceTarget<'window>>,
        size: (u32, u32),
    ) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;

        let adapter_info = adapter.get_info();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Renderer device"),
                required_features: Self::FEATURES,
                required_limits: wgpu::Limits {
                    max_sampled_textures_per_shader_stage: 512,
                    max_binding_array_elements_per_shader_stage: 512,
                    max_push_constant_size: 128,
                    max_bind_groups: 6,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await?;

        let mut surface_config = surface
            .get_default_config(&adapter, size.0, size.1)
            .ok_or_else(|| anyhow!("Surface not compatible with adapter"))?;
        surface_config.format = surface_config.format.add_srgb_suffix();
        surface_config.present_mode = wgpu::PresentMode::AutoNoVsync;
        // surface_config.present_mode = wgpu::PresentMode::AutoVsync;

        surface.configure(&device, &surface_config);

        #[cfg(feature = "profiler")]
        let profiler = std::cell::RefCell::new(RendererProfiler {
            inner: GpuProfiler::new(
                &device,
                GpuProfilerSettings {
                    enable_debug_groups: false,
                    ..Default::default()
                },
            )?,
            results: vec![],
        });

        Ok(Self {
            adapter,
            adapter_info,
            device: Arc::new(device),
            queue,
            surface,
            surface_config,

            #[cfg(feature = "profiler")]
            profiler,
        })
    }

    pub fn resize(&mut self, (width, height): (u32, u32)) {
        if (width, height) == (self.surface_config.width, self.surface_config.height) {
            return;
        }

        self.surface_config.width = width;
        self.surface_config.height = height;
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
        let query = profiler.begin_query("RenderFrame", &mut encoder);

        let mut context = RenderContext {
            encoder: ProfilerCommandEncoder {
                encoder: &mut encoder,
                #[cfg(feature = "profiler")]
                profiler: &profiler,
            },
            frame: &frame_view,
        };

        cb(&mut context);

        #[cfg(feature = "profiler")]
        profiler.end_query(&mut encoder, query);

        #[cfg(feature = "profiler")]
        profiler.resolve_queries(&mut encoder);

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        #[cfg(feature = "profiler")]
        {
            profiler.end_frame().unwrap();

            if let Some(results) =
                profiler.process_finished_frame(self.queue.get_timestamp_period())
            {
                renderer_profiler.results = results
            }
        }

        Ok(())
    }
}

#[cfg(feature = "egui")]
impl<'window> egui::Widget for &Renderer<'window> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::CollapsingHeader::new("Adapter")
            .default_open(true)
            .show(ui, |ui| {
                let wgpu::AdapterInfo {
                    name,
                    driver,
                    driver_info,
                    ..
                } = &self.adapter_info;

                egui::Grid::new("EguiPass::AdapterInfo")
                    .num_columns(2)
                    .spacing([40.0, 0.0])
                    .show(ui, |ui| {
                        ui.label("Device");
                        ui.label(name);

                        ui.end_row();

                        ui.label("Driver");
                        ui.label(format!("{driver} ({driver_info})"));
                    });
            })
            .header_response
    }
}

pub struct RenderContext<'a> {
    pub encoder: ProfilerCommandEncoder<'a>,
    pub frame: &'a wgpu::TextureView,
}

#[cfg(feature = "profiler")]
pub struct RendererProfiler {
    inner: GpuProfiler,
    results: Vec<GpuTimerQueryResult>,
}

#[cfg(all(feature = "profiler", feature = "egui"))]
impl egui::Widget for &RendererProfiler {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        fn profiler_ui(results: &[GpuTimerQueryResult]) -> impl FnOnce(&mut egui::Ui) + '_ {
            move |ui| {
                let frame = egui::Frame {
                    inner_margin: egui::Margin {
                        left: 10,
                        ..Default::default()
                    },
                    ..Default::default()
                };

                for result in results {
                    ui.vertical(|ui| {
                        ui.columns(2, |columns| {
                            columns[0].label(&result.label);

                            columns[1].with_layout(
                                egui::Layout::right_to_left(egui::Align::TOP),
                                |ui| {
                                    let start =
                                        result.time.as_ref().map(|r| r.start).unwrap_or_default();
                                    let end =
                                        result.time.as_ref().map(|r| r.end).unwrap_or_default();

                                    let diff = end - start;
                                    let time = diff * 1000.0 * 1000.0;
                                    let time_str = format!("{time:.3}");
                                    ui.monospace(format!("{time_str} µs"));
                                },
                            )
                        });

                        frame.show(ui, profiler_ui(&result.nested_queries));
                    });
                }
            }
        }

        egui::CollapsingHeader::new("Profiler")
            .default_open(true)
            .show(ui, profiler_ui(&self.results))
            .header_response
    }
}

pub struct ProfilerCommandEncoder<'a> {
    encoder: &'a mut wgpu::CommandEncoder,
    #[cfg(feature = "profiler")]
    profiler: &'a GpuProfiler,
}

impl<'a> ProfilerCommandEncoder<'a> {
    pub fn profile_start(&mut self, label: &str) {
        #[cfg(debug_assertions)]
        self.encoder.push_debug_group(label);
    }

    pub fn profile_end(&mut self) {
        #[cfg(debug_assertions)]
        self.encoder.pop_debug_group();
    }

    #[cfg(feature = "profiler")]
    pub fn begin_compute_pass<'pass>(
        &'pass mut self,
        desc: &wgpu::ComputePassDescriptor<'pass>,
    ) -> OwningScope<'pass, wgpu::ComputePass<'pass>> {
        self.profiler.owning_scope(
            desc.label.unwrap_or("Unnamed compute pass"),
            self.encoder.begin_compute_pass(desc),
        )
    }

    #[cfg(feature = "profiler")]
    pub fn begin_render_pass<'pass>(
        &'pass mut self,
        desc: &wgpu::RenderPassDescriptor<'pass>,
    ) -> OwningScope<'pass, wgpu::RenderPass<'pass>> {
        self.profiler.owning_scope(
            desc.label.unwrap_or("Unnamed render pass"),
            self.encoder.begin_render_pass(desc).forget_lifetime(),
        )
    }

    #[cfg(feature = "profiler")]
    pub fn begin_manual_render_pass<'pass>(
        &'pass mut self,
        desc: &wgpu::RenderPassDescriptor<'pass>,
    ) -> ManualOwningScope<'pass, wgpu::RenderPass<'pass>> {
        self.profiler.manual_owning_scope(
            desc.label.unwrap_or("Unnamed render pass"),
            self.encoder.begin_render_pass(desc).forget_lifetime(),
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
