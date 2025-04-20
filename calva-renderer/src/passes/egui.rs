use crate::{RenderContext, Renderer};

pub struct EguiPass {
    paint_jobs: Vec<egui::ClippedPrimitive>,
    screen_descriptor: egui_wgpu::ScreenDescriptor,
    egui_renderer: egui_wgpu::Renderer,
}

impl EguiPass {
    pub fn new(device: &wgpu::Device, surface_config: &wgpu::SurfaceConfiguration) -> Self {
        let egui_renderer =
            egui_wgpu::Renderer::new(device, surface_config.format.clone(), None, 1, false);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point: 1.0,
        };

        Self {
            paint_jobs: vec![],
            screen_descriptor,
            egui_renderer,
        }
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        context: &egui::Context,
        shapes: Vec<egui::epaint::ClippedShape>,
        textures_delta: egui::TexturesDelta,
        pixels_per_point: f32,
    ) {
        self.screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [
                renderer.surface_config.width,
                renderer.surface_config.height,
            ],
            pixels_per_point,
        };

        self.paint_jobs = context.tessellate(shapes, self.screen_descriptor.pixels_per_point);

        for (texture_id, image_delta) in &textures_delta.set {
            self.egui_renderer.update_texture(
                &renderer.device,
                &renderer.queue,
                *texture_id,
                image_delta,
            );
        }
        for texture_id in &textures_delta.free {
            self.egui_renderer.free_texture(texture_id);
        }

        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        self.egui_renderer.update_buffers(
            &renderer.device,
            &renderer.queue,
            &mut encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );
        renderer.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn render(&self, ctx: &mut RenderContext) {
        let color_attachments = [Some(wgpu::RenderPassColorAttachment {
            view: ctx.frame,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];

        let pass_desc = wgpu::RenderPassDescriptor {
            label: Some("Egui"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            ..Default::default()
        };

        #[cfg(feature = "profiler")]
        let pass = ctx.encoder.begin_manual_render_pass(&pass_desc).end_query();

        #[cfg(not(feature = "profiler"))]
        let pass = ctx.encoder.begin_render_pass(&pass_desc);

        self.egui_renderer.render(
            &mut pass.forget_lifetime(),
            &self.paint_jobs,
            &self.screen_descriptor,
        );
    }
}

#[cfg(feature = "winit")]
pub use self::winit::*;
#[cfg(feature = "winit")]
mod winit {
    use winit::window::Window;

    use super::EguiPass;
    use crate::Renderer;

    pub struct EguiWinitPass {
        pass: EguiPass,
        state: egui_winit::State,
    }

    impl EguiWinitPass {
        pub fn new(
            device: &wgpu::Device,
            surface_config: &wgpu::SurfaceConfiguration,
            window: &Window,
        ) -> Self {
            let pass = EguiPass::new(device, surface_config);

            let state = egui_winit::State::new(
                egui::Context::default(),
                egui::viewport::ViewportId::ROOT,
                window,
                None,
                None,
                None,
            );

            Self { pass, state }
        }

        pub fn on_event(
            &mut self,
            window: &winit::window::Window,
            event: &winit::event::WindowEvent,
        ) -> egui_winit::EventResponse {
            self.state.on_window_event(window, event)
        }

        pub fn update(
            &mut self,
            renderer: &Renderer,
            window: &winit::window::Window,
            ui: impl FnMut(&egui::Context),
        ) {
            let input = self.state.take_egui_input(window);

            let output = self.state.egui_ctx().run(input, ui);

            self.state
                .handle_platform_output(window, output.platform_output.clone());

            self.pass.update(
                renderer,
                self.state.egui_ctx(),
                output.shapes,
                output.textures_delta,
                output.pixels_per_point,
            );
        }
    }

    impl std::ops::Deref for EguiWinitPass {
        type Target = EguiPass;

        fn deref(&self) -> &Self::Target {
            &self.pass
        }
    }
}
