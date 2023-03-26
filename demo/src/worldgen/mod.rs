use noise::NoiseFn;
use rand::prelude::*;
use rand_seeder::SipHasher;
use std::{cell::RefCell, collections::BTreeSet, hash::Hash};

use calva::{
    gltf::GltfModel,
    renderer::{Instance, PointLight},
};

pub mod tile;
use tile::{Face, Tile};

pub struct WorldGenerator {
    seed: u32,
    noise: Box<dyn NoiseFn<f64, 2>>,
    options: BTreeSet<SlotOption>,

    pub model: GltfModel,
}

impl WorldGenerator {
    pub fn new(seed: impl Hash, model: GltfModel, tiles: &[Tile]) -> Self {
        let seed = SipHasher::from(seed).into_rng().gen();

        let noise = Box::new(
            noise::ScalePoint::new(
                noise::ScaleBias::<f64, _, 2>::new(noise::Perlin::new(seed))
                    .set_scale(0.5)
                    .set_bias(0.5),
            )
            .set_scale(0.08),
        );

        let options = tiles
            .iter()
            .flat_map(|tile| SlotOption::permutations(tile.node_id, tile.wfc_constraints()))
            .collect();

        Self {
            seed,
            noise,
            model,
            options,
        }
    }

    pub fn chunk(&self, coord: glam::IVec2) -> (Vec<Instance>, Vec<PointLight>) {
        let chunk = Chunk::new(&self.seed, coord, self.noise.as_ref(), &self.options);

        let mut instances = vec![];
        let mut point_lights = vec![];

        let offset = coord * (Chunk::SIZE as i32);

        for y in 0..Chunk::SIZE {
            for x in 0..Chunk::SIZE {
                let slot = chunk.grid[y][x].borrow();

                slot.options.first().and_then(|opt| {
                    let node_name = self.model.doc.nodes().nth(opt.id)?.name()?;

                    let res = self.model.node_instances(
                        node_name,
                        Some(opt.transform(offset + glam::ivec2(x as _, y as _))),
                        None,
                    )?;

                    instances.extend(res.0);
                    point_lights.extend(res.1);

                    Some(())
                });
            }
        }

        (instances, point_lights)
    }
}

type ChunkGrid = [[RefCell<Slot>; Chunk::SIZE]; Chunk::SIZE];
struct Chunk {
    grid: ChunkGrid,
}

impl Chunk {
    pub const SIZE: usize = 3;

    pub fn new(
        seed: impl Hash,
        coord: glam::IVec2,
        noise: &dyn NoiseFn<f64, 2>,
        options: &BTreeSet<SlotOption>,
    ) -> Self {
        let mut rng = SipHasher::from((seed, coord)).into_rng();

        let grid = std::array::from_fn(|_| {
            std::array::from_fn(|_| {
                RefCell::new(Slot {
                    options: options.clone(),
                })
            })
        });

        for face in Face::all() {
            for i in 0..Self::SIZE {
                let (x, y) = match face {
                    Face::North => (i, 0),
                    Face::East => (Self::SIZE - 1, i),
                    Face::South => (Self::SIZE - 1 - i, Self::SIZE - 1),
                    Face::West => (0, Self::SIZE - 1 - i),
                };

                let nx = coord.x as f64 * Self::SIZE as f64
                    + x as f64
                    + match face {
                        Face::East | Face::South => 1.0,
                        _ => 0.0,
                    };

                let ny = coord.y as f64 * Self::SIZE as f64
                    + y as f64
                    + match face {
                        Face::South | Face::West => 1.0,
                        _ => 0.0,
                    };

                let elevation_start = noise.get([nx, ny]) * SlotOption::ELEVATION_MAX as f64;

                let nxx = match face {
                    Face::North => nx + 1.0,
                    Face::South => nx - 1.0,
                    _ => nx,
                };
                let nyy = match face {
                    Face::East => ny + 1.0,
                    Face::West => ny - 1.0,
                    _ => ny,
                };

                let elevation_end = noise.get([nxx, nyy]) * SlotOption::ELEVATION_MAX as f64;

                let elevation_start = elevation_start as u8 * 2;
                let elevation_end = elevation_end as u8 * 2;

                let mut constraint = [
                    Some(elevation_start),
                    Some(elevation_start),
                    Some(elevation_start.max(elevation_end)),
                    Some(elevation_end),
                    Some(elevation_end),
                ];
                constraint.reverse();

                grid[y][x]
                    .borrow_mut()
                    .apply_constraints(face, &[constraint]);

                Self::propagate(&grid, x, y);
            }
        }

        while let Some((x, y)) = Self::min_entropy_slot(&grid) {
            let mut slot = grid[y][x].borrow_mut();
            slot.options = [*slot.options.iter().choose(&mut rng).unwrap()].into();
            drop(slot);

            Self::propagate(&grid, x, y);
        }

        Self { grid }
    }

