use std::hash::Hash;

use calva::{gltf::GltfModel, renderer::Instance};
use glam::Vec3Swizzles;
use rand::prelude::*;
use rand_seeder::SipHasher;

#[allow(dead_code)]
pub struct Dungen {
    noise: noise::ScalePoint<noise::ScaleBias<f64, noise::Value, 2>>,
}

impl Dungen {
    #[allow(dead_code)]
    pub fn new(seed: impl Hash) -> Self {
        let mut rng = SipHasher::from(seed).into_rng();
        let noise = noise::ScalePoint::new(
            noise::ScaleBias::new(noise::Value::new(rng.gen()))
                .set_scale(0.5)
                .set_bias(0.5),
        )
        .set_scale(0.325);

        Self { noise }
    }

    #[allow(dead_code)]
    pub fn chunk(&self, coord: glam::IVec2) -> Chunk {
        Chunk::new(coord, &self.noise)
    }
}

pub struct Chunk {
    pub cells: [[Cell; Self::SIZE]; Self::SIZE],
}

impl Chunk {
    const SIZE: usize = 16;

    pub fn new(coord: glam::IVec2, noise: &impl noise::NoiseFn<f64, 2>) -> Self {
        let offset = coord * (Self::SIZE as i32);

        let cells = std::array::from_fn(|x| {
            std::array::from_fn(|y| {
                let cell_coord = glam::ivec2(x as i32, y as i32);
                Cell::new(offset + cell_coord, noise)
            })
        });

        Self { cells }
    }

    pub fn meshes(&self) -> impl Iterator<Item = &(&'static str, glam::Mat4)> + '_ {
        self.cells.iter().flatten().flat_map(|cell| &cell.meshes)
    }

    pub fn instanciate<'data: 'it, 'it>(
        &'data self,
        model: &'data GltfModel,
    ) -> impl Iterator<Item = Instance> + 'it {
        self.meshes().flat_map(|(name, transform)| {
            let (instances, _) = model
                .node_instances(name, Some(*transform), None)
                .unwrap_or_default();

            instances
        })
    }
}

pub struct Cell {
    nodes: Vec<(&'static str, glam::Mat4)>,
}

impl Cell {
    pub fn new(coord: glam::IVec2, noise: &impl noise::NoiseFn<f64, 2>) -> Self {
        Self {
            nodes: std::iter::empty()
                .chain(Self::populate_floor(coord, noise))
                .collect(),
        }
    }

    fn floor_level(coord: glam::IVec2, noise: impl noise::NoiseFn<f64, 2>) -> i32 {
        match (noise.get([coord.x as f64, coord.y as f64]) * 8.0) as u32 {
            6.. => 2,
            4.. => 1,
            1.. => 0,
            _ => -1,
        }
    }

    pub fn populate_floor(
        coord: glam::IVec2,
        noise: impl noise::NoiseFn<f64, 2>,
    ) -> impl IntoIterator<Item = (&'static str, glam::Mat4)> {
        let floor_level = Self::floor_level(coord, noise);

        let nodes = match floor_level {
            -1 => return vec![],
            _ => vec![(
                "Floor_Plane_01_32",
                glam::Mat4::from_scale_rotation_translation(
                    glam::Vec3::splat(0.01),
                    glam::Quat::IDENTITY,
                    (6 * coord).extend(floor_level * 2).as_vec3().xzy(),
                ),
            )],
        };

        if Self::floor_level(coord + glam::IVec2::X, noise) < floor_level {}
    }
}
