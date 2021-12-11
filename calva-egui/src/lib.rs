use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use std::sync::Arc;
use std::time::Instant;
use winit::{event::Event, window::Window};

use renderer::{wgpu, RenderContext, Renderer};

pub use egui;
pub use epi;

struct RepaintSignal;

impl epi::RepaintSignal for RepaintSignal {
    fn request_repaint(&self) {
        println!("req");
    }
}

pub struct EguiPass {
    platform: Platform,
    rpass: RenderPass,
    previous_frame_time: Option<f32>,
    repaint_signal: Arc<RepaintSignal>,
}

impl EguiPass {
    pub fn new(renderer: &Renderer, window: &Window) -> Self {
        let platform = Platform::new(PlatformDescriptor {
            physical_width: renderer.surface_config.width as u32,
            physical_height: renderer.surface_config.height as u32,
            scale_factor: window.scale_factor(),
            font_definitions: Default::default(),
            style: Default::default(),
        });

        let rpass = RenderPass::new(&renderer.device, wgpu::TextureFormat::Bgra8UnormSrgb, 1);

        Self {
            platform,
            rpass,
            previous_frame_time: None,
            repaint_signal: Arc::new(RepaintSignal),
        }
    }

    pub fn handle_event<E>(&mut self, event: &Event<'_, E>) {
        self.platform.handle_event(event)
    }

    pub fn captures_event<E>(&mut self, event: &Event<'_, E>) -> bool {
        self.platform.captures_event(event)
    }

    pub fn render(
        &mut self,
        ctx: &mut RenderContext,
        window: &Window,
        app: &mut impl epi::App,
    ) -> Result<(), BackendError> {
        ctx.encoder.push_debug_group("Egui");

        let scale_factor = window.scale_factor() as f32;

        let egui_start = Instant::now();
        self.platform.begin_frame();
        let mut app_output = epi::backend::AppOutput::default();

        let mut epi_frame = epi::backend::FrameBuilder {
            info: epi::IntegrationInfo {
                name: "egui_wgpu",
                web_info: None,
                prefer_dark_mode: Some(true),
                cpu_usage: self.previous_frame_time,
                native_pixels_per_point: Some(scale_factor),
            },
            tex_allocator: &mut self.rpass,
            output: &mut app_output,
            repaint_signal: self.repaint_signal.clone(),
        }
        .build();

        app.update(&self.platform.context(), &mut epi_frame);
        let (_output, paint_commands) = self.platform.end_frame(Some(window));
        let paint_jobs = self.platform.context().tessellate(paint_commands);

        let frame_time = (Instant::now() - egui_start).as_secs_f64() as f32;
        self.previous_frame_time = Some(frame_time);

        self.rpass
            .update_texture(ctx.device, ctx.queue, &self.platform.context().texture());

        self.rpass.update_user_textures(ctx.device, ctx.queue);
        let screen_descriptor = ScreenDescriptor {
            physical_width: ctx.renderer.surface_config.width,
            physical_height: ctx.renderer.surface_config.height,
            scale_factor,
        };

        self.rpass
            .update_buffers(ctx.device, ctx.queue, &paint_jobs, &screen_descriptor);

        self.rpass.execute(
            &mut ctx.encoder,
            ctx.resolve_target.as_ref().unwrap_or(&ctx.view),
            &paint_jobs,
            &screen_descriptor,
            None,
        )?;

        ctx.encoder.pop_debug_group();

        Ok(())
    }
}
