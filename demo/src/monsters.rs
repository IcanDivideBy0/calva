use anyhow::Result;
use calva::{
    gltf::GltfModel,
    nav::HeatMap,
    renderer::{egui, wgpu, EguiRenderer, Object, Resource, ResourcesManager, Time},
};
use glam::Vec3Swizzles;
use std::collections::HashMap;

use crate::worldgen::WorldGenerator;

pub struct MonstersManager {
    resources: ResourcesManager,

    pub models: HashMap<String, GltfModel>,
    pub objects: Vec<Object>,

    target: Option<glam::Vec2>,
    heat_map: Option<HeatMap<{ WorldGenerator::HEAT_MAP_SIZE }>>,

    texture: (wgpu::Texture, egui::TextureId),
}

impl MonstersManager {
    pub fn set_target(&mut self, target: glam::Vec2) {
        let worldgen = self.resources.read::<WorldGenerator>();

        let Some(heat_map) = worldgen.get_heat_map(target) else {
            return;
        };

        self.heat_map = Some(heat_map);
        self.target = Some(target);
    }
}

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

        let device = resources.read::<wgpu::Device>();
        let mut renderer = resources.write::<EguiRenderer>();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MonstersManager heat map texture"),
            size: wgpu::Extent3d {
                width: WorldGenerator::HEAT_MAP_SIZE as _,
                height: WorldGenerator::HEAT_MAP_SIZE as _,
                ..Default::default()
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });

        let texture_id = renderer.register_native_texture(
            &device,
            &texture.create_view(&Default::default()),
            wgpu::FilterMode::Linear,
        );

        Ok(Self {
            resources: resources.clone(),

            models,
            objects: vec![],

            target: None,
            heat_map: None,
            texture: (texture, texture_id),
        })
    }

    fn update(&mut self, resources: &ResourcesManager) -> Result<()> {
        let worldgen = resources.read::<WorldGenerator>();
        let time = resources.read::<Time>();

        let Some(heat_map) = &self.heat_map else {
            return Ok(());
        };
        let Some(target) = self.target else {
            return Ok(());
        };

        let mut hm_image_data = heat_map.image_data();

        for object in &mut self.objects {
            let mut transform = object.transform();
            let (_, _, pos) = transform.to_scale_rotation_translation();

            let hm_coord = worldgen.get_heat_map_coord(pos.xz(), target);

            hm_image_data[hm_coord.y][hm_coord.x] = [0, u8::MAX, 0, u8::MAX];

            let dir = heat_map.apply_kernel(hm_coord);
            if dir == glam::Vec2::ZERO {
                continue;
            }

            let dest_height = worldgen.get_height(pos.xz() + dir).unwrap_or(pos.y);
            let dh = dest_height - pos.y;

            let dir = dir.extend(dh).xzy().normalize();

            let speed = 10.0; // units / sec
            let translation = dir * speed * time.dt.as_secs_f32();

            transform = glam::Mat4::from_translation(translation) * transform;

            object.set_transform(transform);
        }

        let (texture, _) = &self.texture;
        resources.read::<wgpu::Queue>().write_texture(
            texture.as_image_copy(),
            bytemuck::cast_slice(&hm_image_data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(texture.format().components() as u32 * texture.size().width),
                rows_per_image: None,
            },
            texture.size(),
        );

        Ok(())
    }
}

impl egui::Widget for &mut MonstersManager {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (texture, texture_id) = &self.texture;

        let offset = glam::Vec2::splat(10.0);

        let size = texture.size();
        let size = glam::vec2(size.width as _, size.height as _);

        let image = egui::Image::from_texture(egui::load::SizedTexture {
            id: *texture_id,
            size: mint::Vector2::from(size).into(),
        });

        image.paint_at(
            ui,
            egui::Rect::from_min_size(
                mint::Point2::from(offset).into(),
                mint::Vector2::from(size * 2.0).into(),
            ),
        );

        ui.response()
    }
}