    fn min_entropy_slot(grid: &ChunkGrid) -> Option<(usize, usize)> {
        (0..Self::SIZE)
            .flat_map(|y| {
                (0..Self::SIZE).filter_map(move |x| {
                    let slot = grid[y][x].borrow();

                    if slot.collapsed() {
                        None
                    } else {
                        Some((slot.entropy(), (x, y)))
                    }
                })
            })
            .min_by_key(|(entropy, _)| *entropy)
            .map(|(_, coord)| coord)
    }

    fn propagate(grid: &ChunkGrid, x: usize, y: usize) {
        for face in Face::all() {
            let (xx, yy) = match face {
                Face::North if y > 0 => (x, y - 1),
                Face::East if x < Self::SIZE - 1 => (x + 1, y),
                Face::South if y < Self::SIZE - 1 => (x, y + 1),
                Face::West if x > 0 => (x - 1, y),
                _ => continue,
            };

            let slot = grid[y][x].borrow();
            let mut neighbour = grid[yy][xx].borrow_mut();

            let constraints = slot.constraints(face).collect::<Vec<_>>();
            let has_changed = neighbour.apply_constraints(face.opposite(), &constraints);

            if has_changed {
                drop(slot);
                drop(neighbour);

                Self::propagate(grid, xx, yy);
            }
        }
    }
}

#[derive(Debug)]
struct Slot {
    options: BTreeSet<SlotOption>,
}

impl Slot {
    pub fn entropy(&self) -> usize {
        self.options.len()
    }

    pub fn collapsed(&self) -> bool {
        self.entropy() == 1
    }

    pub fn constraints(&self, face: Face) -> impl Iterator<Item = ModuleConstraint> + '_ {
        self.options.iter().map(move |opt| opt.constraint(face))
    }

    pub fn apply_constraints(&mut self, face: Face, constraints: &[ModuleConstraint]) -> bool {
        if self.collapsed() {
            return false;
        }

        let prev_entropy = self.entropy();

        self.options.retain(|opt| {
            let mut c = opt.constraint(face);
            c.reverse();

            constraints.contains(&c)
        });

        prev_entropy > self.entropy()
    }
}

type ModuleConstraint = [Option<u8>; Tile::WFC_SAMPLES];

#[derive(Debug, Clone, Copy)]
pub struct SlotOption {
    id: usize,
    elevation: u8,
    rotation: u8,
    faces: [ModuleConstraint; 4], // north east south west
}

impl Eq for SlotOption {}
impl PartialEq for SlotOption {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.rotation == other.rotation && self.elevation == other.elevation
    }
}

impl Hash for SlotOption {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.elevation.hash(state);
        self.rotation.hash(state);
    }
}

impl std::cmp::PartialOrd for SlotOption {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for SlotOption {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id
            .cmp(&other.id)
            .then(self.elevation.cmp(&other.elevation))
            .then(self.rotation.cmp(&other.rotation))
    }
}

impl SlotOption {
    pub const ELEVATION_MAX: usize = 4;

    pub fn constraint(&self, face: Face) -> ModuleConstraint {
        match face {
            Face::North => self.faces[0],
            Face::East => self.faces[1],
            Face::South => self.faces[2],
            Face::West => self.faces[3],
        }
    }

    pub fn permutations(id: usize, mut faces: [ModuleConstraint; 4]) -> impl Iterator<Item = Self> {
        (0..4)
            .map(move |rotation| {
                let it = (0..=Self::ELEVATION_MAX as u8).map(move |elevation| Self {
                    id,
                    elevation,
                    rotation,
                    faces: faces.map(|face| face.map(|value| value.map(|i| i + elevation))),
                });

                // Rotate faces
                faces = [faces[3], faces[0], faces[1], faces[2]];

                it
            })
            .flatten()
    }

    pub fn transform(&self, pos: glam::IVec2) -> glam::Mat4 {
        let quat = glam::Quat::from_rotation_y(self.rotation as f32 * -std::f32::consts::FRAC_PI_2);

        let translation =
            glam::vec3(pos.x as f32, self.elevation as f32, pos.y as f32) * Tile::WORLD_POS;

        glam::Mat4::from_rotation_translation(quat, translation)
    }
}
