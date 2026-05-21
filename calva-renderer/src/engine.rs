use anyhow::Result;

use crate::{
    AmbientLightPass, AmbientLightPassOutputs, DirectionalLightPass, FxaaPass, FxaaPassOutputs,
    GeometryPass, GeometryPassOutputs, HierarchicalDepthPass, PointLightsPass, RenderContext,
    Renderer, Resource, ResourcesManager, SkyboxPass, SsaoPass, ToneMappingPass,
};

pub struct Engine {
    pub resources: ResourcesManager,

    renderer: Renderer,

    geometry: GeometryPass,
    hierarchical_depth: HierarchicalDepthPass,
    ambient_light: AmbientLightPass,
    directional_light: DirectionalLightPass,
    point_lights: PointLightsPass,
    ssao: SsaoPass<640, 480>,
    skybox: SkyboxPass,
    fxaa: FxaaPass,
    tone_mapping: ToneMappingPass,
}

impl Engine {
    pub fn new(renderer: Renderer) -> Self {
        let resources = renderer.resources.clone();

        let geometry = GeometryPass::new(&resources);
        let hierarchical_depth = HierarchicalDepthPass::new(&resources);
        let ambient_light = AmbientLightPass::new(&resources);
        let directional_light = DirectionalLightPass::new(&resources);
        let point_lights = PointLightsPass::new(&resources);
        let skybox = SkyboxPass::new(&resources);
        let fxaa = FxaaPass::new(&resources);
        let ssao = SsaoPass::new(&resources);
        let tone_mapping = ToneMappingPass::new(&resources);

        Self {
            renderer,
            resources,

            geometry,
            hierarchical_depth,
            ambient_light,
            directional_light,
            point_lights,
            ssao,
            skybox,
            fxaa,
            tone_mapping,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let mut surface_config = self.resources.write::<wgpu::SurfaceConfiguration>();

        surface_config.width = width;
        surface_config.height = height;

        drop(surface_config);

        // Manual update required until we remove all these rebinds
        // and implement update dependencies in resources manager
        self.resources
            .write::<wgpu::Surface>()
            .update(&self.resources)
            .unwrap();
        self.resources
            .write::<GeometryPassOutputs>()
            .update(&self.resources)
            .unwrap();
        self.resources
            .write::<AmbientLightPassOutputs>()
            .update(&self.resources)
            .unwrap();
        self.resources
            .write::<FxaaPassOutputs>()
            .update(&self.resources)
            .unwrap();

        self.hierarchical_depth.rebind();
        self.ambient_light.rebind();
        self.directional_light.rebind();
        self.point_lights.rebind();
        self.fxaa.rebind();
        self.ssao.rebind();
        self.tone_mapping.rebind();
    }

    pub fn render(&self, cb: impl FnOnce(&mut RenderContext) -> Result<()>) -> Result<()> {
        self.resources.update()?;

        self.renderer.render(|ctx| -> Result<()> {
            self.geometry.render(ctx);
            self.hierarchical_depth.render(ctx);
            self.ambient_light.render(ctx);
            self.directional_light.render(ctx);
            self.point_lights.render(ctx);
            self.skybox.render(ctx);
            self.fxaa.render(ctx);
            self.ssao.render(ctx);
            self.tone_mapping.render(ctx);

            cb(ctx)?;

            Ok(())
        })
    }
}

#[cfg(feature = "egui")]
impl egui::Widget for &mut Engine {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::Panel::right("engine_panel")
            .min_size(460.0)
            .frame(egui::containers::Frame {
                inner_margin: egui::Vec2::splat(10.0).into(),
                fill: egui::Color32::from_black_alpha(200),
                ..Default::default()
            })
            .show_inside(ui, |ui| {
                use crate::{AmbientLightConfig, DirectionalLight, SsaoConfig, ToneMappingConfig};

                ui.add(&self.renderer);

                let resources = &self.resources;

                ui.add(&mut *resources.write::<AmbientLightConfig>());
                ui.add(&mut *resources.write::<SsaoConfig>());
                ui.add(&mut *resources.write::<ToneMappingConfig>());
                ui.add(&mut *resources.write::<DirectionalLight>());
            })
            .response
    }
}
