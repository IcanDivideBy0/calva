use std::hash::Hash;

use noise::NoiseFn;
use rand_seeder::SipHasher;

use crate::worldgen::wfc::WfcConfig;

use super::{
    tile::Tile,
    wfc::{wfc, WfcConstraints},
};

pub struct WorldChunk<const SIZE: usize, const MODULE_SIZE: usize> {
    world_pos: glam::Vec3,
    grid: [[(usize, f32, i8); SIZE]; SIZE],
}

impl<const SIZE: usize, const MODULE_SIZE: usize> WorldChunk<SIZE, MODULE_SIZE> {
    pub const WORLD_SIZE: f32 = Tile::WORLD_SIZE * SIZE as f32;

    pub fn new<'t>(
        tiles: &mut impl Iterator<Item = (&'t usize, &'t Tile)>,
        seed: impl Hash,
        _noise: &dyn NoiseFn<f64, 2>,
        coord: glam::IVec2,
    ) -> Self {
        let rng = SipHasher::from((seed, coord)).into_rng();

        let constraints = WfcConstraints {
            north: std::array::from_fn(|_| std::array::from_fn(|_| Some(0.0))),
            east: std::array::from_fn(|_| std::array::from_fn(|_| Some(0.0))),
            south: std::array::from_fn(|_| std::array::from_fn(|_| Some(0.0))),
            west: std::array::from_fn(|_| std::array::from_fn(|_| Some(0.0))),
        };

        let grid = wfc::<SIZE, MODULE_SIZE>(
            tiles,
            WfcConfig {
                constraints,
                elevations: 4,
                elevations_increments: 4,
                rng: Box::new(rng),
            },
        );

        let world_pos = glam::vec3(
            coord.x as f32 * Self::WORLD_SIZE,
            0.0,
            coord.y as f32 * Self::WORLD_SIZE,
        );

        Self { world_pos, grid }
    }

    pub fn get_tile_id(&self, coord: glam::USizeVec2) -> usize {
        self.grid[coord.y][coord.x].0
    }

    pub fn get_tile_transform(&self, coord: glam::USizeVec2) -> glam::Mat4 {
        let (_, angle, elevation) = self.grid[coord.y][coord.x];

        let rotation = glam::Quat::from_axis_angle(glam::Vec3::Y, angle);

        let translation = glam::vec3(
            (coord.x as f32 + 0.5) * Tile::WORLD_SIZE,
            elevation as f32,
            (coord.y as f32 + 0.5) * Tile::WORLD_SIZE,
        ) + self.world_pos;

        glam::Mat4::from_rotation_translation(rotation, translation)
    }

    pub fn ray_cast<'a>(
        &self,
        ro: glam::Vec3,
        rd: glam::Vec3,
        get_tile: impl Fn(usize) -> &'a Tile,
    ) -> Option<f32> {
        itertools::iproduct!(0..SIZE, 0..SIZE)
            .map(|(y, x)| glam::usizevec2(x, y))
            .fold(None, |prev_hit, coord| {
                let tile_space_transform = self.get_tile_transform(coord).inverse();

                let hit = get_tile(self.get_tile_id(coord)).height_map.ray_cast(
                    tile_space_transform.transform_point3(ro),
                    tile_space_transform.transform_vector3(rd),
                );

                match (hit, prev_hit) {
                    (Some(hit), Some(prev_hit)) => Some(f32::min(hit, prev_hit)),
                    _ => Option::or(hit, prev_hit),
                }
            })
    }
}
