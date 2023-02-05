#![warn(clippy::all)]

use anyhow::Result;
use calva::{
    egui::{egui, EguiPass, EguiWinitPass},
    gltf::GltfModel,
    renderer::{Engine, Renderer},
};
use std::time::Instant;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window,
};

mod camera;

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = window::WindowBuilder::new()
        // .with_fullscreen(Some(window::Fullscreen::Borderless(None)))
        .build(&event_loop)?;

    let mut camera = camera::MyCamera::new(&window);
    camera.controller.transform = glam::Mat4::look_at_rh(
        glam::Vec3::Y + glam::Vec3::Z * 12.0, // eye
        glam::Vec3::Y - glam::Vec3::Z,        // target
        glam::Vec3::Y,                        // up
    )
    .inverse();

    let mut renderer = Renderer::new(&window, window.inner_size().into()).await?;
    let mut engine = Engine::new(&renderer);

    engine.config.skybox = [
        "./demo/assets/sky/right.jpg",
        "./demo/assets/sky/left.jpg",
        "./demo/assets/sky/top.jpg",
        "./demo/assets/sky/bottom.jpg",
        "./demo/assets/sky/front.jpg",
        "./demo/assets/sky/back.jpg",
    ]
    .iter()
    .try_fold(vec![], |mut bytes, filepath| {
        let image = image::open(filepath)?;
        bytes.append(&mut image.to_rgba8().to_vec());
        Ok::<_, image::ImageError>(bytes)
    })
    .ok()
    .map(|pixels| engine.create_skybox(&renderer, &pixels));

    let mut egui = EguiWinitPass::new(&renderer, &event_loop);

    let dungeon = GltfModel::from_path(&renderer, &mut engine, "./demo/assets/dungeon.glb")?;
    dungeon.instanciate(&renderer, &mut engine, &[(glam::Mat4::IDENTITY, None)]);

    let zombie = GltfModel::from_path(&renderer, &mut engine, "./demo/assets/zombie.glb")?;
    let zombie_anims = zombie.animations.keys().collect::<Vec<_>>();
    zombie.instanciate(
        &renderer,
        &mut engine,
        &(0..60_000)
            .map(|i| {
                let x = 4.0 * (i % 100) as f32;
                let z = 4.0 * (i / 100) as f32;
                let transform = glam::Mat4::from_translation(glam::vec3(x, 0.0, z));

                let anim = zombie_anims[i % zombie_anims.len()];

                (transform, Some(anim.as_str()))
            })
            .collect::<Vec<_>>(),
    );

    let mut render_time = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::RedrawRequested(_) => {
                let size = window.inner_size().into();
                camera.resize(size);
                renderer.resize(size);
                engine.resize(&renderer);

                let dt = render_time.elapsed();
                render_time = Instant::now();

                camera.controller.update(dt);
                renderer.camera.update(
                    &renderer.queue,
                    camera.controller.transform.inverse(),
                    camera.projection.into(),
                );

                let egui_output = egui.run(&window, |ctx| {
                    egui::SidePanel::right("engine_panel")
                        .min_width(320.0)
                        .frame(egui::containers::Frame {
                            inner_margin: egui::Vec2::splat(10.0).into(),
                            fill: egui::Color32::from_black_alpha(200),
                            ..Default::default()
                        })
                        .show(ctx, |ui| {
                            EguiPass::engine_config_ui(&mut engine)(ui);
                            EguiPass::renderer_ui(&renderer)(ui);
                        });
                });

                let result = renderer.render(|ctx| {
                    engine.render(ctx, dt);
                    egui.render(ctx, &window, egui_output);
                });

                match result {
                    Ok(_) => {}
                    // // Reconfigure the surface if lost
                    // Err(wgpu::SurfaceError::Lost) => renderer.resize(0, 0),
                    // // The system is out of memory, we should probably quit
                    // Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{e:?}"),
                }
            }

            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually request it.
                window.request_redraw();
            }

            Event::WindowEvent { ref event, .. } => {
                if egui.on_event(event).consumed {
                    return;
                }

                if camera.handle_event(event) {
                    return;
                }

                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    _ => {}
                }
            }

            _ => {}
        }
    });
}
