use anyhow::Result;
use calva::{
    gltf::GltfModel,
    nav::{HeatMap, HeightMap, HeightMapBuilder},
    renderer::{Camera, Object, Resource, ResourcesManager},
};
use glam::Vec3Swizzles;
use noise::NoiseFn;
use rand::RngExt;
use rand_seeder::SipHasher;
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    fs::File,
    hash::Hash,
    io::Read,
};

mod chunk;
mod wfc;

use chunk::WorldChunk;
use wfc::Wfc;

use crate::{controls::TopDownCamera, worldgen::wfc::ModuleRotation};

const SEED: &str = "Calva!533d";

type Chunk = WorldChunk<{ WorldGenerator::CHUNK_SIZE }, { WorldGenerator::WFC_MODULE_SIZE }>;

pub struct WorldGenerator {
    model: GltfModel,
    height_maps: BTreeMap<usize, HeightMap>, // Needs to be a sorted structure for wfc to produce stable results w/ rng

    seed: u32,
    noise: Box<dyn NoiseFn<f64, 2> + Send + Sync>,
    wfc: Wfc<{ Self::CHUNK_SIZE }, { Self::WFC_MODULE_SIZE }>,
    chunks: HashMap<glam::IVec2, (Chunk, Vec<Object>)>,
    main_chunk: glam::IVec2,
}

impl WorldGenerator {
    pub const TILE_WORLD_SIZE: f32 = 5.0 * 6.0;
    pub const CHUNK_SIZE: usize = 3;
    pub const WFC_MODULE_SIZE: usize = 5;

    fn new(seed: impl Hash, model: GltfModel, height_maps: BTreeMap<usize, HeightMap>) -> Self {
        let seed = SipHasher::from(seed).into_rng().random();

        let noise = Box::new(
            noise::ScalePoint::new(
                noise::ScaleBias::<f64, _, 2>::new(noise::Perlin::new(seed))
                    .set_scale(0.5)
                    .set_bias(0.5),
            )
            .set_scale(0.08),
        );

        let wfc = Wfc::new(4, 8, &mut height_maps.iter());

        Self {
            model,
            height_maps,

            seed,
            noise,
            wfc,
            chunks: Default::default(),
            main_chunk: Default::default(),
        }
    }

    pub fn ray_cast(&self, ro: glam::Vec3, rd: glam::Vec3) -> Option<f32> {
        self.chunks.values().fold(None, |prev_hit, (chunk, _)| {
            let hit = chunk.ray_cast(ro, rd, |id| &self.height_maps[&id]);

            match (hit, prev_hit) {
                (Some(hit), Some(prev_hit)) => Some(f32::min(hit, prev_hit)),
                _ => Option::or(hit, prev_hit),
            }
        })
    }

    pub fn get_heat_map(
        &self,
        world_target: glam::Vec3,
    ) -> HeatMap<{ WorldGenerator::CHUNK_SIZE * HeightMap::SIZE }> {
        let (chunk, _) = self.chunks.get(&self.main_chunk).unwrap();

        let height_map_data = chunk.get_height_map_data(|id| self.height_maps.get(&id).unwrap());

        let chunk_target = world_target.xz()
            - glam::vec2(
                self.main_chunk.x as f32 * Self::CHUNK_SIZE as f32 * Self::TILE_WORLD_SIZE,
                self.main_chunk.y as f32 * Self::CHUNK_SIZE as f32 * Self::TILE_WORLD_SIZE,
            );

        let data_target = glam::usizevec2(
            (chunk_target.x / Self::TILE_WORLD_SIZE * HeightMap::SIZE as f32) as usize,
            (chunk_target.y / Self::TILE_WORLD_SIZE * HeightMap::SIZE as f32) as usize,
        );

        HeatMap::new(&height_map_data, data_target)
    }

    fn get_chunk_index(world_pos: glam::Vec3) -> glam::IVec2 {
        glam::ivec2(
            (world_pos.x / Chunk::WORLD_SIZE).floor() as _,
            (world_pos.z / Chunk::WORLD_SIZE).floor() as _,
        )
    }

