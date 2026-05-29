use anyhow::Result;

use crate::{RenderContext, Resource, ResourcesManager};

pub use egui_wgpu::Renderer as EguiRenderer;

impl Resource for egui_wgpu::Renderer {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        let device = resources.read::<wgpu::Device>();
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();

        Ok(egui_wgpu::Renderer::new(
            &device,
            surface_config.format,
            Default::default(),
        ))
    }
}

pub struct EguiPass {
    resources: ResourcesManager,

    paint_jobs: Vec<egui::ClippedPrimitive>,
    screen_descriptor: egui_wgpu::ScreenDescriptor,
}

impl EguiPass {
    pub fn new(resources: &ResourcesManager) -> Self {
        let resources = resources.clone();
        let surface_config = resources.read::<wgpu::SurfaceConfiguration>();

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point: 1.0,
        };

        Self {
            resources,

            paint_jobs: vec![],
            screen_descriptor,
        }
    }

    pub fn update(
        &mut self,
        context: &egui::Context,
        shapes: Vec<egui::epaint::ClippedShape>,
        textures_delta: egui::TexturesDelta,
        pixels_per_point: f32,
    ) {
        let device = self.resources.read::<wgpu::Device>();
        let queue = self.resources.read::<wgpu::Queue>();
        let surface_config = self.resources.read::<wgpu::SurfaceConfiguration>();
        let mut renderer = self.resources.write::<egui_wgpu::Renderer>();

        self.screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point,
        };

        self.paint_jobs = context.tessellate(shapes, self.screen_descriptor.pixels_per_point);

        for (texture_id, image_delta) in &textures_delta.set {
            renderer.update_texture(&device, &queue, *texture_id, image_delta);
        }
        for texture_id in &textures_delta.free {
            renderer.free_texture(texture_id);
        }

        let mut encoder = device.create_command_encoder(&Default::default());
        let mut profiler = self.resources.write::<wgpu_profiler::GpuProfiler>();
        let mut scope = profiler.scope("Egui[update]", &mut encoder);

        renderer.update_buffers(
            &device,
            &queue,
            &mut scope,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        drop(scope);

        profiler.resolve_queries(&mut encoder);

        queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn render(&self, ctx: &mut RenderContext) -> Result<()> {
        let renderer = self.resources.read::<egui_wgpu::Renderer>();

        renderer.render(
            &mut ctx
                .encoder
                .scope("Egui")
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: ctx.frame,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                })
                .forget_lifetime(),
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        Ok(())
    }
}

#[cfg(feature = "egui-winit")]
pub use self::winit::*;
#[cfg(feature = "egui-winit")]
mod winit {
    use winit::window::Window;

    use super::EguiPass;
    use crate::ResourcesManager;

    pub struct EguiWinitPass {
        pass: EguiPass,
        state: egui_winit::State,
    }

    impl EguiWinitPass {
        pub fn new(resources: &ResourcesManager, window: &Window) -> Self {
            let pass = EguiPass::new(resources);

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
            window: &winit::window::Window,
            run_ui: impl FnMut(&mut egui::Ui),
        ) {
            let input = self.state.take_egui_input(window);

            let output = self.state.egui_ctx().run_ui(input, run_ui);

            self.state
                .handle_platform_output(window, output.platform_output.clone());

            self.pass.update(
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
