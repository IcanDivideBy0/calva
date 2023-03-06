use rand::prelude::*;
use rand_seeder::{SipHasher, SipRng};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, hash::Hash};

use calva::{
    gltf::GltfModel,
    renderer::{Instance, PointLight},
};

pub struct WorldGenerator {
    modules: Vec<Module>,
}

impl WorldGenerator {
    pub fn new(model: &GltfModel) -> Self {
        let modules = model
            .doc
            .nodes()
            .filter_map(|node| {
                let name = node.name()?.to_string();
                let extras = node.extras().as_ref()?;

                #[derive(Debug, Serialize, Deserialize)]
                struct WfcInfo {
                    #[serde(rename = "wfc_north")]
                    north: [u8; Module::SIZE],
                    #[serde(rename = "wfc_east")]
                    east: [u8; Module::SIZE],
                    #[serde(rename = "wfc_south")]
                    south: [u8; Module::SIZE],
                    #[serde(rename = "wfc_west")]
                    west: [u8; Module::SIZE],
                }
                let infos = serde_json::from_str::<WfcInfo>(extras.get()).ok()?;

                Some(Module {
                    name,
                    north: infos.north,
                    east: infos.east,
                    south: infos.south,
                    west: infos.west,
                })
            })
            .collect::<Vec<_>>();

        Self { modules }
    }

    pub fn chunk(&self, seed: impl Hash, coord: glam::IVec2) -> Chunk {
        Chunk::new(seed, coord, &self.modules)
    }
}

pub struct Chunk {
    rng: SipRng,
    offset: glam::IVec2,
    pub grid: [[Cell; Self::SIZE]; Self::SIZE],
}

impl Chunk {
    pub const SIZE: usize = 16;

    pub fn new(seed: impl Hash, coord: glam::IVec2, modules: &[Module]) -> Self {
        let rng = SipHasher::from((seed, coord)).into_rng();

        let offset = coord * (Self::SIZE as i32);

        let grid = std::array::from_fn(|_y| {
            std::array::from_fn(|_x| Cell {
                options: modules
                    .iter()
                    .flat_map(|module| {
                        Orientation::all()
                            .into_iter()
                            .map(|orientation| ModuleOption {
                                module: module.clone(),
                                orientation,
                            })
                    })
                    .collect(),
            })
        });

        Self { rng, offset, grid }
    }

    fn propagate(&mut self, x: usize, y: usize, cb: &mut dyn FnMut(&Cell, glam::UVec2)) {
        for face in Face::all() {
            let (xx, yy) = match face {
                Face::North if y > 0 => (x, y - 1),
                Face::East if x < Self::SIZE - 1 => (x + 1, y),
                Face::South if y < Self::SIZE - 1 => (x, y + 1),
                Face::West if x > 0 => (x - 1, y),
                _ => continue,
            };

            let allowed_options = self.grid[y][x].filter_allowed_options(face, &self.grid[yy][xx]);

            let neighbour = &mut self.grid[yy][xx];

            if neighbour.collapsed() {
                continue;
            }

            let prev_entropy = neighbour.entropy();
            neighbour.options = allowed_options;

            if prev_entropy > neighbour.entropy() {
                if neighbour.collapsed() {
                    cb(neighbour, glam::uvec2(xx as _, yy as _));
                }

                self.propagate(xx, yy, cb);
            }
        }
    }