    fn get_module_index(world_pos: glam::Vec3) -> glam::USizeVec2 {
        let chunk_index = Self::get_chunk_index(world_pos);
        let chunk_pos = glam::vec2(
            chunk_index.x as f32 * Chunk::WORLD_SIZE,
            chunk_index.y as f32 * Chunk::WORLD_SIZE,
        );

        let module_chunk_pos = world_pos.xz() - chunk_pos;

        glam::usizevec2(
            (module_chunk_pos.x / Chunk::WORLD_SIZE * Self::CHUNK_SIZE as f32) as _,
            (module_chunk_pos.y / Chunk::WORLD_SIZE * Self::CHUNK_SIZE as f32) as _,
        )
    }

    pub fn get_height(&self, world_pos: glam::Vec3) -> Option<f32> {
        // self.ray_cast(
        //     glam::vec3(world_pos.x, 1000.0, world_pos.z),
        //     glam::Vec3::NEG_Y,
        // )
        // .map(|h| 1000.0 - h)

        let chunk_index = Self::get_chunk_index(world_pos);
        let (chunk, _) = self.chunks.get(&chunk_index)?;

        let module_index = Self::get_module_index(world_pos);
        let module = chunk.grid[module_index.y][module_index.x];

        let height_map = self.height_maps.get(&module.id)?;

        let height_map_pos = world_pos.xz()
            - glam::vec2(
                chunk_index.x as f32 * Chunk::WORLD_SIZE,
                chunk_index.y as f32 * Chunk::WORLD_SIZE,
            )
            - glam::vec2(
                module_index.x as f32 * Self::TILE_WORLD_SIZE,
                module_index.y as f32 * Self::TILE_WORLD_SIZE,
            );
        let height_map_coord = glam::usizevec2(
            (height_map_pos.x / Self::TILE_WORLD_SIZE * HeightMap::SIZE as f32) as _,
            (height_map_pos.y / Self::TILE_WORLD_SIZE * HeightMap::SIZE as f32) as _,
        );

        let x = height_map_coord.x;
        let y = height_map_coord.y;
        const MAX: usize = HeightMap::SIZE - 1;
        let height = match module.rotation {
            ModuleRotation::Cw0 => height_map.grid[y][x],
            ModuleRotation::Cw90 => height_map.grid[MAX - x][y],
            ModuleRotation::Cw180 => height_map.grid[MAX - y][MAX - x],
            ModuleRotation::Cw270 => height_map.grid[x][MAX - y],
        };

        height.map(|h| h + module.elevation as f32)
    }

    pub fn get_heat_map_coord(&self, world_pos: glam::Vec3) -> Option<glam::USizeVec2> {
        let (main_chunk, _) = self.chunks.get(&self.main_chunk)?;

        let chunk_pos = world_pos.xz() - main_chunk.world_pos.xz();

        let chunk_range = 0.0..Chunk::WORLD_SIZE;
        if !chunk_range.contains(&chunk_pos.x) || !chunk_range.contains(&chunk_pos.y) {
            return None;
        }

        let chunk_pos_norm = chunk_pos / Chunk::WORLD_SIZE;

        Some(glam::usizevec2(
            (chunk_pos_norm.x * (HeightMap::SIZE * Self::CHUNK_SIZE) as f32) as _,
            (chunk_pos_norm.y * (HeightMap::SIZE * Self::CHUNK_SIZE) as f32) as _,
        ))
    }
}

impl Resource for WorldGenerator {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        let height_map_builder = resources.read::<HeightMapBuilder>();

        let mut dungeon_buffer = Vec::new();
        File::open("./demo/assets/dungeon.glb")?.read_to_end(&mut dungeon_buffer)?;
        let (doc, buffers, images) = gltf::import_slice(&dungeon_buffer)?;
        let model = GltfModel::new(resources, doc, &buffers, &images)?;

        const TILES: &[&str] = &[
            "module01", "module03", "module07", "module08", "module09", "module10", "module11",
            "module12", "module13", "module14", "module15", "module16", "module17", "module18",
            "module19",
        ];

        let height_maps = TILES
            .iter()
            .filter_map(|node_name| {
                let node = model.get_node(node_name)?;
                let (floor_triangles, walls_triangles) = get_tile_triangles(&buffers, &node);
                let (height_map, _) = height_map_builder.build(
                    Self::TILE_WORLD_SIZE,
                    &floor_triangles,
                    &walls_triangles,
                );

                Some((node.index(), height_map))
            })
            .collect::<BTreeMap<_, _>>();

