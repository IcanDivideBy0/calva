#![warn(clippy::all)]

use anyhow::{anyhow, Result};
use async_std::task;
use calva::{
    gltf::GltfModel,
    renderer::{
        egui, CameraManager, EguiWinitPass, Engine, InstanceHandle, InstancesManager,
        PointLightHandle, PointLightsManager, Renderer, SkyboxManager,
    },
};
use std::{collections::HashMap, sync::Arc, time::Instant};
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

use worldgen::{
    navmesh::{NavMesh, NavMeshDebug},
    Chunk, Tile,
};

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = DemoApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

struct DemoState<'a> {
    window: Arc<Window>,
    camera: camera::MyCamera,
    renderer: Renderer<'a>,
    engine: Engine,
    egui: EguiWinitPass,
}

struct DemoApp<'a> {
    state: Option<DemoState<'a>>,

    worldgen: worldgen::WorldGenerator,
    worldgen_model: Option<GltfModel>,
    worldgen_chunks: HashMap<glam::IVec2, (Vec<InstanceHandle>, Vec<PointLightHandle>)>,

    navmesh: Option<NavMesh>,
    navmesh_debug: Option<NavMeshDebug>,

    monsters_models: Vec<GltfModel>,
    monsters_instances: Vec<InstanceHandle>,

    kb_modifiers: ModifiersState,
    render_time: Instant,
}

impl DemoApp<'_> {
    pub fn new() -> Self {
        Self {
            state: None,

            worldgen: worldgen::WorldGenerator::new("Calva!533d"),
            worldgen_model: None,
            worldgen_chunks: HashMap::new(),

            navmesh: None,
            navmesh_debug: None,

            monsters_models: vec![],
            monsters_instances: vec![],

            kb_modifiers: ModifiersState::empty(),
            render_time: Instant::now(),
        }
    }

    pub fn init_skybox(&mut self) -> Result<()> {
        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow!("Invalid state"))?;

        let pixels = [
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
        })?;

        state
            .engine
            .resources
            .get::<SkyboxManager>()
            .get_mut()
            .set_skybox(&state.renderer.device, &state.renderer.queue, &pixels);

        Ok(())
    }

    pub fn init_worldgen(&mut self) -> Result<()> {
        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow!("Invalid state"))?;

        use std::io::Read;

        let mut dungeon_buffer = Vec::new();
        std::fs::File::open("./demo/assets/dungeon.glb")?.read_to_end(&mut dungeon_buffer)?;
        let (doc, buffers, images) = gltf::import_slice(&dungeon_buffer)?;
        let worldgen_model =
            GltfModel::new(&state.renderer, &mut state.engine, doc, &buffers, &images)?;

        let tile_builder = worldgen::tile::TileBuilder::new(&state.renderer.device);

        let tiles = [
            "module01", "module03", "module07", "module08", "module09", "module10", "module11",
            "module12", "module13", "module14", "module15", "module16", "module17", "module18",
            "module19",
        ]
        .iter()
        .filter_map(|node_name| {
            tile_builder.build(
                &state.renderer.device,
                &state.renderer.queue,
                &buffers,
                worldgen_model.get_node(node_name)?,
            )
        })
        .collect::<Vec<_>>();

        self.worldgen.set_tiles(&tiles);

        let tile = &tiles[7];
        let navmesh = worldgen::navmesh::NavMesh::new(tile);
        // let navmesh_debug = worldgen::navmesh::NavMeshDebug::new(
        //     &state.renderer.device,
        //     &state.engine.resources.get::<CameraManager>().get(),
        //     &navmesh,
        //     state.renderer.surface_config.format,
        //     worldgen::navmesh::NavMeshDebugInput {
        //         depth: &state.engine.geometry.outputs.depth,
        //     },
        // );

        {
            let (instances, point_lights) = worldgen_model.node_instances(
                worldgen_model.doc.nodes().nth(tile.node_id).unwrap(),
                None,
                None,
            );
            state
                .engine
                .resources
                .get::<InstancesManager>()
                .get_mut()
                .add(&instances);
            state
                .engine
                .resources
                .get::<PointLightsManager>()
                .get_mut()
                .add(&state.renderer.queue, &point_lights);
        }

        self.worldgen_model = Some(worldgen_model);
        self.navmesh = Some(navmesh);
        // self.navmesh_debug = Some(navmesh_debug);

        Ok(())
    }

    pub fn init_monster_models(&mut self) -> Result<()> {
        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow!("Invalid state"))?;

        self.monsters_models = [
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
        // .take(1)
        .map(|filepath| GltfModel::from_path(&state.renderer, &mut state.engine, filepath))
        .collect::<Result<Vec<_>>>()?;

        Ok(())
    }
}

