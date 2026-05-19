use anyhow::Result;
use calva::{
    gltf::GltfModel,
    nav::{HeatMap, HeightMap},
    renderer::{Object, Resource, ResourcesManager},
};
use glam::Vec3Swizzles;
use std::collections::HashMap;

use crate::worldgen::WorldGenerator;

pub struct MonstersManager {
    pub models: HashMap<String, GltfModel>,
    pub objects: Vec<Object>,
    pub target: Option<glam::Vec3>,
    pub heat_map: Option<HeatMap<{ WorldGenerator::CHUNK_SIZE * HeightMap::SIZE }>>,
}

impl MonstersManager {}

impl Resource for MonstersManager {
    fn instanciate(resources: &ResourcesManager) -> Result<Self> {
        let models = [
            "./demo/assets/zombies/zombie-boss.glb",
            "./demo/assets/zombies/zombie-common.glb",
            "./demo/assets/zombies/zombie-fat.glb",
            "./demo/assets/zombies/zombie-murderer.glb",
            "./demo/assets/zombies/zombie-snapper.glb",
            "./demo/assets/skeletons/skeleton-archer.glb",
            "./demo/assets/skeletons/skeleton-grunt.glb",
            "./demo/assets/skeletons/skeleton-mage.glb",
            "./demo/assets/skeletons/skeleton-king.glb",
            "./demo/assets/skeletons/skeleton-swordsman.glb",
            "./demo/assets/demons/demon-bomb.glb",
            "./demo/assets/demons/demon-boss.glb",
            "./demo/assets/demons/demon-fatty.glb",
            "./demo/assets/demons/demon-grunt.glb",
            "./demo/assets/demons/demon-imp.glb",
        ]
        .iter()
        // .take(1)
        .map(|filepath| {
            let model = GltfModel::from_path(resources, filepath)?;
            Ok((filepath.to_string(), model))
        })
        .collect::<Result<_>>()?;

        Ok(Self {
            models,
            objects: vec![],
            target: None,
            heat_map: None,
        })
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let worldgen = resources.read::<WorldGenerator>();

        let Some(heat_map) = &self.heat_map else {
            return Ok(());
        };

        for object in &mut self.objects {
            let mut transform = object.transform();
            let (_, _, pos) = transform.to_scale_rotation_translation();

            let Some(heat_map_coord) = worldgen.get_heat_map_coord(pos) else {
                continue;
            };

            let dir = heat_map.apply_kernel(heat_map_coord)
                * (WorldGenerator::TILE_WORLD_SIZE / HeightMap::SIZE as f32)
                / 4.0;

            let dh = worldgen
                .get_height(pos + dir.extend(0.0).xzy())
                .unwrap_or(pos.y)
                - pos.y;

            transform = glam::Mat4::from_translation(glam::vec3(dir.x, dh, dir.y)) * transform;

            object.set_transform(transform);
        }

        Ok(())
    }
}
