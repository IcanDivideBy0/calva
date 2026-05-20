use calva::nav::HeightMap;
use glam::Vec3Swizzles;

use crate::worldgen::{
    wfc::{Module, Wfc},
    WorldGenerator,
};

pub struct WorldChunk<const SIZE: usize, const WFC_MODULE_SIZE: usize> {
    pub world_pos: glam::Vec2,
    pub grid: [[Module<WFC_MODULE_SIZE>; SIZE]; SIZE],
}

impl<const SIZE: usize, const WFC_MODULE_SIZE: usize> WorldChunk<SIZE, WFC_MODULE_SIZE> {
    pub const WORLD_SIZE: f32 = WorldGenerator::TILE_WORLD_SIZE * SIZE as f32;

    pub fn new(coord: glam::IVec2, wfc: &Wfc<SIZE, WFC_MODULE_SIZE>) -> Self {
        let grid = wfc.collapse(coord);

        let world_pos = glam::vec2(
            coord.x as f32 * Self::WORLD_SIZE,
            coord.y as f32 * Self::WORLD_SIZE,
        );

        Self { world_pos, grid }
    }

    pub fn get_object_transform(&self, coord: glam::USizeVec2) -> glam::Mat4 {
        let module = self.grid[coord.y][coord.x];

        let translation = (glam::vec2(coord.x as f32, coord.y as f32) + 0.5)
            * WorldGenerator::TILE_WORLD_SIZE
            + self.world_pos;

        glam::Mat4::from_translation(translation.extend(0.0).xzy()) * module.get_local_transform()
    }

    pub fn module_index(&self, world_pos: glam::Vec2) -> glam::USizeVec2 {
        let grid_pos = world_pos - self.world_pos;

        glam::usizevec2(
            (grid_pos.x / WorldGenerator::TILE_WORLD_SIZE) as _,
            (grid_pos.y / WorldGenerator::TILE_WORLD_SIZE) as _,
        )
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

                let hit = get_height_map(self.grid[coord.y][coord.x].id).ray_cast(
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
