use calva::{
    nav::{HeightMap, HeightMapBuilder},
    renderer::wgpu,
};

#[derive(Debug)]
pub struct Tile {
    #[allow(dead_code)]
    pub depth: wgpu::Texture,
    pub height_map: HeightMap<{ HeightMapBuilder::TEXTURE_SIZE }>,
}

impl Tile {
    pub const WORLD_SIZE: f32 = 5.0 * 6.0;

    // pub const PIXEL_SIZE: f32 = Self::WORLD_SIZE / SIZE as f32;

    // pub fn _world_get_height(&self, pos: glam::Vec2) -> Option<f32> {
    //     let coord = (pos / Self::WORLD_SIZE * SIZE as f32).clamp(
    //         glam::vec2(0.0, 0.0),
    //         glam::vec2((SIZE - 1) as f32, (SIZE - 1) as f32),
    //     );

    //     self.height_map.grid[coord.y.floor() as usize][coord.x.floor() as usize]
    // }

    // pub fn _get_grid_coord(local_coord: &glam::Vec2) -> glam::USizeVec2 {
    //     let coord_norm = local_coord / Self::WORLD_SIZE + 0.5;
    //     let grid_coord = (coord_norm * SIZE as f32)
    //         .clamp(glam::Vec2::ZERO, glam::Vec2::splat((SIZE - 1) as f32));

    //     glam::usizevec2(grid_coord.x as usize, grid_coord.y as usize)
    // }
}
