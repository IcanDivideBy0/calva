use rand::prelude::*;
use rand_seeder::{SipHasher, SipRng};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashSet},
    hash::Hash,
};

use calva::{
    gltf::GltfModel,
    renderer::{Instance, PointLight},
};

pub struct WorldGenerator {
    options: BTreeSet<ModuleOption>,
}

impl WorldGenerator {
    pub fn new(model: &GltfModel) -> Self {
        let options = model
            .doc
            .nodes()
            .filter_map(|node| {
                let id = node.index();
                let extras = node.extras().as_ref()?;

                #[derive(Debug, Serialize, Deserialize)]
                struct WfcInfo {
                    #[serde(rename = "wfc_north")]
                    north: [u8; ModuleOption::TILE_COUNT],
                    #[serde(rename = "wfc_east")]
                    east: [u8; ModuleOption::TILE_COUNT],
                    #[serde(rename = "wfc_south")]
                    south: [u8; ModuleOption::TILE_COUNT],
                    #[serde(rename = "wfc_west")]
                    west: [u8; ModuleOption::TILE_COUNT],
                }
                let infos = serde_json::from_str::<WfcInfo>(extras.get()).ok()?;

                Some(ModuleOption::permutations(
                    id,
                    infos.north,
                    infos.east,
                    infos.south,
                    infos.west,
                ))
            })
            .flatten()
            .collect();

        Self { options }
    }

    pub fn chunk(&self, seed: impl Hash, coord: glam::IVec2) -> Chunk {
        Chunk::new(seed, coord, &self.options)
    }
}

pub struct Chunk {
    rng: SipRng,
    offset: glam::IVec2,
    pub grid: [[RefCell<Slot>; Self::SIZE]; Self::SIZE],
}

impl Chunk {
    pub const SIZE: usize = 4;

    pub fn new(seed: impl Hash, coord: glam::IVec2, options: &BTreeSet<ModuleOption>) -> Self {
        let rng = SipHasher::from((seed, coord)).into_rng();

        let offset = coord * (Self::SIZE as i32);

        let grid = std::array::from_fn(|_y| {
            std::array::from_fn(|_x| {
                RefCell::new(Slot {
                    options: options.clone(),
                })
            })
        });

        Self { rng, offset, grid }
    }

    fn propagate(&mut self, x: usize, y: usize, cb: &mut dyn FnMut(&Slot, glam::IVec2)) {
        for face in Face::all() {
            let (xx, yy) = match face {
                Face::North if y > 0 => (x, y - 1),
                Face::East if x < Self::SIZE - 1 => (x + 1, y),
                Face::South if y < Self::SIZE - 1 => (x, y + 1),
                Face::West if x > 0 => (x - 1, y),
                _ => continue,
            };

            let slot = self.grid[y][x].borrow();
            let mut neighbour = self.grid[yy][xx].borrow_mut();

            if slot.filter_allowed_options(face, &mut *neighbour) {
                if neighbour.collapsed() {
                    cb(&*neighbour, glam::ivec2(xx as _, yy as _));
                }

                drop(slot);
                drop(neighbour);

                self.propagate(xx, yy, cb);
            }
        }
    }

    pub fn solve(&mut self, model: &GltfModel) -> (Vec<Instance>, Vec<PointLight>) {
        let mut min_entropy: Option<(usize, (usize, usize))> = None;

        for y in 0..Self::SIZE {
            for x in 0..Self::SIZE {
                let slot = self.grid[y][x].borrow();

                if slot.collapsed() {
                    continue;
                }

                match min_entropy {
                    None => {
                        min_entropy = Some((slot.entropy(), (x, y)));
                    }
                    Some((min, ..)) if min > slot.entropy() => {
                        min_entropy = Some((slot.entropy(), (x, y)));
                    }
                    _ => {}
                }
            }
        }

        if let Some((_, (x, y))) = min_entropy {
            let world_offset = self.offset;

            let mut instances = (vec![], vec![]);
            let mut instanciate = |slot: &Slot, pos: glam::IVec2| {
                let opt = slot.options.first().unwrap();

                let node_name = model.doc.nodes().nth(opt.id).unwrap().name().unwrap();

                let res =
                    model.node_instances(node_name, Some(opt.transform(world_offset + pos)), None);

                if let Some(res) = res {
                    instances.0.extend(res.0);
                    instances.1.extend(res.1);
                }
            };

            let mut slot = self.grid[y][x].borrow_mut();
            slot.options = [*slot.options.iter().choose(&mut self.rng).unwrap()].into();

            instanciate(&*slot, glam::ivec2(x as _, y as _));

            drop(slot);

            self.propagate(x, y, &mut instanciate);

            instances
        } else {
            println!("already solved");
            (vec![], vec![])
        }
    }

