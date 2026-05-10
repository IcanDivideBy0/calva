use anyhow::Result;

use crate::{
    AmbientLightPass, AmbientLightPassOutputs, AnimatePass, DirectionalLightPass, FxaaPass,
    FxaaPassOutputs, GeometryPass, GeometryPassOutputs, HierarchicalDepthPass, PointLightsPass,
    RenderContext, Renderer, Resource, ResourcesManager, SkyboxPass, SsaoPass, ToneMappingPass,
};

pub struct Engine {
    pub resources: ResourcesManager,

    animate: AnimatePass,
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
    pub fn new(renderer: &Renderer) -> Self {
        let resources = renderer.resources.clone();

        let animate = AnimatePass::new(&resources);

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
            resources,

            animate,
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

    pub fn resize(&mut self) {
        // Manual update required until we remove all these rebinds
        // by moving all inputs/outputs to resources
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

    pub fn render(&self, ctx: &mut RenderContext) -> Result<()> {
        self.resources.update()?;

        self.animate.render(ctx);
        self.geometry.render(ctx);
        self.hierarchical_depth.render(ctx);
        self.ambient_light.render(ctx);
        self.directional_light.render(ctx);
        self.point_lights.render(ctx);
        self.skybox.render(ctx);
        self.fxaa.render(ctx);
        self.ssao.render(ctx);
        self.tone_mapping.render(ctx);

        Ok(())
    }
}
