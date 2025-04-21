use crate::{
    AmbientLightPass, AmbientLightPassInputs, AnimatePass, CameraManager, DirectionalLightPass,
    DirectionalLightPassInputs, FxaaPass, FxaaPassInputs, GeometryPass, HierarchicalDepthPass,
    HierarchicalDepthPassInputs, PointLightsPass, PointLightsPassInputs, RenderContext, Renderer,
    RessourcesManager, SkyboxPass, SkyboxPassInputs, SsaoPass, SsaoPassInputs, ToneMappingPass,
    ToneMappingPassInputs,
};

pub struct Engine {
    pub ressources: RessourcesManager,

    size: (u32, u32),

    pub animate: AnimatePass,
    pub geometry: GeometryPass,
    pub hierarchical_depth: HierarchicalDepthPass,
    pub ambient_light: AmbientLightPass,
    pub directional_light: DirectionalLightPass,
    pub point_lights: PointLightsPass,
    pub ssao: SsaoPass<640, 480>,
    pub skybox: SkyboxPass,
    pub fxaa: FxaaPass,
    pub tone_mapping: ToneMappingPass,
}

impl Engine {
    pub fn new(renderer: &Renderer) -> Self {
        let ressources = RessourcesManager::new(&renderer.device);

        let size = (
            renderer.surface_config.width,
            renderer.surface_config.height,
        );

        let animate = AnimatePass::new(&renderer.device, &ressources);

        let geometry = GeometryPass::new(&renderer.device, &renderer.surface_config, &ressources);

        let hierarchical_depth = HierarchicalDepthPass::new(
            &renderer.device,
            HierarchicalDepthPassInputs {
                depth: &geometry.outputs.depth,
            },
        );

        let ambient_light = AmbientLightPass::new(
            &renderer.device,
            AmbientLightPassInputs {
                albedo: &geometry.outputs.albedo_metallic,
                emissive: &geometry.outputs.emissive,
            },
        );

        let directional_light = DirectionalLightPass::new(
            &renderer.device,
            &ressources,
            DirectionalLightPassInputs {
                albedo_metallic: &geometry.outputs.albedo_metallic,
                normal_roughness: &geometry.outputs.normal_roughness,
                depth: &geometry.outputs.depth,
                output: &ambient_light.outputs.output,
            },
        );

        let point_lights = PointLightsPass::new(
            &renderer.device,
            &ressources,
            PointLightsPassInputs {
                albedo_metallic: &geometry.outputs.albedo_metallic,
                normal_roughness: &geometry.outputs.normal_roughness,
                depth: &geometry.outputs.depth,
                output: &ambient_light.outputs.output,
            },
        );

        let skybox = SkyboxPass::new(
            &renderer.device,
            &ressources,
            SkyboxPassInputs {
                depth: &geometry.outputs.depth,
                output: &ambient_light.outputs.output,
            },
        );

        let fxaa = FxaaPass::new(
            &renderer.device,
            FxaaPassInputs {
                input: &ambient_light.outputs.output,
            },
        );

        let ssao = SsaoPass::new(
            &renderer.device,
            &ressources,
            SsaoPassInputs {
                normal: &geometry.outputs.normal_roughness,
                depth: &geometry.outputs.depth,
                output: &fxaa.outputs.output,
            },
        );

        let tone_mapping = ToneMappingPass::new(
            &renderer.device,
            ToneMappingPassInputs {
                format: renderer.surface_config.format,
                input: &fxaa.outputs.output,
            },
        );

        Self {
            ressources,

            size,

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

    pub fn resize(&mut self, renderer: &Renderer) {
        let renderer_size = (
            renderer.surface_config.width,
            renderer.surface_config.height,
        );

        if self.size == renderer_size {
            return;
        }
        self.size = renderer_size;

        self.geometry
            .resize(&renderer.device, &renderer.surface_config);

        self.hierarchical_depth.rebind(
            &renderer.device,
            HierarchicalDepthPassInputs {
                depth: &self.geometry.outputs.depth,
            },
        );

        self.ambient_light.rebind(
            &renderer.device,
            AmbientLightPassInputs {
                albedo: &self.geometry.outputs.albedo_metallic,
                emissive: &self.geometry.outputs.emissive,
            },
        );

        self.directional_light.rebind(
            &renderer.device,
            DirectionalLightPassInputs {
                albedo_metallic: &self.geometry.outputs.albedo_metallic,
                normal_roughness: &self.geometry.outputs.normal_roughness,
                depth: &self.geometry.outputs.depth,
                output: &self.ambient_light.outputs.output,
            },
        );

        self.point_lights.rebind(
            &renderer.device,
            PointLightsPassInputs {
                albedo_metallic: &self.geometry.outputs.albedo_metallic,
                normal_roughness: &self.geometry.outputs.normal_roughness,
                depth: &self.geometry.outputs.depth,
                output: &self.ambient_light.outputs.output,
            },
        );

        self.skybox.rebind(SkyboxPassInputs {
            depth: &self.geometry.outputs.depth,
            output: &self.ambient_light.outputs.output,
        });

        self.fxaa.rebind(
            &renderer.device,
            FxaaPassInputs {
                input: &self.ambient_light.outputs.output,
            },
        );

        self.ssao.rebind(
            &renderer.device,
            SsaoPassInputs {
                normal: &self.geometry.outputs.normal_roughness,
                depth: &self.geometry.outputs.depth,
                output: &self.fxaa.outputs.output,
            },
        );

        self.tone_mapping.rebind(
            &renderer.device,
            ToneMappingPassInputs {
                format: renderer.surface_config.format,
                input: &self.fxaa.outputs.output,
            },
        );
    }

    pub fn update(&mut self, renderer: &Renderer) {
        self.ressources
            .get::<CameraManager>()
            .get_mut()
            .update(&renderer.queue);

        self.animate.update(&renderer.queue);
        self.directional_light.update(&renderer.queue);
        self.ambient_light.update(&renderer.queue);
        self.ssao.update(&renderer.queue);
        self.tone_mapping.update(&renderer.queue);
    }

    pub fn render(&self, ctx: &mut RenderContext) {
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
    }
}