        Ok(WorldGenerator::new(SEED, model, height_maps))
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let camera = resources.read::<Camera>();
        let (_, _, camera_pos) = camera.view.inverse().to_scale_rotation_translation();
        let chunk_coord = (camera_pos.xz() / Chunk::WORLD_SIZE).floor();

        // let camera = resources.read::<TopDownCamera>();
        // let chunk_coord = (camera.target.xz() / Chunk::WORLD_SIZE).floor();

        self.main_chunk = glam::ivec2(chunk_coord.x as _, chunk_coord.y as _);

        let chunk_x = (self.main_chunk.x - 1)..=(self.main_chunk.x + 1);
        let chunk_y = (self.main_chunk.y - 1)..=(self.main_chunk.y + 1);
        // let chunk_x = (self.main_chunk.x)..=(self.main_chunk.x);
        // let chunk_y = (self.main_chunk.y)..=(self.main_chunk.y);

        self.chunks
            .retain(|pos, _| chunk_x.contains(&pos.x) && chunk_y.contains(&pos.y));

        for coord in itertools::iproduct!(chunk_x, chunk_y).map(glam::IVec2::from) {
            let Entry::Vacant(entry) = self.chunks.entry(coord) else {
                continue;
            };

            let chunk = Chunk::new(coord, self.seed, &self.noise, &self.wfc);

            let objects = itertools::iproduct!(0..Self::CHUNK_SIZE, 0..Self::CHUNK_SIZE)
                .map(|(y, x)| glam::usizevec2(x, y))
                .map(|coord| {
                    let id = chunk.get_height_map_id(coord);
                    let node = self.model.doc.nodes().nth(id).unwrap();
                    self.model
                        .node_object(node)
                        .with_transform(chunk.get_object_transform(coord))
                })
                .collect::<Vec<_>>();

            entry.insert((chunk, objects));
        }

        Ok(())
    }
}

fn get_tile_triangles(
    buffers: &[gltf::buffer::Data],
    node: &gltf::Node,
) -> (Vec<[glam::Vec3; 3]>, Vec<[glam::Vec3; 3]>) {
    let get_buffer_data = |buffer: gltf::Buffer| -> Option<&[u8]> {
        buffers.get(buffer.index()).map(std::ops::Deref::deref)
    };

    let mut floor_triangles = vec![];
    let mut walls_triangles = vec![];

    calva::gltf::traverse_nodes_tree::<glam::Mat4>(
        node.children(),
        &mut |parent_transform, node| {
            let get_flag = |flag: &str| {
                node.extras()
                    .as_ref()
                    .and_then(|extras| {
                        serde_json::from_str::<serde_json::Map<_, _>>(extras.get())
                            .ok()?
                            .get(flag)
                            .and_then(|value| value.as_bool())
                    })
                    .unwrap_or(false)
            };

            let triangles = match (get_flag("floor"), get_flag("wall")) {
                (true, _) => &mut floor_triangles,
                (_, true) => &mut walls_triangles,
                _ => return None,
            };

            let transform =
                *parent_transform * glam::Mat4::from_cols_array_2d(&node.transform().matrix());

            if let Some(mesh) = node.mesh() {
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(get_buffer_data);

                    let vertices: Vec<_> = reader
                        .read_positions()?
                        .map(glam::Vec3::from_array)
                        .collect();

                    let indices: Vec<_> = reader.read_indices()?.into_u32().collect();

                    triangles.extend(indices.chunks_exact(3).filter_map(|chunk| {
                        let [i1, i2, i3] = <[u32; 3]>::try_from(chunk).ok()?;

                        Some([
                            transform.transform_point3(*vertices.get(i1 as usize)?),
                            transform.transform_point3(*vertices.get(i2 as usize)?),
                            transform.transform_point3(*vertices.get(i3 as usize)?),
                        ])
                    }));
                }
            }

            Some(transform)
        },
        glam::Mat4::IDENTITY,
    );

    (floor_triangles, walls_triangles)
}