    pub fn solve(&mut self, model: &GltfModel) -> (Vec<Instance>, Vec<PointLight>) {
        let mut min_entropy: Option<(usize, (usize, usize))> = None;

        for y in 0..Self::SIZE {
            for x in 0..Self::SIZE {
                let cell = &self.grid[y][x];

                if cell.collapsed() {
                    continue;
                }

                match min_entropy {
                    None => {
                        min_entropy = Some((cell.entropy(), (x, y)));
                    }
                    Some((min, ..)) if min > cell.entropy() => {
                        min_entropy = Some((cell.entropy(), (x, y)));
                    }
                    _ => {}
                }
            }
        }

        if let Some((_, (x, y))) = min_entropy {
            let world_offset = glam::vec3(self.offset.x as _, 0.0, self.offset.y as _);

            let mut instances = (vec![], vec![]);
            let mut instanciate = |cell: &Cell, pos: glam::UVec2| {
                let opt = cell.options.first().unwrap();

                let pos = pos * Module::WORLD_SIZE as u32;
                let translation = glam::vec3(pos.x as _, 0.0, pos.y as _);

                let res = model.node_instances(
                    &opt.module.name,
                    Some(glam::Mat4::from_rotation_translation(
                        opt.orientation.into(),
                        world_offset + translation,
                    )),
                    None,
                );

                if let Some(res) = res {
                    instances.0.extend(res.0);
                    instances.1.extend(res.1);
                }
            };

            let cell = &mut self.grid[y][x];

            let rand_index = self.rng.gen::<usize>() % cell.options.len();
            cell.options = cell
                .options
                .iter()
                .skip(rand_index)
                .take(1)
                .cloned()
                .collect();
            instanciate(cell, glam::uvec2(x as _, y as _));

            self.propagate(x, y, &mut instanciate);

            instances
        } else {
            println!("already solved");
            (vec![], vec![])
        }
    }

    #[allow(dead_code)]
    pub fn collapsed(&self) -> bool {
        self.grid.iter().flatten().all(Cell::collapsed)
    }
}

#[derive(Debug)]
pub struct Cell {
    options: BTreeSet<ModuleOption>,
}

impl Cell {
    pub fn entropy(&self) -> usize {
        self.options.len()
    }

    pub fn collapsed(&self) -> bool {
        self.entropy() == 1
    }

    pub fn filter_allowed_options(&self, face: Face, neighbour: &Cell) -> BTreeSet<ModuleOption> {
        self.options
            .iter()
            .flat_map(|opt| {
                let mut floor_levels = opt.floor_levels(face);
                floor_levels.reverse();

                neighbour
                    .options
                    .iter()
                    .cloned()
                    .filter(move |neighbour_opt| {
                        neighbour_opt.floor_levels(face.opposite()) == floor_levels
                    })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleOption {
    module: Module,
    orientation: Orientation,
}

impl ModuleOption {
    pub fn floor_levels(&self, face: Face) -> [u8; Module::SIZE] {
        self.module.floor_levels(face.reorient(self.orientation))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Module {
    name: String,
    north: [u8; Self::SIZE],
    east: [u8; Self::SIZE],
    south: [u8; Self::SIZE],
    west: [u8; Self::SIZE],
}

impl Module {
    pub const SIZE: usize = 5;
    pub const TILE_SIZE: usize = 6;
    pub const WORLD_SIZE: usize = Self::SIZE * Self::TILE_SIZE;

    pub fn floor_levels(&self, face: Face) -> [u8; Self::SIZE] {
        match face {
            Face::North => self.north,
            Face::East => self.east,
            Face::South => self.south,
            Face::West => self.west,
        }
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
        self.rotate().rotate()
    }

    const fn rotate(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }

    const fn reorient(self, orientation: Orientation) -> Face {
        match orientation {
            Orientation::Top => self,
            Orientation::Right => self.rotate(),
            Orientation::Bottom => self.rotate().rotate(),
            Orientation::Left => self.rotate().rotate().rotate(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Orientation {
    Top,
    Right,
    Bottom,
    Left,
}

impl Orientation {
    const fn all() -> impl IntoIterator<Item = Self> {
        [Self::Top, Self::Right, Self::Bottom, Self::Left]
    }
}

impl From<Orientation> for glam::Quat {
    fn from(orientation: Orientation) -> Self {
        glam::Quat::from_rotation_y(match orientation {
            Orientation::Top => 0.0,
            Orientation::Right => std::f32::consts::FRAC_PI_2,
            Orientation::Bottom => std::f32::consts::PI,
            Orientation::Left => -std::f32::consts::FRAC_PI_2,
        })
    }
}
