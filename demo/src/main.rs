#![warn(clippy::all)]

use anyhow::Result;
use calva::{
    egui::{egui, EguiPass, EguiWinitPass},
    gltf::GltfModel,
    renderer::{DirectionalLight, Engine, Renderer},
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
    dungeon.instanciate(
        &renderer,
        &mut engine,
        &vec![glam::vec3(-20.0, 0.0, 0.0), glam::vec3(20.0, 0.0, 0.0)]
            .iter()
            .map(|translation| (glam::Mat4::from_translation(*translation), None))
            .collect::<Vec<_>>(),
    );

    let zombie = GltfModel::from_path(&renderer, &mut engine, "./demo/assets/zombie.glb")?;
    let zombie_anims = zombie.animations.keys().collect::<Vec<_>>();
    zombie.instanciate(
        &renderer,
        &mut engine,
        &(0..600)
            .map(|i| {
                (
                    glam::Mat4::from_translation(glam::vec3(
                        4.0 * (i % 50) as f32,
                        0.0,
                        4.0 * (i / 50) as f32,
                    )),
                    Some(zombie_anims[i % zombie_anims.len()].as_str()),
                )
            })
            .collect::<Vec<_>>(),
    );

    let mut directional_light = DirectionalLight {
        color: glam::vec4(1.0, 1.0, 1.0, 1.0),
        direction: glam::vec3(-1.0, -1.0, -1.0),
    };

    let mut render_time = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually request it.
                window.request_redraw();
            }

            Event::RedrawRequested(_) => {
                let size = window.inner_size().into();
                camera.resize(size);
                renderer.resize(size);
                engine.resize(&renderer);

                let dt = render_time.elapsed();
                render_time = Instant::now();

                camera.controller.update(dt);

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

                            egui::CollapsingHeader::new("Directional light")
                                .default_open(true)
                                .show(ui, |ui| {
                                    ui.columns(2, |columns| {
                                        columns[0].add(
                                            egui::Slider::new(
                                                &mut directional_light.direction.x,
                                                -1.0..=1.0,
                                            )
                                            .text("X"),
                                        );
                                        columns[1].add(
                                            egui::Slider::new(
                                                &mut directional_light.direction.z,
                                                -1.0..=1.0,
                                            )
                                            .text("Z"),
                                        );
                                    });
                                });

                            EguiPass::renderer_ui(&renderer)(ui);
                        });
                });
                egui.update(&renderer, &window, egui_output);

                engine.update(
                    &renderer,
                    camera.controller.transform.inverse(),
                    camera.projection.into(),
                    &directional_light,
                );

                let result = renderer.render(|ctx| {
                    engine.render(ctx, dt);
                    egui.render(ctx);
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
