use crate::{
    AmbientLightConfig, AmbientLightPass, AnimationsManager, CameraManager, DirectionalLight,
    DirectionalLightPass, FxaaPass, GeometryPass, InstancesManager, LightsManager,
    MaterialsManager, MeshesManager, PointLightsPass, RenderContext, Renderer, SkinsManager,
    Skybox, SkyboxPass, SsaoConfig, SsaoPass, TexturesManager, ToneMappingConfig, ToneMappingPass,
};

#[derive(Default)]
pub struct EngineConfig {
    pub ambient: AmbientLightConfig,
    pub ssao: SsaoConfig,
    pub tone_mapping: ToneMappingConfig,
    pub skybox: Option<Skybox>,
}

pub struct Engine {
    pub camera: CameraManager,
    pub textures: TexturesManager,
    pub materials: MaterialsManager,
    pub meshes: MeshesManager,
    pub skins: SkinsManager,
    pub animations: AnimationsManager,
    pub instances: InstancesManager,
    pub lights: LightsManager,

    size: (u32, u32),

    geometry: GeometryPass,
    ambient_light: AmbientLightPass,
    directional_light: DirectionalLightPass,
    point_lights: PointLightsPass,
    ssao: SsaoPass<640, 480>,
    skybox: SkyboxPass,
    fxaa: FxaaPass,
    tone_mapping: ToneMappingPass,

    pub config: EngineConfig,
}

impl Engine {
    pub fn new(renderer: &Renderer) -> Self {
        let camera = CameraManager::new(&renderer.device);
        let textures = TexturesManager::new(&renderer.device);
        let materials = MaterialsManager::new(&renderer.device);
        let meshes = MeshesManager::new(&renderer.device);
        let skins = SkinsManager::new(&renderer.device);
        let animations = AnimationsManager::new(&renderer.device);
        let instances = InstancesManager::new(&renderer.device);
        let lights = LightsManager::new(&renderer.device);

        let size = renderer.size();

        let geometry = GeometryPass::new(
            renderer,
            &camera,
            &textures,
            &materials,
            &meshes,
            &skins,
            &animations,
            &instances,
        );
        let ambient_light = AmbientLightPass::new(renderer, &geometry);
        let directional_light = DirectionalLightPass::new(
            renderer,
            &camera,
            &meshes,
            &skins,
            &animations,
            &instances,
            &geometry,
        );
        let point_lights = PointLightsPass::new(renderer, &camera, &geometry);
        let ssao = SsaoPass::new(renderer, &camera, &geometry);
        let skybox = SkyboxPass::new(renderer, &camera);
        let fxaa = FxaaPass::new(renderer, &ambient_light.output);
        let tone_mapping = ToneMappingPass::new(renderer, &fxaa.output);

        Self {
            camera,
            textures,
            materials,
            meshes,
            instances,
            skins,
            animations,
            lights,

            size,
            geometry,
            ambient_light,
            directional_light,
            point_lights,
            ssao,
            skybox,
            fxaa,
            tone_mapping,

            config: Default::default(),
        }
    }

    pub fn resize(&mut self, renderer: &Renderer) {
        if self.size == renderer.size() {
            return;
        }

        self.geometry.resize(renderer);
        self.ambient_light.rebind(renderer, &self.geometry);
        self.directional_light.rebind(renderer, &self.geometry);
        self.point_lights.rebind(renderer, &self.geometry);
        self.ssao.rebind(renderer, &self.geometry);
        self.fxaa.rebind(renderer, &self.ambient_light.output);
        self.tone_mapping.rebind(renderer, &self.fxaa.output);

        self.size = renderer.size();
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        view: glam::Mat4,
        proj: glam::Mat4,
        directional_light: &DirectionalLight,
    ) {
        self.camera.update(&renderer.queue, view, proj);

        self.directional_light
            .update(renderer, &self.camera, directional_light);
        self.ssao.update(renderer, &self.config.ssao);
    }

    pub fn render(&self, ctx: &mut RenderContext, dt: std::time::Duration) {
        self.instances.anim(ctx, &dt);

        self.geometry.render(
            ctx,
            &self.camera,
            &self.textures,
            &self.materials,
            &self.meshes,
            &self.skins,
            &self.animations,
            &self.instances,
        );
        self.ambient_light.render(ctx, &self.config.ambient);
        // self.directional_light.render(
        //     ctx,
        //     &self.ambient_light.output,
        //     &self.camera,
        //     &self.meshes,
        //     &self.skins,
        //     &self.animations,
        //     &self.instances,
        // );
        self.point_lights
            .render(ctx, &self.ambient_light.output, &self.camera, &self.lights);
        self.ssao
            .render(ctx, &self.ambient_light.output, &self.camera);

        if let Some(skybox) = &self.config.skybox {
            self.skybox
                .render(ctx, &self.ambient_light.output, &self.camera, skybox);
        }

        self.fxaa.render(ctx);

        self.tone_mapping
            .render(ctx, &self.config.tone_mapping, &ctx.frame);
    }

    pub fn create_skybox(&self, renderer: &Renderer, pixels: &[u8]) -> Skybox {
        self.skybox.create_skybox(renderer, pixels)
    }
}
