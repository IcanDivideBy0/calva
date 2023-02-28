#![warn(clippy::all)]

use anyhow::{anyhow, Result};
use calva::{
    egui::{egui, EguiPass, EguiWinitPass},
    gltf::GltfModel,
    renderer::{DirectionalLight, Engine, Renderer},
};
use std::time::Instant;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, WindowBuilder},
};

mod camera;
// mod dungen;
// mod dungen2;
// mod dungen3;
// mod dungen4;

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

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
    engine.instances.add(
        &renderer.queue,
        dungeon
            .scene_instances(Some("modules"), None, None)
            .ok_or_else(|| anyhow!("Unable to load dungeon scene"))?
            .0,
    );

    // dungen::Dungen::new(&renderer, &mut engine, None)?.gen(&renderer, &mut engine);

    // let mut dungen = dungen3::Chunk::new(&renderer, &mut engine, Some(1841186548))?;
    // while !dungen.collapsed() {
    //     dungen.solve();
    // }
    // dungen.instanciate(&renderer, &mut engine);

    // dungen::Dungen::new(&renderer, &mut engine, None)?.gen(&renderer, &mut engine);

    // let dungen = dungen4::Dungen::new(rand::random::<u32>());
    // engine.instances.add(
    //     &renderer.queue,
    //     dungen.chunk((0, 0).into()).instanciate(&dungeon),
    // );

    // engine.instances.add(
    //     &renderer.queue,
    //     vec![glam::vec3(-20.0, 0.0, 0.0), glam::vec3(20.0, 0.0, 0.0)]
    //         .iter()
    //         .flat_map(|&t| {
    //             dungeon
    //                 .scene_data(Some("default"), glam::Mat4::from_translation(t), None)
    //                 .0
    //         }),
    // );

    let ennemies = [
        "./demo/assets/zombies/zombie-boss.glb",
        "./demo/assets/zombies/zombie-common.glb",
        "./demo/assets/zombies/zombie-fat.glb",
        "./demo/assets/zombies/zombie-murderer.glb",
        "./demo/assets/zombies/zombie-snapper.glb",
        "./demo/assets/skeletons/skeleton-archer.glb",
        "./demo/assets/skeletons/skeleton-grunt.glb",
        "./demo/assets/skeletons/skeleton-mage.glb",
        "./demo/assets/skeletons/skeleton-king.glb",
        "./demo/assets/skeletons/skeleton-swordsman.glb",
        "./demo/assets/demons/demon-bomb.glb",
        "./demo/assets/demons/demon-boss.glb",
        "./demo/assets/demons/demon-fatty.glb",
        "./demo/assets/demons/demon-grunt.glb",
        "./demo/assets/demons/demon-imp.glb",
    ]
    .iter()
    .take(1)
    .map(|s| GltfModel::from_path(&renderer, &mut engine, s))
    .collect::<Result<Vec<_>>>()?;

    let mut instances = vec![];
    for (z, ennemy) in ennemies.iter().enumerate() {
        for (x, animation) in ennemy.animations.keys().enumerate() {
            for y in 0..1 {
                let transform = glam::Mat4::from_translation(glam::vec3(
                    4.0 * x as f32,
                    4.0 * y as f32,
                    4.0 * z as f32,
                ));

                instances.extend(
                    ennemy
                        .scene_instances(None, Some(animation), Some(transform))
                        .unwrap()
                        .0,
                );
            }
        }
    }
    engine.instances.add(&renderer.queue, instances);

    let mut directional_light = DirectionalLight {
        color: glam::vec4(1.0, 1.0, 1.0, 1.0),
        direction: glam::vec3(-1.0, -1.0, -1.0),
    };

    let mut kb_modifiers = ModifiersState::empty();
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

                            // if ui.button("solve").clicked() {
                            //     engine.instances.add(&renderer.queue, dungen.solve())
                            // }

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

                    WindowEvent::ModifiersChanged(modifiers) => kb_modifiers = *modifiers,
                    WindowEvent::KeyboardInput { input, .. } => match input {
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Return),
                            ..
                        } if kb_modifiers.alt() => {
                            window.set_fullscreen(match window.fullscreen() {
                                None => Some(Fullscreen::Borderless(None)),
                                _ => None,
                            });
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }

            _ => {}
        }
    });
}