    #[allow(dead_code)]
    pub fn collapsed(&self) -> bool {
        self.grid
            .iter()
            .flatten()
            .all(|slot| slot.borrow().collapsed())
    }
}

#[derive(Debug)]
pub struct Slot {
    options: BTreeSet<ModuleOption>,
}

impl Slot {
    pub fn entropy(&self) -> usize {
        self.options.len()
    }

    pub fn collapsed(&self) -> bool {
        self.entropy() == 1
    }

    pub fn filter_allowed_options(&self, face: Face, neighbour: &mut Self) -> bool {
        if neighbour.collapsed() {
            return false;
        }

        let floor_levels = self
            .options
            .iter()
            .map(|opt| {
                let mut floor_levels = opt.floor_levels(face);
                floor_levels.reverse();
                floor_levels
            })
            .collect::<HashSet<_>>();

        let prev_entropy = neighbour.entropy();

        neighbour.options.retain(|neighbour_opt| {
            floor_levels.contains(&neighbour_opt.floor_levels(face.opposite()))
        });

        prev_entropy > neighbour.entropy()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleOption {
    id: usize,
    rotation: u8,
    elevation: u8,

    north: [u8; Self::TILE_COUNT],
    east: [u8; Self::TILE_COUNT],
    south: [u8; Self::TILE_COUNT],
    west: [u8; Self::TILE_COUNT],
}

impl ModuleOption {
    pub const TILE_COUNT: usize = 5;
    pub const TILE_SIZE: usize = 6;
    pub const ELEVATION_MAX: usize = 8;

    pub const WORLD_FLOOR_HEIGHT: usize = 4;
    pub const WORLD_SIZE: usize = Self::TILE_COUNT * Self::TILE_SIZE;

    pub fn floor_levels(&self, face: Face) -> [u8; Self::TILE_COUNT] {
        match face {
            Face::North => self.north,
            Face::East => self.east,
            Face::South => self.south,
            Face::West => self.west,
        }
        .map(|level| level + self.elevation)
    }

    pub fn permutations(
        id: usize,
        mut north: [u8; Self::TILE_COUNT],
        mut east: [u8; Self::TILE_COUNT],
        mut south: [u8; Self::TILE_COUNT],
        mut west: [u8; Self::TILE_COUNT],
    ) -> impl Iterator<Item = Self> {
        (0..4)
            .map(move |rotation| {
                let it = (0..=Self::ELEVATION_MAX as u8).map(move |elevation| Self {
                    id,
                    rotation,
                    elevation,
                    north,
                    east,
                    south,
                    west,
                });

                // Rotate faces
                (west, north, east, south) = (north, east, south, west);

                it
            })
            .flatten()
    }

    // pub fn permutations(&self) -> impl Iterator<Item = Self> {
    //     let ModuleOption {
    //         id,
    //         rotation,
    //         mut north,
    //         mut east,
    //         mut south,
    //         mut west,
    //         ..
    //     } = *self;

    //     (rotation..rotation + 4)
    //         .map(move |rotation| {
    //             let it = (0..=Self::ELEVATION_MAX as u8).map(move |elevation| Self {
    //                 id,
    //                 rotation,
    //                 elevation,
    //                 north,
    //                 east,
    //                 south,
    //                 west,
    //             });

    //             // Rotate faces
    //             (west, north, east, south) = (north, east, south, west);

    //             it
    //         })
    //         .flatten()
    // }

    pub fn transform(&self, pos: glam::IVec2) -> glam::Mat4 {
        let quat = glam::Quat::from_rotation_y(self.rotation as f32 * std::f32::consts::FRAC_PI_2);

        dbg!(self.elevation);

        let translation = glam::vec3(
            pos.x as f32 * Self::WORLD_SIZE as f32,
            self.elevation as f32 * Self::WORLD_FLOOR_HEIGHT as f32,
            pos.y as f32 * Self::WORLD_SIZE as f32,
        );

        glam::Mat4::from_rotation_translation(quat, translation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    North,
    East,
    South,
    West,
}

impl Face {
    const fn all() -> impl IntoIterator<Item = Self> {
        [Self::North, Self::East, Self::South, Self::West]
    }

    const fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}
