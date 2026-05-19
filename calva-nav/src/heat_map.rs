use core::{f32, fmt};
use std::collections::VecDeque;

use crate::util::debug_map;

pub struct HeatMap<const SIZE: usize> {
    grid: [[Option<f32>; SIZE]; SIZE],
}

impl<const SIZE: usize> HeatMap<SIZE> {
    pub fn new(height_map_data: &[[Option<f32>; SIZE]; SIZE], target: glam::USizeVec2) -> Self {
        let mut grid = [[None; SIZE]; SIZE];
        grid[target.y][target.x] = Some(0.0);

        let mut open_list = VecDeque::from([target]);

        while let Some(head) = open_list.pop_front() {
            for coord in itertools::iproduct!(
                head.y.saturating_sub(1)..=head.y.saturating_add(1).min(SIZE - 1),
                head.x.saturating_sub(1)..=head.x.saturating_add(1).min(SIZE - 1),
            )
            .map(|(y, x)| glam::usizevec2(x, y))
            {
                if height_map_data[coord.y.min(SIZE)][coord.x.min(SIZE)].is_none() {
                    continue;
                }

                let mut dist = if head.x == coord.x || head.y == coord.y {
                    1.0
                } else {
                    f32::consts::SQRT_2
                };
                dist += grid[head.y][head.x].unwrap_or_default();

                if let Some(ref mut d) = grid[coord.y][coord.x] {
                    *d = d.min(dist)
                } else {
                    grid[coord.y][coord.x] = Some(dist);
                    open_list.push_back(coord);
                }
            }
        }

        Self { grid }
    }

    pub fn apply_kernel(&self, coord: glam::USizeVec2) -> glam::Vec2 {
        if self.grid[coord.y][coord.x] == Some(0.0) {
            return glam::Vec2::ZERO;
        }

        itertools::iproduct!(
            coord.y.saturating_sub(1)..=coord.y.saturating_add(1).min(SIZE - 1),
            coord.x.saturating_sub(1)..=coord.x.saturating_add(1).min(SIZE - 1),
        )
        .fold(glam::Vec2::ZERO, |acc, (y, x)| {
            if let Some(heat) = self.grid[y][x] {
                let dir = (glam::vec2(x as f32 - coord.x as f32, y as f32 - coord.y as f32))
                    .normalize_or_zero();

                acc + dir / (heat + f32::EPSILON)
            } else {
                acc
            }
        })
        .normalize_or_zero()
    }
}

impl<const SIZE: usize> fmt::Debug for HeatMap<SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max = self
            .grid
            .iter()
            .flatten()
            .filter_map(|h| *h)
            .fold(f32::MIN, f32::max);

        write!(
            f,
            "{SIZE}×{SIZE}\n{}",
            debug_map(&self.grid, |heat| match heat {
                Some(0.0) => (0, 255, 0),
                Some(heat) => {
                    let heat_norm = heat / max * u8::MAX as f32;

                    let blue = heat_norm as u8;
                    let red = u8::MAX - blue;

                    (red, 0, blue)
                }
                None => (0, 0, 0),
            })
        )
    }
}
