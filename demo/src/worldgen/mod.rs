use noise::NoiseFn;
use rand::prelude::*;
use rand_seeder::SipHasher;
use std::fmt;
use std::{cell::RefCell, collections::BTreeSet, hash::Hash};

use calva::{
    gltf::GltfModel,
    renderer::{Instance, PointLight},
};

pub mod navmesh;
pub mod tile;

pub use tile::{Face, Tile};

pub struct WorldGenerator {
    seed: u32,
    noise: Box<dyn NoiseFn<f64, 2>>,
    options: BTreeSet<SlotOption>,
}

impl WorldGenerator {
    pub fn new(seed: impl Hash) -> Self {
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
            seed,
            noise,
            options: BTreeSet::new(),
        }
    }

    pub fn set_tiles(&mut self, tiles: &[Tile]) {
        self.options = tiles.iter().flat_map(SlotOption::permutations).collect();
    }

    pub fn chunk(&self, model: &GltfModel, coord: glam::IVec2) -> (Vec<Instance>, Vec<PointLight>) {
        let chunk = Chunk::new(self.seed, coord, self.noise.as_ref(), &self.options);

        // dbg!(&chunk);

        let mut instances = vec![];
        let mut point_lights = vec![];

        let offset = coord * (Chunk::SIZE as i32);

        itertools::iproduct!(0..Chunk::SIZE, 0..Chunk::SIZE)
            .filter_map(|(x, y)| -> Option<(Vec<Instance>, Vec<PointLight>)> {
                let slot = chunk.grid[y][x].borrow();
                let opt = slot.options.first()?;

                Some(model.node_instances(
                    model.doc.nodes().nth(opt.id)?,
                    Some(opt.transform(offset + glam::ivec2(x as _, y as _))),
                    None,
                ))
            })
            .for_each(|res| {
                instances.extend(res.0);
                point_lights.extend(res.1);
            });

        (instances, point_lights)
    }
}

type ChunkGrid = [[RefCell<Slot>; Chunk::SIZE]; Chunk::SIZE];
pub struct Chunk {
    coord: glam::IVec2,
    grid: ChunkGrid,
}

impl Chunk {
    pub const SIZE: usize = 3;
    pub const WORLD_SIZE: f32 = Self::SIZE as f32 * Tile::WORLD_SIZE;

    fn new(
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
                    Some(u8::max(elevation_start, elevation_end)),
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
            slot.options = [*slot.options.iter().choose(&mut rng).unwrap_or_else(|| {
                eprintln!("Cannot generate chunk {coord}");
                options.first().unwrap()
            })]
            .into();
            drop(slot);

            Self::propagate(&grid, x, y);
        }

        Self { coord, grid }
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
                // #[allow(clippy::absurd_extreme_comparisons)]
                Face::East if x < Self::SIZE - 1 => (x + 1, y),
                // #[allow(clippy::absurd_extreme_comparisons)]
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

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use colored::Colorize;
        let get_constraint = |face: Face, i: usize| {
            let (x, y) = match face {
                Face::North => (i, 0),
                Face::East => (Self::SIZE - 1, i),
                Face::South => (Self::SIZE - 1 - i, Self::SIZE - 1),
                Face::West => (0, Self::SIZE - 1 - i),
            };

            self.grid[y][x]
                .borrow()
                .options
                .first()
                .unwrap()
                .constraint(face)
                .map(|c| match c {
                    Some(x) => {
                        let c = (x + 1) * 31;
                        format!("{}", x.to_string().bold().red().on_truecolor(c, c, c))
                    }
                    None => String::from(" "),
                })
        };

        let north: [_; Self::SIZE] = std::array::from_fn(|i| get_constraint(Face::North, i));

        writeln!(f, "Chunk ({},{})", self.coord.x, self.coord.y)?;

        writeln!(
            f,
            "{}",
            north
                .iter()
                .enumerate()
                .map(|(idx, c)| c[(idx.min(1))..].join(""))
                .collect::<Vec<_>>()
                .join("")
        )?;

        let east: [_; Self::SIZE] = std::array::from_fn(|i| get_constraint(Face::East, i));
        let west: [_; Self::SIZE] = std::array::from_fn(|i| get_constraint(Face::West, i));
        for y in 0..Self::SIZE {
            let e = &east[y];
            let w = &west[Self::SIZE - 1 - y];

            let end = if y < Self::SIZE - 1 {
                SlotOption::WFC_SAMPLES
            } else {
                SlotOption::WFC_SAMPLES - 1
            };

            for i in 1..end {
                let last = i == SlotOption::WFC_SAMPLES - 1;

                let c = if last { "┼" } else { "│" };
                let mut strs = [c; Self::SIZE + 1];

                strs[0] = w[SlotOption::WFC_SAMPLES - 1 - i].as_str();
                strs[Self::SIZE] = e[i].as_str();

                let sep = if last { "─" } else { " " };
                writeln!(f, "{}", strs.join(&sep.repeat(SlotOption::WFC_SAMPLES - 2)))?;
            }
        }

