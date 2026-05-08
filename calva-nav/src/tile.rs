use colored::{Colorize, CustomColor};
use core::{f32, fmt};
use std::collections::VecDeque;

use itertools::Itertools;
use parry3d::{
    math::Vector3,
    partitioning::{Bvh, BvhBuildStrategy},
    query::{Ray, RayCast},
    shape::Triangle,
};

pub struct NavTile<const SIZE: usize> {
    pub grid: [[Option<f32>; SIZE]; SIZE],
    pub triangles: Vec<[glam::Vec3; 3]>,
    bvh: Bvh,
}

impl<const SIZE: usize> NavTile<SIZE> {
    pub fn new(height_map: &[[f32; SIZE]; SIZE], sample_size: f32) -> Self {
        let get_height = |x: usize, y: usize| {
            let y = y.min(SIZE - 1);
            let x = x.min(SIZE - 1);

            height_map[y][x]
        };

        let tile_world_size = SIZE as f32 * sample_size;
        let min_height = -tile_world_size;

        let mut grid = [[None; SIZE]; SIZE];
        for (y, x) in itertools::iproduct!(0..SIZE, 0..SIZE) {
            let height = get_height(x, y);

            if height <= min_height {
                continue;
            }

            let valid_neighbours = itertools::iproduct!(
                y.saturating_sub(1)..=y.saturating_add(1),
                x.saturating_sub(1)..=x.saturating_add(1),
            )
            .all(|(yy, xx)| {
                let dist = if xx != x && yy != y {
                    f32::consts::SQRT_2
                } else {
                    1.0
                };

                // This 2x factor on threshold is not logic to me, but omitting it
                // produce different results than the triangles list where we're
                // checking the dot product of the normal and the up axis.
                (height - get_height(xx, yy)).abs() < dist * sample_size * 2.0
            });
            if !valid_neighbours {
                continue;
            }

            grid[y][x] = Some(height);
        }

        let transform = glam::Mat4::from_translation(glam::vec3(
            -tile_world_size / 2.0,
            0.0,
            -tile_world_size / 2.0,
        ));

        let points: Vec<Vec<glam::Vec3>> = itertools::iproduct!(0..=SIZE, 0..=SIZE)
            .map(|(y, x)| {
                let it = itertools::iproduct!(0..=1, 0..=1,)
                    .map(|(yy, xx)| get_height(x.saturating_sub(xx), y.saturating_sub(yy)));

                let min = it.clone().fold(f32::INFINITY, f32::min);

                let height = if min > min_height {
                    it.fold(0.0, std::ops::Add::add) / 4.0
                } else {
                    min_height
                };

                transform.transform_point3(glam::vec3(
                    x as f32 * sample_size,
                    height, // engine is Y-up
                    y as f32 * sample_size,
                ))
            })
            .chunks(SIZE + 1)
            .into_iter()
            .map(std::iter::Iterator::collect)
            .collect();

        let triangles: Vec<_> = itertools::iproduct!(0..SIZE, 0..SIZE)
            .map(|(y, x)| {
                [
                    [points[y][x], points[y + 1][x + 1], points[y][x + 1]],
                    [points[y][x], points[y + 1][x], points[y + 1][x + 1]],
                ]
            })
            .filter(|[t1, t2]| {
                let check_triangle = |[a, b, c]: &[glam::Vec3; 3]| -> bool {
                    let normal = glam::Vec3::cross(b - a, a - c).normalize();

                    glam::Vec3::dot(normal, glam::Vec3::Y).abs() > f32::consts::SQRT_2 / 2.0
                        && a.y > min_height
                        && b.y > min_height
                        && c.y > min_height
                };

                check_triangle(t1) && check_triangle(t2)
            })
            .flatten()
            .collect();

        let bvh = Bvh::from_iter(
            BvhBuildStrategy::default(),
            triangles
                .iter()
                .map(|[a, b, c]| {
                    Triangle::new(
                        Vector3::from_array(a.to_array()),
                        Vector3::from_array(b.to_array()),
                        Vector3::from_array(c.to_array()),
                    )
                    .local_aabb()
                })
                .enumerate(),
        );

        Self {
            grid,
            triangles,
            bvh,
        }
    }

