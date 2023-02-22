use anyhow::Result;
use calva::{
    gltf::GltfModel,
    renderer::{Engine, Instance, Renderer},
};
use noise::NoiseFn;
// use rand::prelude::*;

pub struct Dungen {
    dungeon: GltfModel,
    noise: noise::Value,

    pub seed: u32,
}

// https://en.wikipedia.org/wiki/Box-drawing_character
// https://vazgriz.com/119/procedurally-generated-dungeons/

impl Dungen {
    const WORLD_SIZE: i32 = 32;

    pub fn new(renderer: &Renderer, engine: &mut Engine, seed: Option<u32>) -> Result<Self> {
        let dungeon = GltfModel::from_path(renderer, engine, "./demo/assets/dungeon.glb")?;
        let seed = seed.unwrap_or_else(rand::random);
        let noise = noise::Value::new(seed);

        Ok(Self {
            dungeon,
            noise,

            seed,
        })
    }

    pub fn gen(&self, renderer: &Renderer, engine: &mut Engine) {
        let instances = (0..Self::WORLD_SIZE)
            .flat_map(|z| (0..Self::WORLD_SIZE).flat_map(move |x| self.get_tile_instances(x, z)))
            .collect::<Vec<_>>();

        engine.instances.add(&renderer.queue, &instances);
    }

    fn get_tile_instances(&self, x: i32, y: i32) -> Vec<Instance> {
        let mut instances = vec![];

        let tile = Tile::new(x, y, &self.noise);

        if let Some(floor_level) = tile.floor_level() {
            instances = self.place_floor(&tile, floor_level).collect();

            for dir in Direction::values() {
                match tile.neighbour(dir).floor_level() {
                    Some(level) if level < floor_level => {
                        instances.extend(self.place_railling(&tile, floor_level, dir));
                        instances.extend(self.place_wall(&tile, floor_level, dir));
                    }
                    None => {
                        instances.extend(self.place_railling(&tile, floor_level, dir));
                        instances.extend(self.place_wall(&tile, floor_level, dir));
                    }
                    _ => {}
                }
            }
        }

        instances
    }

    fn place_floor(&self, tile: &Tile, floor_level: u32) -> impl Iterator<Item = Instance> + '_ {
        let transform = glam::Mat4::from_scale_rotation_translation(
            Tile::MODEL_SCALE,
            Direction::random().rotation(),
            tile.world_coordinates(floor_level),
        );

        self.dungeon.mesh_instances("Floor_Plane_01_32", transform)
    }

    fn place_wall(
        &self,
        tile: &Tile,
        floor_level: u32,
        direction: Direction,
    ) -> impl Iterator<Item = Instance> + '_ {
        let transform = glam::Mat4::from_scale_rotation_translation(
            Tile::MODEL_SCALE,
            direction.rotation(),
            tile.world_coordinates(floor_level) - glam::Vec3::Y * Tile::TILE_SCALE
                + direction.shift() * Tile::TILE_SCALE / 2.0,
        );

        self.dungeon.mesh_instances("Wall_Plane_01_64", transform)
    }

    fn place_railling(
        &self,
        tile: &Tile,
        floor_level: u32,
        direction: Direction,
    ) -> impl Iterator<Item = Instance> + '_ {
        let railling_transform = glam::Mat4::from_scale_rotation_translation(
            Tile::MODEL_SCALE,
            direction.rotation(),
            tile.world_coordinates(floor_level) + direction.shift() * Tile::TILE_SCALE / 2.0,
        );
        let pillar_transform = glam::Mat4::from_scale_rotation_translation(
            Tile::MODEL_SCALE,
            glam::Quat::IDENTITY,
            tile.world_coordinates(floor_level)
                + (direction.shift() + direction.cross()) * Tile::TILE_SCALE / 2.0,
        );

        std::iter::Iterator::chain(
            self.dungeon
                .mesh_instances(tile.railling(direction), railling_transform),
            self.dungeon
                .mesh_instances("Railing_Pillar_01", pillar_transform),
        )
    }
}

struct Tile<'a> {
    x: i32,
    y: i32,
    value: f64,
    noise: &'a noise::Value,
}

impl<'a> Tile<'a> {
    const MODEL_SCALE: glam::Vec3 = glam::Vec3::splat(0.01);
    const TILE_SCALE: glam::Vec3 = glam::vec3(6.0, 2.0, 6.0);

    fn new(x: i32, y: i32, noise: &'a noise::Value) -> Self {
        const SCALE: f64 = 0.225;
        let value = noise.get([x as f64 * SCALE, y as f64 * SCALE]) * 0.5 + 0.5;

        Self { x, y, value, noise }
    }

    fn floor_level(&self) -> Option<u32> {
        match (self.value * 8.0) as i32 {
            6.. => Some(2),
            4.. => Some(1),
            1.. => Some(0),
            _ => None,
        }
    }

    fn world_coordinates(&self, floor_level: u32) -> glam::Vec3 {
        const SCALE: glam::Vec3 = glam::vec3(6.0, 2.0, 6.0);
        SCALE * glam::vec3(self.x as f32, floor_level as f32, self.y as f32)
    }

    fn neighbour(&self, direction: Direction) -> Self {
        match direction {
            Direction::North => Self::new(self.x, self.y + 1, self.noise),
            Direction::South => Self::new(self.x, self.y - 1, self.noise),
            Direction::East => Self::new(self.x + 1, self.y, self.noise),
            Direction::West => Self::new(self.x - 1, self.y, self.noise),
        }
    }

    fn railling(&self, direction: Direction) -> &'static str {
        let shift = direction.shift() + direction.cross();
        let x = self.x as f64 + shift.x as f64;
        let y = self.y as f64 + shift.z as f64;
        let rng = self.noise.get([x, y]) * 0.5 + 0.5;

        match (rng * 10.0) as u32 {
            9 => "Damaged_Railing_07",
            _ => "Railing_01",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    fn random() -> Self {
        match rand::random::<u32>() % 4 {
            0 => Self::North,
            1 => Self::South,
            2 => Self::East,
            _ => Self::West,
        }
    }

    fn values() -> [Self; 4] {
        [Self::North, Self::South, Self::East, Self::West]
    }

    fn shift(&self) -> glam::Vec3 {
        match self {
            Self::North => glam::Vec3::Z,
            Self::South => -glam::Vec3::Z,
            Self::East => glam::Vec3::X,
            Self::West => -glam::Vec3::X,
        }
    }

    fn cross(&self) -> glam::Vec3 {
        self.shift().cross(glam::Vec3::Y)
    }

    fn rotation(&self) -> glam::Quat {
        glam::Quat::from_rotation_y(match self {
            Self::North => 0.0,
            Self::South => std::f32::consts::PI,
            Self::East => std::f32::consts::FRAC_PI_2,
            Self::West => -std::f32::consts::FRAC_PI_2,
        })
    }
}
