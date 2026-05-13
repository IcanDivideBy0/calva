use std::hash::Hash;

use calva::renderer::Object;
use noise::NoiseFn;
use rand_seeder::SipHasher;

use crate::worldgen::wfc::WfcConfig;

use super::{
    tile::Tile,
    wfc::{wfc, WfcConstraints},
};

pub struct WorldChunk<const SIZE: usize, const MODULE_SIZE: usize> {
    #[allow(dead_code)]
    objects: Vec<Object>,

    world_pos: glam::Vec3,
    tiles: [[Option<(usize, f32, i8)>; SIZE]; SIZE],
}

impl<const SIZE: usize, const MODULE_SIZE: usize> WorldChunk<SIZE, MODULE_SIZE> {
    pub const WORLD_SIZE: f32 = Tile::WORLD_SIZE * SIZE as f32;

    pub fn new<'t>(
        tiles: &mut impl Iterator<Item = (&'t usize, &'t Tile)>,
        seed: impl Hash,
        _noise: &dyn NoiseFn<f64, 2>,
        coord: glam::IVec2,
        tile_object: impl Fn(usize) -> Object,
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

        let mut tiles = std::array::from_fn(|_| std::array::from_fn(|_| Default::default()));

        let objects = itertools::iproduct!(0..SIZE, 0..SIZE)
            .map(|(y, x)| {
                let cell = grid[y][x];
                tiles[y][x] = Some(cell);

                let (tile_id, angle, elevation) = cell;

                tile_object(tile_id).with_transform(glam::Mat4::from_rotation_translation(
                    glam::Quat::from_axis_angle(glam::Vec3::Y, angle),
                    glam::vec3(
                        (x as f32 + 0.5) * Tile::WORLD_SIZE,
                        elevation as f32,
                        (y as f32 + 0.5) * Tile::WORLD_SIZE,
                    ) + world_pos,
                ))
            })
            .collect::<Vec<_>>();

        Self {
            objects,
            world_pos,
            tiles,
        }
    }

    pub fn ray_cast<'a>(
        &self,
        ro: glam::Vec3,
        rd: glam::Vec3,
        tile_getter: impl Fn(usize) -> &'a Tile,
    ) -> Option<f32> {
        itertools::iproduct!(0..SIZE, 0..SIZE).fold(None, |prev_hit, (y, x)| {
            let Some((tile_id, angle, elevation)) = self.tiles[y][x] else {
                return prev_hit;
            };

            let tile = tile_getter(tile_id);

            let rotation = glam::Quat::from_axis_angle(glam::Vec3::Y, angle);

            let translation = glam::vec3(
                (x as f32 + 0.5) * Tile::WORLD_SIZE,
                elevation as f32,
                (y as f32 + 0.5) * Tile::WORLD_SIZE,
            ) + self.world_pos;

            let tile_space_transform = glam::Mat4::from_rotation_translation(rotation, translation);

            let hit = tile.height_map.ray_cast(
                tile_space_transform.inverse().transform_point3(ro),
                rotation.inverse() * rd,
            );

            match (hit, prev_hit) {
                (Some(hit), Some(prev_hit)) => Some(f32::min(hit, prev_hit)),
                _ => Option::or(hit, prev_hit),
            }
        })
    }
}
