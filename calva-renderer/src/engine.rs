use crate::{
    AmbientLightPass, AnimationsManager, DirectionalLight, DirectionalLightPass, GeometryPass,
    InstancesManager, LightsManager, MaterialsManager, MeshesManager, PointLightsPass,
    RenderContext, Renderer, SkinsManager, Skybox, SkyboxPass, SsaoConfig, SsaoPass,
    TexturesManager,
};

pub struct EngineConfig {
    pub gamma: f32,
    pub ambient: f32,
    pub ssao: SsaoConfig,
    pub skybox: Option<Skybox>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            gamma: 2.2,
            ambient: 0.1,
            ssao: SsaoConfig::default(),
            skybox: None,
        }
    }
}

pub struct Engine {
    pub textures: TexturesManager,
    pub materials: MaterialsManager,
    pub meshes: MeshesManager,
    pub skins: SkinsManager,
    pub animations: AnimationsManager,
    pub instances: InstancesManager,
    pub lights: LightsManager,

    geometry: GeometryPass,
    ambient_light: AmbientLightPass,
    directional_light: DirectionalLightPass,
    point_lights: PointLightsPass,
    ssao: SsaoPass<640, 480>,
    skybox: SkyboxPass,

    pub config: EngineConfig,
}

impl Engine {
    pub fn new(renderer: &Renderer) -> Self {
        let textures = TexturesManager::new(&renderer.device);
        let materials = MaterialsManager::new(&renderer.device);
        let meshes = MeshesManager::new(&renderer.device);
        let skins = SkinsManager::new(&renderer.device);
        let animations = AnimationsManager::new(&renderer.device);
        let instances = InstancesManager::new(&renderer.device, &meshes);
        let lights = LightsManager::new(&renderer.device);

        let geometry = GeometryPass::new(
            &renderer,
            &textures,
            &materials,
            &skins,
            &animations,
            &instances,
        );
        let ambient_light = AmbientLightPass::new(renderer, &geometry);
        let directional_light =
            DirectionalLightPass::new(renderer, &geometry, &skins, &animations, &instances);
        let point_lights = PointLightsPass::new(renderer, &geometry);
        let ssao = SsaoPass::new(&renderer, &geometry);
        let skybox = SkyboxPass::new(&renderer);

        Self {
            textures,
            materials,
            meshes,
            instances,
            skins,
            animations,
            lights,

            geometry,
            ambient_light,
            directional_light,
            point_lights,
            ssao,
            skybox,

            config: Default::default(),
        }
    }

    pub fn resize(&mut self, renderer: &Renderer) {
        if self.geometry.size() == renderer.size() {
            return;
        }

        self.geometry.resize(&renderer);
        self.ambient_light.rebind(renderer, &self.geometry);
        self.directional_light.rebind(renderer, &self.geometry);
        self.point_lights.rebind(renderer, &self.geometry);
        self.ssao.rebind(&renderer, &self.geometry);
    }

    pub fn render(
        &self,
        ctx: &mut RenderContext,
        dt: std::time::Duration,
        directional_light: &DirectionalLight,
    ) {
        self.instances.anim(&mut ctx.encoder, &dt);

        self.geometry.render(
            ctx,
            &self.textures,
            &self.materials,
            &self.meshes,
            &self.skins,
            &self.animations,
            &self.instances,
        );
        self.ambient_light
            .render(ctx, self.config.gamma, self.config.ambient);
        self.directional_light.render(
            ctx,
            &self.meshes,
            &self.skins,
            &self.animations,
            &self.instances,
            self.config.gamma,
            directional_light,
        );
        self.point_lights
            .render(ctx, self.config.gamma, &self.lights);
        self.ssao.render(ctx, &self.config.ssao);

        if let Some(skybox) = &self.config.skybox {
            self.skybox.render(ctx, self.config.gamma, skybox);
        }
    }

    pub fn create_skybox(&self, renderer: &Renderer, pixels: &[u8]) -> Skybox {
        self.skybox.create_skybox(renderer, pixels)
    }
}
