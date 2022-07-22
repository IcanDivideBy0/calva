#![warn(clippy::all)]

use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use std::time::Instant;
use winit::{event::Event, window::Window};

use renderer::{wgpu, RenderContext, Renderer};

pub use egui;

pub trait App {
    fn ui(&mut self, ctx: &egui::Context);
}

pub struct EguiPass {
    platform: Platform,
    rpass: RenderPass,
    previous_frame_time: Option<f32>,
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
        app: &mut impl App,
    ) -> Result<(), BackendError> {
        ctx.encoder.push_debug_group("Egui");

        let scale_factor = window.scale_factor() as f32;

        let egui_start = Instant::now();
        self.platform.begin_frame();

        app.ui(&self.platform.context());

        let output = self.platform.end_frame(Some(window));
        let paint_jobs = self.platform.context().tessellate(output.shapes);

        let frame_time = (Instant::now() - egui_start).as_secs_f64() as f32;
        self.previous_frame_time = Some(frame_time);

        self.rpass.add_textures(
            &ctx.renderer.device,
            &ctx.renderer.queue,
            &output.textures_delta,
        )?;

        let screen_descriptor = ScreenDescriptor {
            physical_width: ctx.renderer.surface_config.width,
            physical_height: ctx.renderer.surface_config.height,
            scale_factor,
        };

        self.rpass.update_buffers(
            &ctx.renderer.device,
            &ctx.renderer.queue,
            &paint_jobs,
            &screen_descriptor,
        );

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
