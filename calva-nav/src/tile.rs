use core::f32;

use itertools::Itertools;
use parry3d::{
    math::Vector3,
    partitioning::{Bvh, BvhBuildStrategy},
    query::{Ray, RayCast},
    shape::Triangle,
};

pub struct NavTile<const SIZE: usize> {
    pub triangles: Vec<[glam::Vec3; 3]>,
    bvh: Bvh,
}

impl<const SIZE: usize> NavTile<SIZE> {
    pub fn new(height_map: &[[f32; SIZE]; SIZE], sample_size: f32) -> Self {
        let get_tex_height = |x: usize, y: usize| {
            let y = y.min(SIZE - 1);
            let x = x.min(SIZE - 1);

            height_map[y][x]
        };

        let tile_world_size = SIZE as f32 * sample_size;
        let min_height = -tile_world_size;

        let transform = glam::Mat4::from_translation(glam::vec3(
            -tile_world_size / 2.0,
            0.0,
            -tile_world_size / 2.0,
        ));

        let points: Vec<Vec<glam::Vec3>> = itertools::iproduct!(0..=SIZE, 0..=SIZE)
            .map(|(y, x)| {
                let it = itertools::iproduct!(0..=1, 0..=1)
                    .map(|(yy, xx)| get_tex_height(x.saturating_sub(xx), y.saturating_sub(yy)));

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
            .flat_map(|(y, x)| {
                [
                    [points[y][x], points[y + 1][x + 1], points[y][x + 1]],
                    [points[y][x], points[y + 1][x], points[y + 1][x + 1]],
                ]
            })
            .filter(|[a, b, c]| {
                let normal = glam::Vec3::cross(b - a, a - c).normalize();

                glam::Vec3::dot(normal, glam::Vec3::Y).abs() > 0.5
                    && a.y > min_height
                    && b.y > min_height
                    && c.y > min_height
            })
            .collect();

        let bvh = Bvh::from_iter(
            BvhBuildStrategy::default(),
            triangles
                .iter()
                .map(|[a, b, c]| {
                    Triangle::new(
                        Vector3::new(a.x, a.y, a.z),
                        Vector3::new(b.x, b.y, b.z),
                        Vector3::new(c.x, c.y, c.z),
                    )
                    .local_aabb()
                })
                .enumerate(),
        );

        Self { triangles, bvh }
    }

    pub fn ray_cast(&self, ro: glam::Vec3, rd: glam::Vec3) -> Option<glam::Vec3> {
        let ray = Ray::new(
            Vector3::new(ro.x, ro.y, ro.z),
            Vector3::new(rd.x, rd.y, rd.z),
        );

        self.bvh
            .cast_ray(&ray, f32::MAX, |i, _| {
                let [a, b, c] = self.triangles[i as usize];
                Triangle::new(
                    Vector3::new(a.x, a.y, a.z),
                    Vector3::new(b.x, b.y, b.z),
                    Vector3::new(c.x, c.y, c.z),
                )
                .cast_local_ray(&ray, f32::MAX, true)
            })
            .map(|(_, t)| ro + rd * t)
    }
}
