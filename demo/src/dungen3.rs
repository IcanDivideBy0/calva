use std::collections::HashSet;

use anyhow::Result;

use calva::{
    gltf::GltfModel,
    renderer::{Engine, Instance, Renderer},
};
use noise::NoiseFn;

pub struct Chunk {
    model: GltfModel,
    noise: noise::Value,
    cells: [[[Cell; Self::SIZE_Z]; Self::SIZE_Y]; Self::SIZE_X],
}

impl Chunk {
    pub const SIZE_X: usize = 8;
    pub const SIZE_Y: usize = 3;
    pub const SIZE_Z: usize = 8;

    pub fn new(renderer: &Renderer, engine: &mut Engine, seed: Option<u32>) -> Result<Self> {
        let model = GltfModel::from_path(renderer, engine, "./demo/assets/dungeon.glb")?;

        let seed = seed.unwrap_or_else(rand::random);
        dbg!(seed);

        Ok(Self {
            model,
            noise: noise::Value::new(seed),
            cells: Default::default(),
        })
    }

    fn propagate(&mut self, x: usize, y: usize, z: usize, cb: &mut dyn FnMut(&Cell, glam::UVec3)) {
        for face in Face::all() {
            let mut allowed = self.cells[x][y][z].allowed(face);

            let (xx, yy, zz) = match face {
                Face::Up if z > 0 => (x, y, z - 1),
                Face::Down if z < Self::SIZE_Z - 1 => (x, y, z + 1),
                Face::Right if x < Self::SIZE_X - 1 => (x + 1, y, z),
                Face::Left if x > 0 => (x - 1, y, z),
                Face::Top if y < Self::SIZE_Y - 1 => (x, y + 1, z),
                Face::Bottom if y > 0 => (x, y - 1, z),
                _ => continue,
            };

            let neighbour = &mut self.cells[xx][yy][zz];

            if neighbour.collapsed() {
                continue;
            }

            let new_opt = allowed
                .clone()
                .drain()
                .filter(|o| neighbour.options.contains(o))
                .collect::<Vec<_>>();

            if allowed.len() == 0 || new_opt.len() == 0 {
                println!("wtf {:?} {:?} {:?}", self.cells[x][y][z], face, allowed);
                println!("neighbour {:?} {:?}", self.cells[xx][yy][zz], new_opt);
                panic!()
            }

            let prev_count = neighbour.options.len();

            neighbour.options = allowed
                .drain()
                .filter(|o| neighbour.options.contains(o))
                .collect();

            if prev_count > neighbour.options.len() {
                if neighbour.collapsed() {
                    cb(neighbour, glam::uvec3(xx as _, yy as _, zz as _));
                }

                self.propagate(xx, yy, zz, cb);
            }
        }
    }

