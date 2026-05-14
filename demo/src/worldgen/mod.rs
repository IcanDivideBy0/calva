use anyhow::Result;
use calva::{
    gltf::GltfModel,
    nav::HeightMapBuilder,
    renderer::{Camera, Object, Resource, ResourcesManager},
};
use rand::RngExt;
use rand_seeder::SipHasher;
use std::{
    collections::{hash_map::Entry, HashMap},
    fs::File,
    hash::Hash,
    io::Read,
};

mod chunk;
mod tile;
mod wfc;

use chunk::WorldChunk;
use tile::Tile;

const CHUNK_SIZE: usize = 3;
const MODULE_SIZE: usize = 5;
type Chunk = WorldChunk<CHUNK_SIZE, MODULE_SIZE>;

pub struct WorldGenerator {
    model: GltfModel,
    tiles: HashMap<usize, Tile>,

    seed: u32,
    // noise: Box<dyn NoiseFn<f64, 2> + Send + Sync>,
    noise: Box<noise::ScalePoint<noise::ScaleBias<f64, noise::Perlin, 2>>>,
    chunks: HashMap<glam::IVec2, (Chunk, Vec<Object>)>,
}

impl WorldGenerator {
    fn new(seed: impl Hash, model: GltfModel, tiles: HashMap<usize, Tile>) -> Self {
        let seed = SipHasher::from(seed).into_rng().random();

        let noise = Box::new(
            noise::ScalePoint::new(
                noise::ScaleBias::<f64, _, 2>::new(noise::Perlin::new(seed))
                    .set_scale(0.5)
                    .set_bias(0.5),
            )
            .set_scale(0.08),
        );

        Self {
            model,
            tiles,

            seed,
            noise,
            chunks: Default::default(),
        }
    }

    pub fn ray_cast(&self, ro: glam::Vec3, rd: glam::Vec3) -> Option<f32> {
        self.chunks.values().fold(None, |prev_hit, (chunk, _)| {
            let hit = chunk.ray_cast(ro, rd, |tile_id| &self.tiles[&tile_id]);

            match (hit, prev_hit) {
                (Some(hit), Some(prev_hit)) => Some(f32::min(hit, prev_hit)),
                _ => Option::or(hit, prev_hit),
            }
        })
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

        let tiles = TILES
            .iter()
            .filter_map(|node_name| {
                let node = model.get_node(node_name)?;
                let (floor_triangles, walls_triangles) = get_tile_triangles(&buffers, &node);
                let (height_map, depth) =
                    height_map_builder.build(Tile::WORLD_SIZE, &floor_triangles, &walls_triangles);

                Some((node.index(), Tile { depth, height_map }))
            })
            .collect::<HashMap<_, _>>();

        Ok(WorldGenerator::new("Calva!533d", model, tiles))
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let camera = resources.read::<Camera>();

        let (_, _, cam_pos) = camera.view.inverse().to_scale_rotation_translation();

        let chunk_coord = ((cam_pos + Tile::WORLD_SIZE * 0.5) / Chunk::WORLD_SIZE).floor();
        let chunk_coord = glam::ivec2(chunk_coord.x as _, chunk_coord.z as _);
        let chunk_x = (chunk_coord.x - 1)..=(chunk_coord.x + 1);
        let chunk_y = (chunk_coord.y - 1)..=(chunk_coord.y + 1);

        self.chunks
            .retain(|pos, _| chunk_x.contains(&pos.x) && chunk_y.contains(&pos.y));

        for key in itertools::iproduct!(chunk_x, chunk_y).map(|(x, y)| glam::ivec2(x, y)) {
            if let Entry::Vacant(entry) = self.chunks.entry(key) {
                let chunk = Chunk::new(&mut self.tiles.iter(), self.seed, &self.noise, key);

                let objects = itertools::iproduct!(0..CHUNK_SIZE, 0..CHUNK_SIZE)
                    .map(|(y, x)| glam::usizevec2(x, y))
                    .map(|coord| {
                        let tile_id = chunk.get_tile_id(coord);

                        let node = self.model.doc.nodes().nth(tile_id).unwrap();
                        self.model
                            .node_object(node)
                            .with_transform(chunk.get_tile_transform(coord))
                    })
                    .collect::<Vec<_>>();

                entry.insert((chunk, objects));
            }
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
