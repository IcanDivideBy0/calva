#![warn(clippy::all)]

use anyhow::{anyhow, Result};
use async_std::task;
use calva::{
    gltf::GltfModel,
    nav::HeatMap,
    renderer::{
        egui, AmbientLightConfig, AnimateUniform, Camera, CameraManager, EguiWinitPass, Engine,
        Object, Renderer, SkyboxManager, SsaoConfig, ToneMappingConfig, UniformBuffer,
    },
};
use core::f32;
use glam::Vec3Swizzles;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
    time::Instant,
};
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

pub mod camera;
pub mod controls;
pub mod debug;
pub mod fog;
pub mod worldgen;

use worldgen::{Chunk, Tile};

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
    mouse_pos: glam::Vec2,
    camera: camera::PerspectiveCamera,
    flying_camera: controls::FlyingCamera,
    player_controller: controls::PlayerController,
    renderer: Renderer<'a>,
    engine: Engine,
    egui: EguiWinitPass,

    monster_objects: Vec<Object>,
}

struct DemoApp<'a> {
    state: Option<DemoState<'a>>,

    worldgen: worldgen::WorldGenerator,
    worldgen_model: Option<GltfModel>,
    worldgen_chunks: HashMap<glam::IVec2, Vec<Object>>,

    height_map: Option<calva::nav::HeightMap<{ Tile::TEXTURE_SIZE }>>,
    heat_map: Option<calva::nav::HeatMap<{ Tile::TEXTURE_SIZE }>>,
    navgrid_debug: Option<debug::Debug>,

    monsters_models: Vec<GltfModel>,

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

            height_map: None,
            heat_map: None,
            navgrid_debug: None,

            monsters_models: vec![],

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
            .write::<SkyboxManager>()
            .set_skybox(&pixels);

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
        let worldgen_model = GltfModel::new(&state.engine.resources, doc, &buffers, &images)?;

        let tile_builder = worldgen::tile::TileBuilder::new(&state.engine.resources.device);

        let tiles = [
            "module01", "module03", "module07", "module08", "module09", "module10", "module11",
            "module12", "module13", "module14", "module15", "module16", "module17", "module18",
            "module19",
        ]
        .iter()
        .filter_map(|node_name| {
            tile_builder.build(
                &state.engine.resources.device,
                &state.engine.resources.queue,
                &buffers,
                worldgen_model.get_node(node_name)?,
            )
        })
        .collect::<Vec<_>>();

        self.worldgen.set_tiles(&tiles);

        let tile = &tiles[7];
        let height_map = calva::nav::HeightMap::new(&tile.height_map, Tile::PIXEL_SIZE);
        dbg!(&height_map);

        self.navgrid_debug = Some(debug::Debug::new(
            &state.engine.resources.device,
            &state.engine.resources.read::<CameraManager>(),
            &height_map.triangles,
            state.renderer.surface_config.format,
            debug::DebugInput {
                depth: &state.engine.geometry.outputs.depth,
            },
        ));

        worldgen_model
            .node_object(worldgen_model.doc.nodes().nth(tile.node_id).unwrap())
            .with_static(true);

        self.worldgen_model = Some(worldgen_model);
        self.height_map = Some(height_map);

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
        .map(|filepath| GltfModel::from_path(&state.engine.resources, filepath))
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
        let mouse_pos = Default::default();

        let camera = camera::PerspectiveCamera::new(window.inner_size().into());

        let flying_camera = controls::FlyingCamera::from_look_at(
            glam::Vec3::Y + glam::Vec3::Z * 12.0, // eye
            glam::Vec3::Y - glam::Vec3::Z,        // target
            glam::Vec3::Y,                        // up
        );

        let player_controller = controls::PlayerController::default();

        let renderer: Renderer<'a> = task::block_on(Renderer::new(
            Box::new(event_loop.owned_display_handle()),
            window.clone(),
            window.inner_size().into(),
        ))
        .unwrap();
        let engine = Engine::new(&renderer);

        let mut ambient_light_config = engine
            .resources
            .write::<UniformBuffer<AmbientLightConfig>>();
        ambient_light_config.color = [0.106535, 0.061572, 0.037324];
        ambient_light_config.strength = 0.1;

        let egui = EguiWinitPass::new(&engine.resources, &renderer.surface_config, &window);

        self.state = Some(DemoState {
            window,
            mouse_pos,
            camera,
            flying_camera,
            player_controller,
            renderer,
            engine,
            egui,
            monster_objects: vec![],
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

        if state.flying_camera.handle_event(&event) {
            // return;
        }

        if state.player_controller.handle_event(&event) {
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
                        .flying_camera
                        .transform
                        .to_scale_rotation_translation();

                    let chunk_coord =
                        ((cam_pos + Tile::WORLD_SIZE * 0.5) / Chunk::WORLD_SIZE).floor();
                    let chunk_coord = glam::ivec2(chunk_coord.x as _, chunk_coord.z as _);
                    let chunk_x = (chunk_coord.x - 1)..=(chunk_coord.x + 1);
                    let chunk_y = (chunk_coord.y - 1)..=(chunk_coord.y + 1);

                    self.worldgen_chunks
                        .retain(|pos, _| chunk_x.contains(&pos.x) && chunk_y.contains(&pos.y));

                    for key in
                        itertools::iproduct!(chunk_x, chunk_y).map(|(x, y)| glam::ivec2(x, y))
                    {
                        if let Entry::Vacant(_entry) = self.worldgen_chunks.entry(key) {
                            // let model = self.worldgen_model.as_ref().unwrap();
                            // entry.insert(self.worldgen.chunk(model, key));
                        }
                    }
                }

                // Update monster pos
                if let (Some(height_map), Some(heat_map)) = (&self.height_map, &self.heat_map) {
                    for monster in &mut state.monster_objects {
                        let mut transform = monster.transform();
                        let (_, _, translation) = transform.to_scale_rotation_translation();

                        let grid_coord = Tile::get_grid_coord(&translation.xz());

                        let dir = heat_map.apply_kernel(grid_coord) * Tile::PIXEL_SIZE / 4.0;

                        let new_grid_coord = Tile::get_grid_coord(&(translation.xz() + dir));

                        let dh = height_map.get_height(&new_grid_coord).unwrap_or_default()
                            - height_map.get_height(&grid_coord).unwrap_or_default();

                        transform =
                            glam::Mat4::from_translation(glam::vec3(dir.x, dh, dir.y)) * transform;

                        monster.set_transform(transform);
                    }
                }

                if let Some(navgrid_debug) = self.navgrid_debug.as_mut() {
                    navgrid_debug.rebind(debug::DebugInput {
                        depth: &state.engine.geometry.outputs.depth,
                    });
                };

                let dt = self.render_time.elapsed();
                self.render_time = Instant::now();

                ***state
                    .engine
                    .resources
                    .write::<UniformBuffer<AnimateUniform>>() = dt;

                state.flying_camera.update(dt);
                ***state.engine.resources.write::<CameraManager>() = Camera {
                    view: state.flying_camera.get_view(),
                    proj: state.camera.get_proj(),
                };

                state.egui.update(&state.renderer, &state.window, |ui| {
                    egui::Panel::right("engine_panel")
                        .min_size(320.0)
                        .frame(egui::containers::Frame {
                            inner_margin: egui::Vec2::splat(10.0).into(),
                            fill: egui::Color32::from_black_alpha(200),
                            ..Default::default()
                        })
                        .show_inside(ui, |ui| {
                            ui.add(&state.renderer);

                            let resources = &state.engine.resources;

                            ui.add(&mut **resources.write::<UniformBuffer<AmbientLightConfig>>());
                            ui.add(&mut **resources.write::<UniformBuffer<SsaoConfig>>());
                            ui.add(&mut **resources.write::<UniformBuffer<ToneMappingConfig>>());

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

                state.engine.update().unwrap();

                let result = state.renderer.render(|ctx| {
                    state.engine.render(ctx);
                    // fog.render(ctx, &engine.resources.camera, &time);
                    if let Some(navgrid_debug) = self.navgrid_debug.as_ref() {
                        navgrid_debug.render(ctx, &state.engine.resources.read::<CameraManager>())
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

            WindowEvent::CursorMoved { position, .. } => {
                state.mouse_pos = glam::Vec2::new(position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                if let Some(height_map) = self.height_map.as_ref() {
                    let camera = state.engine.resources.read::<CameraManager>();

                    let (ro, rd) = camera.ray_cast(
                        state.mouse_pos,
                        glam::vec2(
                            state.renderer.surface_config.width as f32,
                            state.renderer.surface_config.height as f32,
                        ),
                    );

                    if let Some(hit) = height_map.ray_cast(ro, rd) {
                        if button == MouseButton::Left {
                            let grid_coord = Tile::get_grid_coord(&hit.xz());
                            let heat_map = HeatMap::new(height_map, grid_coord);
                            dbg!(&heat_map);
                            self.heat_map = Some(heat_map);
                        }

                        if button == MouseButton::Right {
                            let transform = glam::Mat4::from_translation(hit)
                                * glam::Mat4::from_axis_angle(
                                    glam::Vec3::Y,
                                    rand::random::<f32>() * f32::consts::TAU,
                                );

                            let monster_model = &self.monsters_models
                                [rand::random::<u32>() as usize % self.monsters_models.len()];

                            let animation_keys =
                                monster_model.animations.keys().collect::<Vec<_>>();
                            let animation = &monster_model.animations[animation_keys
                                [rand::random::<u32>() as usize % animation_keys.len()]];

                            state.monster_objects.push(
                                monster_model
                                    .object()
                                    .with_transform(transform)
                                    .with_animation((*animation).into()),
                            );
                        }
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => match event {
                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::KeyR),
                    ..
                } => {
                    state.monster_objects.pop();
                }

                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::KeyT),
                    ..
                } => {
                    for (z, monster_model) in self.monsters_models.iter().enumerate() {
                        for (x, animation) in monster_model.animations.values().enumerate() {
                            for y in 0..1 {
                                let transform = glam::Mat4::from_translation(glam::vec3(
                                    4.0 * x as f32,
                                    8.0 + 4.0 * y as f32,
                                    4.0 * z as f32,
                                ));

                                state.monster_objects.push(
                                    monster_model
                                        .object()
                                        .with_transform(transform)
                                        .with_animation((*animation).into()),
                                );
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
