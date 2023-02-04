use anyhow::Result;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    AmbientConfig, AmbientPass, AnimationsManager, GeometryPass, InstancesManager, LightingPass,
    LightsManager, MaterialsManager, MeshesManager, RenderContext, Renderer, SkinsManager, Skybox,
    SkyboxPass, SsaoConfig, SsaoPass, TexturesManager,
};

pub struct EngineConfig {
    pub gamma: f32,
    pub ambient: AmbientConfig,
    pub ssao: SsaoConfig,
    pub skybox: Option<Skybox>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            gamma: 2.2,
            ambient: AmbientConfig::default(),
            ssao: SsaoConfig::default(),
            skybox: None,
        }
    }
}

pub struct Engine {
    pub renderer: Renderer,

    pub textures: TexturesManager,
    pub materials: MaterialsManager,
    pub meshes: MeshesManager,
    pub skins: SkinsManager,
    pub animations: AnimationsManager,
    pub instances: InstancesManager,
    pub lights: LightsManager,

    geometry: GeometryPass,
    ambient: AmbientPass,
    lighting: LightingPass,
    ssao: SsaoPass<640, 480>,
    skybox: SkyboxPass,

    pub config: EngineConfig,
}

impl Engine {
    pub async fn new<W>(window: &W, size: (u32, u32)) -> Result<Self>
    where
        W: HasRawWindowHandle + HasRawDisplayHandle,
    {
        let renderer = Renderer::new(&window, size).await?;

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
        let ambient = AmbientPass::new(&renderer, &geometry);
        let lighting = LightingPass::new(&renderer, &geometry);
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
            lights,

            geometry,
            ambient,
            lighting,
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
        self.lighting.resize(&self.renderer, &self.geometry);
        self.ssao.resize(&self.renderer, &self.geometry);
    }

    pub fn render(
        &self,
        dt: std::time::Duration,
        cb: impl FnOnce(&mut RenderContext),
    ) -> Result<()> {
        self.renderer.render(|ctx| {
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
            self.ambient
                .render(ctx, &self.config.ambient, self.config.gamma);
            self.lighting.render(ctx, self.config.gamma, &self.lights);
            self.ssao.render(ctx, &self.config.ssao);

            if let Some(skybox) = &self.config.skybox {
                self.skybox.render(ctx, self.config.gamma, skybox);
            }

            cb(ctx);
        })
    }

    pub fn create_skybox(&self, pixels: &[u8]) -> Skybox {
        self.skybox
            .create_skybox(&self.renderer.device, &self.renderer.queue, pixels)
    }
}
