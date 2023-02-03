use anyhow::Result;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    AmbientConfig, AmbientPass, AnimationsManager, GeometryPass, InstancesManager, LightsPass,
    MaterialsManager, MeshesManager, PointLight, RenderContext, Renderer, SkinsManager, Skybox,
    SkyboxPass, SsaoConfig, SsaoPass, TexturesManager,
};

#[derive(Debug, Copy, Clone)]
pub struct EngineConfig {
    pub gamma: f32,
    pub ambient: AmbientConfig,
    pub ssao: SsaoConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            gamma: 2.2,
            ambient: AmbientConfig::default(),
            ssao: SsaoConfig::default(),
        }
    }
}

pub struct Engine {
    pub renderer: Renderer,

    pub textures: TexturesManager,
    pub materials: MaterialsManager,
    pub meshes: MeshesManager,
    pub instances: InstancesManager,
    pub skins: SkinsManager,
    pub animations: AnimationsManager,

    geometry: GeometryPass,
    ambient: AmbientPass,
    lights: LightsPass,
    ssao: SsaoPass<640, 480>,
    skybox: SkyboxPass,

    pub config: EngineConfig,
}

impl Engine {
    pub async fn new<W: HasRawWindowHandle + HasRawDisplayHandle>(
        window: &W,
        size: (u32, u32),
    ) -> Result<Self> {
        let renderer = Renderer::new(&window, size).await?;

        let textures = TexturesManager::new(&renderer.device);
        let materials = MaterialsManager::new(&renderer.device);
        let meshes = MeshesManager::new(&renderer.device);
        let instances = InstancesManager::new(&renderer.device, &meshes);
        let skins = SkinsManager::new(&renderer.device);
        let animations = AnimationsManager::new(&renderer.device);

        let geometry = GeometryPass::new(&renderer, &textures, &materials, &skins, &animations);
        let ambient = AmbientPass::new(&renderer, &geometry);
        let lights = LightsPass::new(&renderer, &geometry);
        let ssao = SsaoPass::<640, 480>::new(&renderer, &geometry);
        let skybox = SkyboxPass::new(&renderer);

        Ok(Self {
            renderer,

            textures,
            materials,
            meshes,
            instances,
            skins,
            animations,

            geometry,
            ambient,
            lights,
            ssao,
            skybox,

            config: Default::default(),
        })
    }

    pub fn resize(&mut self, size: (u32, u32)) {
        let s = (
            self.renderer.surface_config.width,
            self.renderer.surface_config.height,
        );

        if size == s {
            return;
        }

        self.renderer.resize(size);
        self.geometry.resize(&self.renderer);
        self.ambient.resize(&self.renderer, &self.geometry);
        self.lights.resize(&self.renderer, &self.geometry);
        self.ssao.resize(&self.renderer, &self.geometry);
    }

    pub fn draw(&self, ctx: &mut RenderContext, data: &EngineRenderData) {
        self.instances.anim(&mut ctx.encoder, &data.dt);

        self.geometry.render(
            ctx,
            &self.textures,
            &self.materials,
            &self.meshes,
            &self.skins,
            &self.animations,
            &self.instances,
        );
        self.ambient
            .render(ctx, &self.config.ambient, self.config.gamma);
        self.lights
            .render(ctx, self.config.gamma, data.point_lights);
        self.ssao.render(ctx, &self.config.ssao);
        self.skybox.render(ctx, self.config.gamma, data.skybox);
    }

    pub fn render(
        &self,
        data: &EngineRenderData,
        cb: impl FnOnce(&mut RenderContext),
    ) -> Result<()> {
        self.renderer.render(|ctx| {
            self.draw(ctx, data);
            cb(ctx);
        })
    }

    pub fn create_skybox(&self, pixels: &[u8]) -> Skybox {
        self.skybox
            .create_skybox(&self.renderer.device, &self.renderer.queue, pixels)
    }
}

pub struct EngineRenderData<'a> {
    pub dt: std::time::Duration,
    pub point_lights: &'a [PointLight],
    pub skybox: &'a Skybox,
}
