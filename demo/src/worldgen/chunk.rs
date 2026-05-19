use std::hash::Hash;

use calva::nav::HeightMap;
use noise::NoiseFn;
use rand_seeder::SipHasher;

use crate::worldgen::{
    wfc::{Module, ModuleRotation, Wfc, WfcConstraints},
    WorldGenerator,
};

pub struct WorldChunk<const SIZE: usize, const WFC_MODULE_SIZE: usize> {
    pub world_pos: glam::Vec3,
    pub grid: [[Module<WFC_MODULE_SIZE>; SIZE]; SIZE],
}

impl<const SIZE: usize, const WFC_MODULE_SIZE: usize> WorldChunk<SIZE, WFC_MODULE_SIZE> {
    pub const WORLD_SIZE: f32 = WorldGenerator::TILE_WORLD_SIZE * SIZE as f32;

    pub fn new(
        coord: glam::IVec2,
        seed: impl Hash,
        noise: &dyn NoiseFn<f64, 2>,
        wfc: &Wfc<SIZE, WFC_MODULE_SIZE>,
    ) -> Self {
        let get_noise = |x: usize, y: usize| {
            let noise = noise.get([
                coord.x as f64 * SIZE as f64 + x as f64,
                coord.y as f64 * SIZE as f64 + y as f64,
            ]) as f32;

            let h = noise * wfc.elevations as f32;
            h.floor() * wfc.elevations_increments as f32
        };

        let mut constraints: WfcConstraints<SIZE, WFC_MODULE_SIZE> = WfcConstraints {
            north: std::array::from_fn(|_| std::array::from_fn(|_| None)),
            east: std::array::from_fn(|_| std::array::from_fn(|_| None)),
            south: std::array::from_fn(|_| std::array::from_fn(|_| None)),
            west: std::array::from_fn(|_| std::array::from_fn(|_| None)),
        };

        for y in 0..SIZE {
            for x in 0..SIZE {
                let top_left = get_noise(x, y);
                let top_right = get_noise(x + 1, y);
                let bottom_left = get_noise(x, y + 1);
                let bottom_right = get_noise(x + 1, y + 1);

                if y == 0 {
                    constraints.north[x] = std::array::from_fn(|i| {
                        if (WFC_MODULE_SIZE as f32 / 2.0) > (i as f32) + 1.0 {
                            Some(top_left)
                        } else if (WFC_MODULE_SIZE as f32 / 2.0) < (i as f32) {
                            Some(top_right)
                        } else {
                            Some(f32::max(top_left, top_right))
                        }
                    });
                }

                if x == SIZE - 1 {
                    constraints.east[y] = std::array::from_fn(|i| {
                        if (WFC_MODULE_SIZE as f32 / 2.0) > (i as f32) + 1.0 {
                            Some(top_right)
                        } else if (WFC_MODULE_SIZE as f32 / 2.0) < (i as f32) {
                            Some(bottom_right)
                        } else {
                            Some(f32::max(top_right, bottom_right))
                        }
                    });
                }

                if y == SIZE - 1 {
                    constraints.south[x] = std::array::from_fn(|i| {
                        if (WFC_MODULE_SIZE as f32 / 2.0) > (i as f32) + 1.0 {
                            Some(bottom_left)
                        } else if (WFC_MODULE_SIZE as f32 / 2.0) < (i as f32) {
                            Some(bottom_right)
                        } else {
                            Some(f32::max(bottom_left, bottom_right))
                        }
                    });
                }

                if x == 0 {
                    constraints.west[y] = std::array::from_fn(|i| {
                        if (WFC_MODULE_SIZE as f32 / 2.0) > (i as f32) + 1.0 {
                            Some(top_left)
                        } else if (WFC_MODULE_SIZE as f32 / 2.0) < (i as f32) {
                            Some(bottom_left)
                        } else {
                            Some(f32::max(top_left, bottom_left))
                        }
                    });
                }
            }
        }

        let grid = wfc.collapse(constraints, &mut SipHasher::from((seed, coord)).into_rng());

        let world_pos = glam::vec3(
            coord.x as f32 * Self::WORLD_SIZE,
            0.0,
            coord.y as f32 * Self::WORLD_SIZE,
        );

        Self { world_pos, grid }
    }

    pub fn get_height_map_id(&self, coord: glam::USizeVec2) -> usize {
        self.grid[coord.y][coord.x].id
    }

    pub fn get_object_transform(&self, coord: glam::USizeVec2) -> glam::Mat4 {
        let module = self.grid[coord.y][coord.x];

        let rotation = glam::Quat::from_axis_angle(glam::Vec3::Y, module.rotation.angle());

        let translation = glam::vec3(
            (coord.x as f32 + 0.5) * WorldGenerator::TILE_WORLD_SIZE,
            module.elevation as f32,
            (coord.y as f32 + 0.5) * WorldGenerator::TILE_WORLD_SIZE,
        ) + self.world_pos;

        glam::Mat4::from_rotation_translation(rotation, translation)
    }

    pub fn get_height_map_data<'h>(
        &self,
        get_height_map: impl Fn(usize) -> &'h HeightMap,
    ) -> [[Option<f32>; WorldGenerator::CHUNK_SIZE * HeightMap::SIZE];
           WorldGenerator::CHUNK_SIZE * HeightMap::SIZE] {
        let mut height_map_data = std::array::from_fn(|_| std::array::from_fn(|_| None));

        for (grid_y, grid_x) in itertools::iproduct!(0..SIZE, 0..SIZE) {
            let module = self.grid[grid_y][grid_x];
            let height_map = get_height_map(module.id);

            for (y, x) in itertools::iproduct!(0..HeightMap::SIZE, 0..HeightMap::SIZE) {
                const MAX: usize = HeightMap::SIZE - 1;

                height_map_data[grid_y * HeightMap::SIZE + y][grid_x * HeightMap::SIZE + x] =
                    match module.rotation {
                        ModuleRotation::Cw0 => height_map.grid[y][x],
                        ModuleRotation::Cw90 => height_map.grid[MAX - x][y],
                        ModuleRotation::Cw180 => height_map.grid[MAX - y][MAX - x],
                        ModuleRotation::Cw270 => height_map.grid[x][MAX - y],
                    };
            }
        }

        height_map_data
    }

    pub fn ray_cast<'h>(
        &self,
        ro: glam::Vec3,
        rd: glam::Vec3,
        get_height_map: impl Fn(usize) -> &'h HeightMap,
    ) -> Option<f32> {
        itertools::iproduct!(0..SIZE, 0..SIZE)
            .map(|(y, x)| glam::usizevec2(x, y))
            .fold(None, |prev_hit, coord| {
                let height_map_space_transform = self.get_object_transform(coord).inverse();

                let hit = get_height_map(self.get_height_map_id(coord)).ray_cast(
                    height_map_space_transform.transform_point3(ro),
                    height_map_space_transform.transform_vector3(rd),
                );

                match (hit, prev_hit) {
                    (Some(hit), Some(prev_hit)) => Some(f32::min(hit, prev_hit)),
                    _ => Option::or(hit, prev_hit),
                }
            })
    }
}
