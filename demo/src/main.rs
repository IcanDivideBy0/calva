#![warn(clippy::all)]

use anyhow::Result;
use calva::{
    gltf::GltfModel,
    renderer::{
        egui::{self},
        CameraManager, EguiWinitPass, Engine, InstancesManager, LightsManager, Renderer,
        SkyboxManager,
    },
};
use std::sync::Arc;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

pub mod camera;
pub mod fog;
pub mod worldgen;

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = DemoApp::default();
    event_loop.run_app(&mut app).unwrap();

    Ok(())
}

#[derive(Default)]
struct DemoApp<'a> {
    state: Option<(
        Arc<Window>,
        camera::MyCamera,
        Renderer<'a>,
        Engine,
        EguiWinitPass,
        ModifiersState,
        Instant,
    )>,
}

impl<'a> ApplicationHandler for DemoApp<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let mut camera = camera::MyCamera::new(window.inner_size());
        camera.controller.transform = glam::Mat4::look_at_rh(
            glam::Vec3::Y + glam::Vec3::Z * 12.0, // eye
            glam::Vec3::Y - glam::Vec3::Z,        // target
            glam::Vec3::Y,                        // up
        )
        .inverse();

        let renderer: Renderer<'a> =
            pollster::block_on(Renderer::new(window.clone(), window.inner_size().into())).unwrap();
        let mut engine = Engine::new(&renderer);

        engine.ambient_light.config.color = [0.106535, 0.061572, 0.037324];
        engine.ambient_light.config.strength = 0.1;

        engine
            .ressources
            .get::<SkyboxManager>()
            .get_mut()
            .set_skybox(
                &renderer.device,
                &renderer.queue,
                &[
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
                .unwrap(),
            );

        let egui = EguiWinitPass::new(&renderer.device, &renderer.surface_config, &window);

        use std::io::Read;
        let mut dungeon_buffer = Vec::new();
        std::fs::File::open("./demo/assets/dungeon.glb")
            .unwrap()
            .read_to_end(&mut dungeon_buffer)
            .unwrap();
        let (doc, buffers, images) = gltf::import_slice(&dungeon_buffer).unwrap();
        let dungeon = GltfModel::new(&renderer, &mut engine, doc, &buffers, &images).unwrap();

        let tile_builder = worldgen::tile::TileBuilder::new(&renderer.device);

        let tiles = [
            "module01", "module03", "module07", "module08", "module09", "module10", "module11",
            "module12", "module13", "module14", "module15", "module16", "module17", "module18",
            "module19",
        ]
        .iter()
        .map(|node_name| {
            tile_builder.build(
                &renderer.device,
                &renderer.queue,
                &buffers,
                dungeon.get_node(node_name).unwrap(),
            )
        })
        .collect::<Vec<_>>();

        // let tile = &tiles[7];
        // let navmesh = worldgen::navmesh::NavMesh::new(tile);
        // let _navmesh_debug = worldgen::navmesh::NavMeshDebug::new(
        //     &renderer.device,
        //     &engine.ressources.get::<CameraManager>().get(),
        //     &navmesh,
        //     renderer.surface_config.format,
        //     worldgen::navmesh::NavMeshDebugInput {
        //         depth: &engine.geometry.outputs.depth,
        //     },
        // );

        // {
        //     let (instances, point_lights) =
        //         dungeon.node_instances(dungeon.doc.nodes().nth(tile.node_id).unwrap(), None, None);
        //     engine
        //         .ressources
        //         .get::<InstancesManager>()
        //         .get_mut()
        //         .add(&renderer.queue, instances);
        //     engine
        //         .ressources
        //         .get::<LightsManager>()
        //         .get_mut()
        //         .add_point_lights(&renderer.queue, &point_lights);
        // }

        let worldgen = worldgen::WorldGenerator::new(
            "Calva!533d", // rand::random::<u32>(),
            &tiles,
        );

        const DIM: i32 = 3;
        for x in -DIM..=DIM {
            for y in -DIM..=DIM {
                let res = worldgen.chunk(&dungeon, glam::ivec2(x, y));
                engine
                    .ressources
                    .get::<InstancesManager>()
                    .get_mut()
                    .add(&renderer.queue, res.0);
                engine
                    .ressources
                    .get::<LightsManager>()
                    .get_mut()
                    .add_point_lights(&renderer.queue, &res.1);
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
        .take(1)
        .map(|s| GltfModel::from_path(&renderer, &mut engine, s))
        .collect::<Result<Vec<_>>>()
        .unwrap();

        let mut instances = vec![];
        for (z, ennemy) in ennemies.iter().enumerate() {
            for (x, animation) in ennemy.animations.values().enumerate() {
                for y in 0..1 {
                    let transform = glam::Mat4::from_translation(glam::vec3(
                        4.0 * x as f32,
                        8.0 + 4.0 * y as f32,
                        4.0 * z as f32,
                    ));

                    instances.extend(
                        ennemy
                            .scene_instances(None, Some(transform), Some(*animation))
                            .unwrap()
                            .0,
                    );
                }
            }
        }
        engine
            .ressources
            .get::<InstancesManager>()
            .get_mut()
            .add(&renderer.queue, instances);

        let kb_modifiers = ModifiersState::empty();
        let render_time = Instant::now();

        self.state = Some((
            window,
            camera,
            renderer,
            engine,
            egui,
            kb_modifiers,
            render_time,
        ))
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let (
            window,
            ref mut camera,
            renderer,
            ref mut engine,
            ref mut egui,
            kb_modifiers,
            render_time,
        ) = if let Some(state) = self.state.as_mut() {
            state
        } else {
            return;
        };

        if egui.on_event(window, &event).consumed {
            return;
        }

        if camera.handle_event(&event) {
            return;
        }

        match event {
            WindowEvent::RedrawRequested => {
                let size = window.inner_size();
                camera.resize(size);
                renderer.resize(size.into());
                engine.resize(&renderer);

                // navmesh_debug.rebind(worldgen::navmesh::NavMeshDebugInput {
                //     depth: &engine.geometry.outputs.depth,
                // });

                let dt = render_time.elapsed();
                *render_time = Instant::now();

                camera.update(dt);

                egui.update(&renderer, &window, |ctx| {
                    egui::SidePanel::right("engine_panel")
                        .min_width(320.0)
                        .frame(egui::containers::Frame {
                            inner_margin: egui::Vec2::splat(10.0).into(),
                            fill: egui::Color32::from_black_alpha(200),
                            ..Default::default()
                        })
                        .show(ctx, |ui| {
                            ui.add(&*renderer);
                            ui.add(&*renderer.profiler.try_borrow().unwrap());

                            ui.add(&mut *engine.ambient_light.config);
                            ui.add(&mut *engine.ssao.config);
                            ui.add(&mut *engine.tone_mapping.config);

                            egui::CollapsingHeader::new("Directional light")
                                .default_open(true)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        egui::color_picker::color_edit_button_rgb(
                                            ui,
                                            &mut engine.directional_light.uniform.light.color,
                                        );
                                        ui.add(
                                            egui::Label::new(egui::WidgetText::from("Color"))
                                                .wrap_mode(egui::TextWrapMode::Truncate),
                                        );
                                    });

                                    ui.add(
                                        egui::Slider::new(
                                            &mut engine.directional_light.uniform.light.intensity,
                                            0.0..=50.0,
                                        )
                                        .text("Intensity"),
                                    );

                                    ui.columns(2, |columns| {
                                        columns[0].add(
                                            egui::Slider::new(
                                                &mut engine
                                                    .directional_light
                                                    .uniform
                                                    .light
                                                    .direction
                                                    .x,
                                                -1.0..=1.0,
                                            )
                                            .text("X"),
                                        );
                                        columns[1].add(
                                            egui::Slider::new(
                                                &mut engine
                                                    .directional_light
                                                    .uniform
                                                    .light
                                                    .direction
                                                    .z,
                                                -1.0..=1.0,
                                            )
                                            .text("Z"),
                                        );
                                    });
                                });
                        });
                });

                ***engine.ressources.get::<CameraManager>().get_mut() = (&*camera).into();
                **engine.animate.uniform = dt;
                engine.update(&renderer);

                let result = renderer.render(|ctx| {
                    engine.render(ctx);
                    // fog.render(ctx, &engine.ressources.camera, &time);
                    // navmesh_debug.render(ctx, &engine.ressources.get::<CameraManager>().get());
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

                // Emits a new redraw requested event.
                window.request_redraw();
            }

            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),

            WindowEvent::ModifiersChanged(modifiers) => *kb_modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => match event {
                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::Enter),
                    ..
                } if kb_modifiers.alt_key() => {
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
}