    pub fn solve(&mut self) -> Vec<Instance> {
        let mut min_entropy: Option<(u32, usize, usize, usize)> = None;
        for x in 0..Self::SIZE_X {
            for y in 0..Self::SIZE_Y {
                for z in 0..Self::SIZE_Z {
                    let cell = &self.cells[x][y][z];

                    if cell.collapsed() {
                        continue;
                    }

                    match min_entropy {
                        None => {
                            min_entropy = Some((cell.entropy(), x, y, z));
                        }
                        Some((min, ..)) if min > cell.entropy() => {
                            min_entropy = Some((cell.entropy(), x, y, z));
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some((_, x, y, z)) = min_entropy {
            let mut instances = vec![];
            let mut instanciate = |cell: &Cell, pos: glam::UVec3| {
                instances.extend(cell.nodes(pos));
            };

            let cell = &mut self.cells[x][y][z];

            let rand = self.noise.get([x as f64, y as f64, z as f64]) * 0.5 + 0.5;
            let idx = (rand.min(0.999) * cell.entropy() as f64).floor() as usize;
            println!("{x} {y} {z} : {rand} --- {:?}", cell.options[idx]);

            cell.options = vec![cell.options[idx]];
            instanciate(cell, glam::uvec3(x as _, y as _, z as _));

            self.propagate(x, y, z, &mut instanciate);

            instances
                .iter()
                .filter_map(|(name, transform)| {
                    self.model.node_instances(name, Some(*transform), None)
                })
                .flat_map(|(instances, _)| instances)
                .collect()
        } else {
            println!("already solved");
            vec![]
        }
    }

    #[allow(dead_code)]
    pub fn collapsed(&self) -> bool {
        self.cells.iter().flatten().flatten().all(Cell::collapsed)
    }

    #[allow(dead_code)]
    pub fn instanciate(&self, renderer: &Renderer, engine: &mut Engine) {
        let mut instances = vec![];

        for x in 0..Self::SIZE_X {
            for y in 0..Self::SIZE_Y {
                for z in 0..Self::SIZE_Z {
                    let cell = &self.cells[x][y][z];

                    if !cell.collapsed() {
                        continue;
                    }

                    let position = glam::uvec3(x as _, y as _, z as _);

                    instances.extend(
                        cell.nodes(position)
                            .iter()
                            .filter_map(|(name, transform)| {
                                self.model.node_instances(name, Some(*transform), None)
                            })
                            .flat_map(|(instances, _)| instances),
                    );
                }
            }
        }

        engine.instances.add(&renderer.queue, instances)
    }
}

#[derive(Debug, Clone)]
struct Cell {
    options: Vec<(Module, Rotation)>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            options: Module::all()
                .into_iter()
                .flat_map(|module| std::iter::repeat(module).zip(module.rotations()))
                .collect(),
        }
    }
}

impl Cell {
    fn entropy(&self) -> u32 {
        self.options.len() as _
    }

    fn collapsed(&self) -> bool {
        self.entropy() == 1
    }

    fn allowed(&self, face: Face) -> HashSet<(Module, Rotation)> {
        self.options
            .iter()
            .flat_map(|(module, rot)| module.allowed(face.rotate(*rot)))
            .collect()
    }

    fn nodes(&self, pos: glam::UVec3) -> Vec<(&'static str, glam::Mat4)> {
        let (module, rot) = self.options[0];
        module.nodes(pos, rot)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dungen() {
        let cell = Cell {
            options: vec![
                (Module::Empty, Rotation::ToNorth),
                (Module::Floor, Rotation::ToNorth),
            ],
        };

        dbg!(cell.allowed(Face::Up));
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum Module {
    Empty,
    Floor,
    SmallStairs,
}

impl Module {
    const fn all() -> impl IntoIterator<Item = Self> {
        [Self::Empty, Self::Floor, Self::SmallStairs]
        // [Self::Empty, Self::Floor]
    }

    fn rotations(&self) -> Vec<Rotation> {
        match self {
            Self::Empty => vec![Rotation::ToNorth],
            Self::Floor => vec![Rotation::ToNorth],
            // Self::SmallStairs => vec![Rotation::ToNorth],
            _ => Rotation::all().into_iter().collect(),
        }
    }

    fn connectors(&self, face: Face) -> Vec<Connector> {
        match self {
            Self::Empty => vec![Connector::Void],
            Self::Floor => match face {
                // Face::Top => vec![Connector::Void],
                Face::Bottom => vec![Connector::Solid],
                _ => vec![Connector::Void, Connector::Solid],
            },
            Self::SmallStairs => match face {
                Face::Top => vec![Connector::Void],
                Face::Bottom => vec![Connector::Solid],

                Face::Up => vec![Connector::Solid],
                Face::Down => vec![Connector::Void],
                _ => vec![Connector::Void, Connector::Solid],
            },
        }
    }

    fn allowed(&self, face: Face) -> Vec<(Self, Rotation)> {
        let connectors = self.connectors(face);

        Self::all()
            .into_iter()
            .flat_map(|module| std::iter::repeat(module).zip(module.rotations()))
            .filter(|(module, rot)| {
                module
                    .connectors(face.opposite().rotate(*rot))
                    .iter()
                    .any(|c| connectors.contains(c))
            })
            .collect()
    }

    fn nodes(&self, pos: glam::UVec3, rot: Rotation) -> Vec<(&'static str, glam::Mat4)> {
        const MODULE_SIZE: glam::Vec3 = glam::vec3(6.0, 2.0, 6.0);
        const SCALE: glam::Vec3 = glam::Vec3::splat(0.01);

        let translation = pos.as_vec3() * MODULE_SIZE;

        match self {
            Self::Empty => vec![(
                "Railing_Pillar_Low_01",
                glam::Mat4::from_scale_rotation_translation(SCALE, rot.into(), translation),
            )],
            Self::Floor => vec![(
                "Base_Thin_01",
                glam::Mat4::from_scale_rotation_translation(SCALE, rot.into(), translation),
            )],
            Self::SmallStairs => vec![(
                "Stair_Pool_01",
                glam::Mat4::from_scale_rotation_translation(
                    SCALE,
                    rot.into(),
                    translation + glam::Vec3::Z,
                ),
            )],
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Connector {
    Void,
    Solid,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Face {
    Up,
    Down,
    Left,
    Right,
    Top,
    Bottom,
}

impl Face {
    const fn all() -> impl IntoIterator<Item = Self> {
        [
            Self::Up,
            Self::Down,
            Self::Left,
            Self::Right,
            Self::Top,
            Self::Bottom,
        ]
    }

    const fn rotate(&self, rot: Rotation) -> Self {
        match self {
            Self::Up => match rot {
                Rotation::ToNorth => *self,
                Rotation::ToEast => Self::Right,
                Rotation::ToSouth => Self::Down,
                Rotation::ToWest => Self::Left,
            },
            Self::Down => match rot {
                Rotation::ToNorth => *self,
                Rotation::ToEast => Self::Left,
                Rotation::ToSouth => Self::Up,
                Rotation::ToWest => Self::Right,
            },
            Self::Left => match rot {
                Rotation::ToNorth => *self,
                Rotation::ToEast => Self::Up,
                Rotation::ToSouth => Self::Right,
                Rotation::ToWest => Self::Down,
            },
            Self::Right => match rot {
                Rotation::ToNorth => *self,
                Rotation::ToEast => Self::Down,
                Rotation::ToSouth => Self::Left,
                Rotation::ToWest => Self::Right,
            },
            _ => *self,
        }
    }

    const fn opposite(&self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[allow(dead_code)]
enum Rotation {
    ToNorth,
    ToEast,
    ToSouth,
    ToWest,
}

impl Rotation {
    const fn all() -> impl IntoIterator<Item = Self> {
        [Self::ToNorth, Self::ToEast, Self::ToSouth, Self::ToWest]
    }

    // fn random() -> Self {
    //     match rand::random::<u32>() % 4 {
    //         0 => Self::ToNorth,
    //         1 => Self::ToEast,
    //         2 => Self::ToSouth,
    //         _ => Self::ToWest,
    //     }
    // }
}

impl From<Rotation> for glam::Quat {
    fn from(rot: Rotation) -> glam::Quat {
        glam::Quat::from_rotation_y(match rot {
            Rotation::ToNorth => 0.0,
            Rotation::ToEast => std::f32::consts::FRAC_PI_2,
            Rotation::ToSouth => std::f32::consts::PI,
            Rotation::ToWest => -std::f32::consts::FRAC_PI_2,
        })
    }
}
