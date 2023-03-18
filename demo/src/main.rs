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
    window::{Fullscreen, WindowBuilder},
};

mod camera;
mod navmesh;
mod worldgen;
// mod fog;

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

    engine.config.ambient.factor = 0.05;
    engine.config.tone_mapping.exposure = 0.05;

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

    use std::io::Read;
    let mut dungeon_buffer = Vec::new();
    std::fs::File::open("./demo/assets/dungeon.glb")?.read_to_end(&mut dungeon_buffer)?;
    let (doc, buffers, images) = gltf::import_slice(&dungeon_buffer)?;

    let navmesh = navmesh::NavMesh::new(&renderer, &engine.camera, &doc, &buffers);
    let dungeon = GltfModel::new(&renderer, &mut engine, doc, &buffers, &images)?;
    // if let Some((instances, point_lights)) = dungeon.node_instances("module01", None, None) {
    //     engine.instances.add(&renderer.queue, instances);
    //     engine
    //         .lights
    //         .add_point_lights(&renderer.queue, &point_lights);
    // }

    let worldgen = worldgen::WorldGenerator::new(
        "Calva!533d", // rand::random::<u32>(),
        // GltfModel::from_path(&renderer, &mut engine, "./demo/assets/dungeon.glb")?,
        // GltfModel::new(&renderer, &mut engine, doc, &buffers, &images)?,
        dungeon,
    );

    for x in -3..3 {
        for y in -3..3 {
            let res = worldgen.chunk(glam::ivec2(x, y));
            engine.instances.add(&renderer.queue, res.0);
            engine.lights.add_point_lights(&renderer.queue, &res.1);
        }
    }

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
    .take(0)
    .map(|s| GltfModel::from_path(&renderer, &mut engine, s))
    .collect::<Result<Vec<_>>>()?;

    let mut instances = vec![];
    for (z, ennemy) in ennemies.iter().enumerate() {
        for (x, animation) in ennemy.animations().enumerate() {
            for y in 0..1 {
                let transform = glam::Mat4::from_translation(glam::vec3(
                    4.0 * x as f32,
                    8.0 + 4.0 * y as f32,
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
        color: glam::vec3(1.0, 1.0, 1.0),
        intensity: 100.0,
        direction: glam::vec3(-1.0, -1.0, -1.0),
    };

    // let fog = fog::FogPass::new(&renderer, &engine.camera);

    let mut kb_modifiers = ModifiersState::empty();
    // let time = Instant::now();
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

                egui.update(&renderer, &window, |ctx| {
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

                engine.update(
                    &renderer,
                    camera.controller.transform.inverse(),
                    camera.projection.into(),
                    &directional_light,
                );

                let result = renderer.render(|ctx| {
                    engine.render(ctx, dt);
                    // fog.render(ctx, &engine.camera, &time);
                    navmesh.render(ctx, &engine.camera);
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
