use anyhow::Result;
use calva::{
    gltf::GltfModel,
    renderer::{Engine, Renderer},
};
use noise::Value;

pub struct Chunk {
    model: GltfModel,
    noise: Value,
    slots: [[[Slot; Self::SIZE]; Self::SIZE]; Self::SIZE],
}

impl Chunk {
    const SIZE: usize = 2;

    pub fn new(renderer: &Renderer, engine: &mut Engine) -> Result<Self> {
        let model = GltfModel::from_path(renderer, engine, "./demo/assets/dungeon.glb")?;

        Ok(Self {
            model,
            noise: Value::new(rand::random()),
            slots: Default::default(),
        })
    }

    fn slot(&self, x: usize, y: usize, z: usize) -> &Slot {
        &self.slots[x][y][z]
    }
    fn slot_mut(&mut self, x: usize, y: usize, z: usize) -> &mut Slot {
        &mut self.slots[x][y][z]
    }

    fn collapse(&mut self, x: usize, y: usize, z: usize) {
        for dir in Direction::all() {
            let allowed = self.slot(x, y, z).possibilities[0].0.neighbours(dir);

            let (xx, yy, zz) = match dir {
                Direction::Up if z > 1 => (x, y, z - 1),
                Direction::Down if z < Self::SIZE - 1 => (x, y, z + 1),
                Direction::Left if x > 1 => (x - 1, y, z),
                Direction::Right if x < Self::SIZE - 1 => (x + 1, y, z),
                Direction::Top if y > 1 => (x, y - 1, z),
                Direction::Bottom if y < Self::SIZE - 1 => (x, y + 1, z),
                _ => continue,
            };

            let mut neighbour = &mut self.slots[xx][yy][zz];

            let prev_count = neighbour.possibilities.len();

            neighbour.possibilities = neighbour
                .possibilities
                .drain(..)
                .filter(|possibility| allowed.contains(&possibility))
                .collect();

            if prev_count > neighbour.possibilities.len() {
                self.collapse(xx, yy, zz);
            }
        }
    }

    pub fn solve(&mut self, renderer: &Renderer, engine: &mut Engine) {
        self.slot_mut(0, 0, 0).possibilities = vec![(Module::Floor, Orientation::Up)];
        self.collapse(0, 0, 0);

        loop {
            let mut all_collapsed = true;
            for x in 0..Self::SIZE {
                for y in 0..Self::SIZE {
                    for z in 0..Self::SIZE {
                        all_collapsed &= self.slots[x][y][z].collapsed();
                    }
                }
            }
            if all_collapsed {
                break;
            }

            let mut min_entropy: Option<(u32, usize, usize, usize)> = None;
            for x in 0..Self::SIZE {
                for y in 0..Self::SIZE {
                    for z in 0..Self::SIZE {
                        let slot = &self.slots[x][y][z];

                        if slot.collapsed() {
                            continue;
                        }

                        match min_entropy {
                            None => {
                                min_entropy = Some((slot.entropy(), x, y, z));
                            }
                            Some((min, ..)) if min < slot.entropy() => {
                                min_entropy = Some((slot.entropy(), x, y, z));
                            }
                            _ => {}
                        }
                    }
                }
            }

            if let Some((_, x, y, z)) = min_entropy {
                self.slot_mut(x, y, z).collapse();
                self.collapse(x, y, z);
            }
        }

        dbg!(&self.slots);

        for x in 0..Self::SIZE {
            for y in 0..Self::SIZE {
                for z in 0..Self::SIZE {
                    let slot = self.slot(x, y, z);

                    if !slot.collapsed() {
                        continue;
                    }

                    let position = glam::uvec3(x as _, y as _, z as _);
                    for (name, transform) in slot.instances(position) {
                        self.model
                            .instanciate_mesh(renderer, engine, name, &[(transform, None)])
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Slot {
    possibilities: Vec<(Module, Orientation)>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            possibilities: Module::all()
                .into_iter()
                .flat_map(|module| std::iter::repeat(module).zip(Orientation::all()))
                .collect(),
        }
    }
}

impl Slot {
    fn entropy(&self) -> u32 {
        self.possibilities.len() as _
    }

    fn collapsed(&self) -> bool {
        self.entropy() == 1
    }

    fn collapse(&mut self) {
        self.possibilities = vec![self.possibilities[0]];
    }

    fn instances(&self, cell: glam::UVec3) -> Vec<(&'static str, glam::Mat4)> {
        let (module, orientation) = self.possibilities[0];
        module.instances(cell, orientation)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Module {
    Empty,
    Floor,
}

impl Module {
    const fn all() -> impl IntoIterator<Item = Self> {
        [Self::Empty, Self::Floor]
    }

    fn neighbours(&self, dir: Direction) -> Vec<(Module, Orientation)> {
        match self {
            Self::Empty => match dir {
                Direction::Top | Direction::Bottom => vec![
                    (Self::Empty, Orientation::Up),
                    (Self::Floor, Orientation::Up),
                    (Self::Floor, Orientation::Down),
                    (Self::Floor, Orientation::Left),
                    (Self::Floor, Orientation::Right),
                ],
                _ => vec![(Self::Empty, Orientation::Up)],
            },
            Self::Floor => match dir {
                Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
                    vec![
                        (Self::Floor, Orientation::Up),
                        (Self::Floor, Orientation::Down),
                        (Self::Floor, Orientation::Left),
                        (Self::Floor, Orientation::Right),
                    ]
                }
                Direction::Top | Direction::Bottom => vec![(Self::Empty, Orientation::Up)],
            },
        }
    }

    fn instances(
        &self,
        cell: glam::UVec3,
        orientation: Orientation,
    ) -> Vec<(&'static str, glam::Mat4)> {
        const MODULE_SIZE: glam::Vec3 = glam::vec3(6.0, 2.0, 6.0);
        const SCALE: glam::Vec3 = glam::Vec3::splat(0.01);

        let translation = cell.as_vec3() * MODULE_SIZE;

        match self {
            Self::Empty => vec![],
            Self::Floor => vec![(
                "Floor_Plane_01_32",
                glam::Mat4::from_scale_rotation_translation(SCALE, orientation.into(), translation),
            )],
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
    Top = 4,
    Bottom = 5,
}

impl Direction {
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
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Orientation {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
}

impl Orientation {
    const fn all() -> impl IntoIterator<Item = Self> {
        [Self::Up, Self::Down, Self::Left, Self::Right]
    }

    fn random() -> Self {
        match rand::random::<u32>() % 4 {
            0 => Self::Up,
            1 => Self::Down,
            2 => Self::Left,
            _ => Self::Right,
        }
    }

    fn axis(&self) -> glam::Vec3 {
        match self {
            Self::Up => glam::Vec3::Z,
            Self::Down => -glam::Vec3::Z,
            Self::Left => -glam::Vec3::X,
            Self::Right => glam::Vec3::X,
        }
    }

    fn cross_axis(&self) -> glam::Vec3 {
        self.axis().cross(glam::Vec3::Y)
    }
}

impl From<Orientation> for glam::Quat {
    fn from(orientation: Orientation) -> glam::Quat {
        glam::Quat::from_rotation_y(match orientation {
            Orientation::Up => 0.0,
            Orientation::Down => std::f32::consts::PI,
            Orientation::Left => -std::f32::consts::FRAC_PI_2,
            Orientation::Right => std::f32::consts::FRAC_PI_2,
        })
    }
}