impl<'a> ApplicationHandler for DemoApp<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let mut camera = camera::MyCamera::new(window.inner_size().into());
        camera.controller.transform = glam::Mat4::look_at_rh(
            glam::Vec3::Y + glam::Vec3::Z * 12.0, // eye
            glam::Vec3::Y - glam::Vec3::Z,        // target
            glam::Vec3::Y,                        // up
        )
        .inverse();

        let renderer: Renderer<'a> =
            task::block_on(Renderer::new(window.clone(), window.inner_size().into())).unwrap();
        let mut engine = Engine::new(&renderer);

        engine.ambient_light.config.color = [0.106535, 0.061572, 0.037324];
        engine.ambient_light.config.strength = 0.1;

        let egui = EguiWinitPass::new(&renderer.device, &renderer.surface_config, &window);

        self.state = Some(DemoState {
            window,
            camera,
            renderer,
            engine,
            egui,
        });

        self.init_skybox().unwrap();
        self.init_worldgen().unwrap();
        self.init_monster_models().unwrap();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        if state.egui.on_event(&state.window, &event).consumed {
            return;
        }

        if state.camera.handle_event(&event) {
            return;
        }

        match event {
            WindowEvent::Resized(size) => {
                let size = size.into();

                state.camera.resize(size);
                state.renderer.resize(size);
                state.engine.resize(&state.renderer);
            }

            WindowEvent::RedrawRequested => {
                // Worldgen
                {
                    let (_, _, cam_pos) = state
                        .camera
                        .controller
                        .transform
                        .to_scale_rotation_translation();

                    let chunk_coord =
                        ((cam_pos + Tile::WORLD_SIZE * 0.5) / Chunk::WORLD_SIZE).floor();
                    let chunk_coord = glam::ivec2(chunk_coord.x as _, chunk_coord.z as _);
                    let chunk_x = (chunk_coord.x - 1)..=(chunk_coord.x + 1);
                    let chunk_y = (chunk_coord.y - 1)..=(chunk_coord.y + 1);

                    let instances_manager_resource =
                        state.engine.resources.get::<InstancesManager>();
                    let mut instances_manager = instances_manager_resource.get_mut();

                    let point_lights_manager_resource =
                        state.engine.resources.get::<PointLightsManager>();
                    let mut point_lights_manager = point_lights_manager_resource.get_mut();

                    self.worldgen_chunks
                        .retain(|pos, (instances, point_lights)| {
                            let should_remove =
                                !chunk_x.contains(&pos.x) || !chunk_y.contains(&pos.y);

                            if should_remove {
                                instances_manager.remove(instances);
                                point_lights_manager.remove(&state.renderer.queue, point_lights);
                            }

                            !should_remove
                        });

                    for key in
                        itertools::iproduct!(chunk_x, chunk_y).map(|(x, y)| glam::ivec2(x, y))
                    {
                        if let std::collections::hash_map::Entry::Vacant(entry) =
                            self.worldgen_chunks.entry(key)
                        {
                            let (instances, point_lights) = self
                                .worldgen
                                .chunk(self.worldgen_model.as_ref().unwrap(), key);

                            entry.insert((
                                instances_manager.add(&instances),
                                point_lights_manager.add(&state.renderer.queue, &point_lights),
                            ));
                        }
                    }
                }

                if let Some(navmesh_debug) = self.navmesh_debug.as_mut() {
                    navmesh_debug.rebind(worldgen::navmesh::NavMeshDebugInput {
                        depth: &state.engine.geometry.outputs.depth,
                    });
                };

                let dt = self.render_time.elapsed();
                self.render_time = Instant::now();

                **state.engine.animate.uniform = dt;

                state.camera.update(dt);
                ***state.engine.resources.get::<CameraManager>().get_mut() = (&state.camera).into();

                state.egui.update(&state.renderer, &state.window, |ctx| {
                    egui::SidePanel::right("engine_panel")
                        .min_width(320.0)
                        .frame(egui::containers::Frame {
                            inner_margin: egui::Vec2::splat(10.0).into(),
                            fill: egui::Color32::from_black_alpha(200),
                            ..Default::default()
                        })
                        .show(ctx, |ui| {
                            ui.add(&state.renderer);

                            ui.add(&mut *state.engine.ambient_light.config);
                            ui.add(&mut *state.engine.ssao.config);
                            ui.add(&mut *state.engine.tone_mapping.config);

                            egui::CollapsingHeader::new("Directional light")
                                .default_open(true)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        egui::color_picker::color_edit_button_rgb(
                                            ui,
                                            &mut state.engine.directional_light.uniform.light.color,
                                        );
                                        ui.add(
                                            egui::Label::new(egui::WidgetText::from("Color"))
                                                .wrap_mode(egui::TextWrapMode::Truncate),
                                        );
                                    });

                                    ui.add(
                                        egui::Slider::new(
                                            &mut state
                                                .engine
                                                .directional_light
                                                .uniform
                                                .light
                                                .intensity,
                                            0.0..=50.0,
                                        )
                                        .text("Intensity"),
                                    );

                                    ui.columns(2, |columns| {
                                        columns[0].add(
                                            egui::Slider::new(
                                                &mut state
                                                    .engine
                                                    .directional_light
                                                    .uniform
                                                    .light
                                                    .direction
                                                    .x,
                                                -1.5..=1.5,
                                            )
                                            .text("X"),
                                        );
                                        columns[1].add(
                                            egui::Slider::new(
                                                &mut state
                                                    .engine
                                                    .directional_light
                                                    .uniform
                                                    .light
                                                    .direction
                                                    .z,
                                                -1.5..=1.5,
                                            )
                                            .text("Z"),
                                        );
                                    });
                                });
                        });
                });

                state.engine.update(&state.renderer);

                let result = state.renderer.render(|ctx| {
                    state.engine.render(ctx);
                    // fog.render(ctx, &engine.resources.camera, &time);
                    if let Some(navmesh_debug) = self.navmesh_debug.as_ref() {
                        navmesh_debug
                            .render(ctx, &state.engine.resources.get::<CameraManager>().get())
                    }
                    state.egui.render(ctx);
                });

                match result {
                    Ok(_) => {}
                    // // Reconfigure the surface if lost
                    // Err(wgpu::SurfaceError::Lost) => renderer.resize((0, 0)),
                    // // The system is out of memory, we should probably quit
                    // Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{e:?}"),
                }

                // Emits a new redraw requested event.
                state.window.request_redraw();
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

            WindowEvent::ModifiersChanged(modifiers) => self.kb_modifiers = modifiers.state(),

            WindowEvent::KeyboardInput { event, .. } => match event {
                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::KeyR),
                    ..
                } => {
                    if let Some(handle) = self.monsters_instances.pop() {
                        state
                            .engine
                            .resources
                            .get::<InstancesManager>()
                            .get_mut()
                            .remove(&mut [handle])
                    }
                }

                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::KeyT),
                    ..
                } => {
                    for (z, ennemy) in self.monsters_models.iter().enumerate() {
                        for (x, animation) in ennemy.animations.values().enumerate() {
                            for y in 0..1 {
                                let transform = glam::Mat4::from_translation(glam::vec3(
                                    4.0 * x as f32,
                                    8.0 + 4.0 * y as f32,
                                    4.0 * z as f32,
                                ));

                                let instances_handles = state
                                    .engine
                                    .resources
                                    .get::<InstancesManager>()
                                    .get_mut()
                                    .add(
                                        &ennemy
                                            .scene_instances(
                                                None,
                                                Some(transform),
                                                Some(*animation),
                                            )
                                            .unwrap()
                                            .0,
                                    );

                                self.monsters_instances.extend(instances_handles);
                            }
                        }
                    }
                }

                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::Enter),
                    ..
                } if self.kb_modifiers.alt_key() => {
                    state
                        .window
                        .set_fullscreen(match state.window.fullscreen() {
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