        let mut south: [_; Self::SIZE] = std::array::from_fn(|i| get_constraint(Face::South, i));
        writeln!(
            f,
            "{}",
            south
                .iter_mut()
                .rev()
                .enumerate()
                .map(|(idx, c)| {
                    c.reverse();
                    c[(idx.min(1))..].join("")
                })
                .collect::<Vec<_>>()
                .join("")
        )?;

        Ok(())
    }
}

#[derive(Debug)]
struct Slot {
    options: BTreeSet<SlotOption>,
}

impl Slot {
    fn entropy(&self) -> usize {
        self.options.len()
    }

    fn collapsed(&self) -> bool {
        self.entropy() == 1
    }

    fn constraints(&self, face: Face) -> impl Iterator<Item = SlotConstraint> + '_ {
        self.options.iter().map(move |opt| opt.constraint(face))
    }

    fn apply_constraints(&mut self, face: Face, constraints: &[SlotConstraint]) -> bool {
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

type SlotConstraint = [Option<u8>; SlotOption::WFC_SAMPLES];

#[derive(Clone, Copy)]
struct SlotOption {
    id: usize,
    elevation: u8,
    rotation: u8,
    constraints: [SlotConstraint; 4], // north east south west
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

impl std::cmp::Ord for SlotOption {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id
            .cmp(&other.id)
            .then(self.elevation.cmp(&other.elevation))
            .then(self.rotation.cmp(&other.rotation))
    }
}

impl std::cmp::PartialOrd for SlotOption {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl SlotOption {
    const WFC_SAMPLES: usize = 5;
    const FLOOR_HEIGHT: f32 = 4.0;

    const ELEVATION_MAX: usize = 4;

    fn constraint(&self, face: Face) -> SlotConstraint {
        match face {
            Face::North => self.constraints[0],
            Face::East => self.constraints[1],
            Face::South => self.constraints[2],
            Face::West => self.constraints[3],
        }
    }

    fn permutations(tile: &Tile) -> impl Iterator<Item = Self> + '_ {
        fn wfc_to_world(i: usize) -> f32 {
            const STEP: f32 = Tile::WORLD_SIZE / SlotOption::WFC_SAMPLES as f32;
            const HALF: f32 = STEP / 2.0;

            i as f32 * STEP + HALF
        }

        let mut constraints = Face::all().map(|face| {
            std::array::from_fn(|i| {
                let reverse = |i: usize| Self::WFC_SAMPLES - 1 - i;

                let height = tile.get_height(
                    match face {
                        Face::North => [wfc_to_world(i), 0.0],
                        Face::East => [Tile::WORLD_SIZE, wfc_to_world(i)],
                        Face::South => [wfc_to_world(reverse(i)), Tile::WORLD_SIZE],
                        Face::West => [0.0, wfc_to_world(reverse(i))],
                    }
                    .into(),
                );

                let floor_level = (height / Self::FLOOR_HEIGHT).round();
                u8::try_from(floor_level as i32).ok()
            })
        });

        (0..4).flat_map(move |rotation| {
            let it = (0..=(Self::ELEVATION_MAX as u8 + 2)).map(move |elevation| Self {
                id: tile.node_id,
                elevation,
                rotation,
                constraints: constraints
                    .map(|constraint| constraint.map(|value| value.map(|i| i + elevation))),
            });

            // Rotate faces
            constraints = [
                constraints[3],
                constraints[0],
                constraints[1],
                constraints[2],
            ];

            it
        })
    }

    fn transform(&self, pos: glam::IVec2) -> glam::Mat4 {
        let quat = glam::Quat::from_rotation_y(self.rotation as f32 * -std::f32::consts::FRAC_PI_2);

        let translation = glam::vec3(pos.x as f32, self.elevation as f32, pos.y as f32)
            * glam::vec3(Tile::WORLD_SIZE, Self::FLOOR_HEIGHT, Tile::WORLD_SIZE);

        glam::Mat4::from_rotation_translation(quat, translation)
    }
}

impl fmt::Debug for SlotOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use colored::Colorize;
        let get_visual_constraint = |face: Face| {
            self.constraint(face).map(|c| match c {
                Some(x) => {
                    let c = (x + 1) * 31;
                    format!("{x}").bold().red().on_truecolor(c, c, c)
                }
                None => " ".normal(),
            })
        };

        let north = get_visual_constraint(Face::North);
        let east = get_visual_constraint(Face::East);
        let south = get_visual_constraint(Face::South);
        let west = get_visual_constraint(Face::West);

        writeln!(
            f,
            "SlotOption {{ id[{}], elevetion[{}], rotation [{}] }}",
            self.id, self.elevation, self.rotation
        )?;

        writeln!(
            f,
            "╔{}{}{}{}{}╗",
            north[0], north[1], north[2], north[3], north[4],
        )?;

        writeln!(f, "{}     {}", west[4], east[0])?;
        writeln!(f, "{}     {}", west[3], east[1])?;
        writeln!(f, "{}     {}", west[2], east[2])?;
        writeln!(f, "{}     {}", west[1], east[3])?;
        writeln!(f, "{}     {}", west[0], east[4])?;

        writeln!(
            f,
            "╚{}{}{}{}{}╝",
            south[4], south[3], south[2], south[1], south[0],
        )?;

        Ok(())
    }
}
