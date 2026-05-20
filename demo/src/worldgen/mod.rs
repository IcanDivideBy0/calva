use anyhow::Result;
use calva::{
    gltf::GltfModel,
    nav::{HeatMap, HeightMap, HeightMapBuilder},
    renderer::{Object, Resource, ResourcesManager},
};
use glam::Vec3Swizzles;
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

use crate::controls::TopDownCamera;

const SEED: &str = "Calva!533d";

type Chunk = WorldChunk<{ WorldGenerator::CHUNK_SIZE }, { WorldGenerator::WFC_MODULE_SIZE }>;

pub struct WorldGenerator {
    model: GltfModel,
    height_maps: BTreeMap<usize, HeightMap>, // Needs to be a sorted structure for wfc to produce stable results w/ rng

    wfc: Wfc<{ Self::CHUNK_SIZE }, { Self::WFC_MODULE_SIZE }>,
    chunks: HashMap<glam::IVec2, (Chunk, Vec<Object>)>,
    main_chunk: glam::IVec2,
}

impl WorldGenerator {
    pub const TILE_WORLD_SIZE: f32 = 5.0 * 6.0;
    pub const CHUNK_SIZE: usize = 3;
    pub const WFC_MODULE_SIZE: usize = 5;

    fn new(seed: impl Hash, model: GltfModel, height_maps: BTreeMap<usize, HeightMap>) -> Self {
        let wfc = Wfc::new(seed, 4, 8, &mut height_maps.iter());

        Self {
            model,
            height_maps,

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

    fn get_chunk(&self, world_pos: glam::Vec2) -> Option<&Chunk> {
        let chunk_index = glam::ivec2(
            (world_pos.x / Chunk::WORLD_SIZE).floor() as _,
            (world_pos.y / Chunk::WORLD_SIZE).floor() as _,
        );

        self.chunks.get(&chunk_index).map(|(chunk, _)| chunk)
    }

    pub fn get_height(&self, world_pos: glam::Vec2) -> Option<f32> {
        let chunk = self.get_chunk(world_pos)?;

        let module_index = chunk.module_index(world_pos);
        let module = chunk.grid[module_index.y][module_index.x];

        let height_map = self.height_maps.get(&module.id)?;

        let module_pos = world_pos
            - chunk.world_pos
            - glam::vec2(
                module_index.x as f32 * Self::TILE_WORLD_SIZE,
                module_index.y as f32 * Self::TILE_WORLD_SIZE,
            );
        let module_coord = glam::usizevec2(
            (module_pos.x / Self::TILE_WORLD_SIZE * HeightMap::SIZE as f32) as _,
            (module_pos.y / Self::TILE_WORLD_SIZE * HeightMap::SIZE as f32) as _,
        );

        module.get_height(module_coord, height_map)
    }

    pub fn get_heat_map(
        &self,
        world_target: glam::Vec2,
    ) -> Option<HeatMap<{ 3 * HeightMap::SIZE }>> {
        let mut height_map_data = std::array::from_fn(|_| std::array::from_fn(|_| None));

        for (grid_y, grid_x) in itertools::iproduct!(0..3, 0..3) {
            let w_pos = world_target
                + glam::vec2(grid_x as f32 - 1.0, grid_y as f32 - 1.0) * Self::TILE_WORLD_SIZE;

            let chunk = self.get_chunk(w_pos)?;

            let (mx, my) = chunk.module_index(w_pos).into();
            let module = chunk.grid[my][mx];

            let height_map = self.height_maps.get(&module.id)?;

            for (y, x) in itertools::iproduct!(0..HeightMap::SIZE, 0..HeightMap::SIZE) {
                height_map_data[grid_y * HeightMap::SIZE + y][grid_x * HeightMap::SIZE + x] =
                    module.get_height(glam::usizevec2(x, y), height_map);
            }
        }

        let t =
            (world_target % Self::TILE_WORLD_SIZE + Self::TILE_WORLD_SIZE) % Self::TILE_WORLD_SIZE;
        let t = (t / Self::TILE_WORLD_SIZE + 1.0) * HeightMap::SIZE as f32;

        let height_map_target = glam::usizevec2(t.x as _, t.y as _);

        Some(HeatMap::new(&height_map_data, height_map_target))
    }

    pub fn get_heat_map_coord(
        &self,
        world_pos: glam::Vec2,
        world_target: glam::Vec2,
    ) -> glam::USizeVec2 {
        const HEAT_MAP_SIZE: usize = 3 * HeightMap::SIZE;

        let hm_offset =
            (world_target % Self::TILE_WORLD_SIZE + Self::TILE_WORLD_SIZE) % Self::TILE_WORLD_SIZE;
        let world_hm_center = world_target - hm_offset + Self::TILE_WORLD_SIZE / 2.0;

        let heat_map_pos_norm = (world_pos - world_hm_center) / (Self::TILE_WORLD_SIZE * 3.0) + 0.5;

        let heat_map_coord = heat_map_pos_norm * HEAT_MAP_SIZE as f32;
        let heat_map_coord = heat_map_coord.clamp(
            glam::Vec2::splat(0.0),
            glam::Vec2::splat((HEAT_MAP_SIZE - 1) as f32),
        );
        glam::usizevec2(heat_map_coord.x as _, heat_map_coord.y as _)
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
        let camera = resources.read::<TopDownCamera>();
        let chunk_coord = (camera.target.xz() / Chunk::WORLD_SIZE).floor();

        self.main_chunk = glam::ivec2(chunk_coord.x as _, chunk_coord.y as _);

        let chunk_x = (self.main_chunk.x - 1)..=(self.main_chunk.x + 1);
        let chunk_y = (self.main_chunk.y - 1)..=(self.main_chunk.y + 1);

        self.chunks
            .retain(|coord, _| chunk_x.contains(&coord.x) && chunk_y.contains(&coord.y));

        for coord in itertools::iproduct!(chunk_x, chunk_y).map(glam::IVec2::from) {
            let Entry::Vacant(entry) = self.chunks.entry(coord) else {
                continue;
            };

            let chunk = Chunk::new(coord, &self.wfc);

            let objects = itertools::iproduct!(0..Self::CHUNK_SIZE, 0..Self::CHUNK_SIZE)
                .map(|(y, x)| glam::usizevec2(x, y))
                .map(|coord| {
                    let id = chunk.grid[coord.y][coord.x].id;
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