    pub fn ray_cast(&self, ro: glam::Vec3, rd: glam::Vec3) -> Option<glam::Vec3> {
        let ray = Ray::new(
            Vector3::from_array(ro.to_array()),
            Vector3::from_array(rd.to_array()),
        );

        self.bvh
            .cast_ray(&ray, f32::MAX, |i, _| {
                let [a, b, c] = self.triangles[i as usize];
                Triangle::new(
                    Vector3::from_array(a.to_array()),
                    Vector3::from_array(b.to_array()),
                    Vector3::from_array(c.to_array()),
                )
                .cast_local_ray(&ray, f32::MAX, true)
            })
            .map(|(_, t)| ro + rd * t)
    }
}

impl<const SIZE: usize> fmt::Debug for NavTile<SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max = self
            .grid
            .iter()
            .flatten()
            .filter_map(|h| *h)
            .fold(f32::MIN, f32::max);

        write!(
            f,
            "\n{}",
            debug_map(&self.grid, |height| match height {
                Some(height) => {
                    let height_norm = (height / max + 0.5) * (u8::MAX / 2) as f32;
                    let value = height_norm as u8;

                    (value, value, value)
                }
                None => (0, 0, 0),
            })
        )
    }
}

pub struct FlowField<const SIZE: usize> {
    pub heat_map: [[Option<f32>; SIZE]; SIZE],
}

impl<const SIZE: usize> FlowField<SIZE> {
    pub fn new(nav_tile: &NavTile<SIZE>, target: glam::USizeVec2) -> Self {
        let mut heat_map = [[None; SIZE]; SIZE];
        heat_map[target.y][target.x] = Some(0.0);

        let mut open_list = VecDeque::from([target]);

        while let Some(head) = open_list.pop_front() {
            for (y, x) in itertools::iproduct!(
                head.y.saturating_sub(1)..=head.y.saturating_add(1).min(SIZE - 1),
                head.x.saturating_sub(1)..=head.x.saturating_add(1).min(SIZE - 1),
            ) {
                if nav_tile.grid[y][x].is_none() {
                    continue;
                }

                let mut dist = if head.x == x || head.y == y {
                    1.0
                } else {
                    f32::consts::SQRT_2
                };
                dist += heat_map[head.y][head.x].unwrap_or_default();

                if let Some(ref mut d) = heat_map[y][x] {
                    *d = d.min(dist)
                } else {
                    heat_map[y][x] = Some(dist);
                    open_list.push_back(glam::usizevec2(x, y));
                }
            }
        }

        Self { heat_map }
    }
}

impl<const SIZE: usize> fmt::Debug for FlowField<SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max = self
            .heat_map
            .iter()
            .flatten()
            .filter_map(|h| *h)
            .fold(f32::MIN, f32::max);

        write!(
            f,
            "\n{}",
            debug_map(&self.heat_map, |heat| match heat {
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

fn debug_map<T, const W: usize, const H: usize, C: Into<CustomColor>>(
    map: &[[T; W]; H],
    color: impl Fn(&T) -> C,
) -> String {
    let (row_pairs, rest) = map.as_chunks::<2>();

    row_pairs
        .iter()
        .map(|[row_up, row_down]| {
            std::iter::zip(row_up, row_down)
                .map(|(up, down)| {
                    "▀"
                        .custom_color(color(up))
                        .on_custom_color(color(down))
                        .to_string()
                })
                .collect::<String>()
        })
        .chain(rest.iter().map(|row| {
            row.iter()
                .map(|cell| "▀".custom_color(color(cell)).to_string())
                .collect::<String>()
        }))
        .join("\n")
}
