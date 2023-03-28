use crate::{
    AmbientLightConfig, AmbientLightPass, AmbientLightPassInputs, AnimationsManager, CameraManager,
    DirectionalLight, DirectionalLightPass, DirectionalLightPassInputs, FxaaPass, FxaaPassInputs,
    GeometryPass, HierarchicalDepthPass, HierarchicalDepthPassInputs, InstancesManager,
    LightsManager, MaterialsManager, MeshesManager, PointLightsPass, PointLightsPassInputs,
    RenderContext, Renderer, SkinsManager, Skybox, SkyboxPass, SkyboxPassInputs, SsaoConfig,
    SsaoPass, SsaoPassInputs, TexturesManager, ToneMappingConfig, ToneMappingPass,
    ToneMappingPassInputs,
};

#[derive(Default)]
pub struct EngineConfig {
    pub ambient: AmbientLightConfig,
    pub ssao: SsaoConfig,
    pub tone_mapping: ToneMappingConfig,
    pub skybox: Option<Skybox>,
}

pub struct EngineRessources {
    pub camera: CameraManager,
    pub textures: TexturesManager,
    pub materials: MaterialsManager,
    pub meshes: MeshesManager,
    pub skins: SkinsManager,
    pub animations: AnimationsManager,
    pub instances: InstancesManager,
    pub lights: LightsManager,
}

pub struct Engine {
    pub ressources: EngineRessources,

    size: (u32, u32),

    pub geometry: GeometryPass,
    pub hierarchical_depth: HierarchicalDepthPass,
    pub ambient_light: AmbientLightPass,
    pub directional_light: DirectionalLightPass,
    pub point_lights: PointLightsPass,
    pub ssao: SsaoPass<640, 480>,
    pub skybox: SkyboxPass,
    pub fxaa: FxaaPass,
    pub tone_mapping: ToneMappingPass,

    pub config: EngineConfig,
}

impl Engine {
    pub fn new(renderer: &Renderer) -> Self {
        let ressources = EngineRessources {
            camera: CameraManager::new(&renderer.device),
            textures: TexturesManager::new(&renderer.device),
            materials: MaterialsManager::new(&renderer.device),
            meshes: MeshesManager::new(&renderer.device),
            skins: SkinsManager::new(&renderer.device),
            animations: AnimationsManager::new(&renderer.device),
            instances: InstancesManager::new(&renderer.device),
            lights: LightsManager::new(&renderer.device),
        };

        let size = renderer.size();

        let geometry = GeometryPass::new(
            &renderer.device,
            &renderer.surface_config,
            &ressources.camera,
            &ressources.textures,
            &ressources.materials,
            &ressources.meshes,
            &ressources.skins,
            &ressources.animations,
            &ressources.instances,
        );

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
            &ressources.camera,
            &ressources.meshes,
            &ressources.skins,
            &ressources.animations,
            &ressources.instances,
            DirectionalLightPassInputs {
                albedo_metallic: &geometry.outputs.albedo_metallic,
                normal_roughness: &geometry.outputs.normal_roughness,
                depth: &geometry.outputs.depth,
                output: &ambient_light.outputs.output,
            },
        );

        let point_lights = PointLightsPass::new(
            &renderer.device,
            &ressources.camera,
            PointLightsPassInputs {
                albedo_metallic: &geometry.outputs.albedo_metallic,
                normal_roughness: &geometry.outputs.normal_roughness,
                depth: &geometry.outputs.depth,
                output: &ambient_light.outputs.output,
            },
        );

        let skybox = SkyboxPass::new(
            &renderer.device,
            &ressources.camera,
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
            &ressources.camera,
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

            geometry,
            hierarchical_depth,
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

        self.size = renderer.size();
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        view: glam::Mat4,
        proj: glam::Mat4,
        directional_light: &DirectionalLight,
    ) {
        self.ressources.camera.update(&renderer.queue, view, proj);

        self.directional_light
            .update(&renderer.queue, &self.ressources.camera, directional_light);

        self.ssao.update(&renderer.queue, &self.config.ssao);
    }

    pub fn render(&self, ctx: &mut RenderContext, dt: std::time::Duration) {
        self.ressources.instances.anim(ctx, &dt);

        self.geometry.render(
            ctx,
            &self.ressources.camera,
            &self.ressources.textures,
            &self.ressources.materials,
            &self.ressources.meshes,
            &self.ressources.skins,
            &self.ressources.animations,
            &self.ressources.instances,
        );

        self.hierarchical_depth.render(ctx);

        self.ambient_light.render(ctx, &self.config.ambient);

        // self.directional_light.render(
        //     ctx,
        //     &self.camera,
        //     &self.meshes,
        //     &self.skins,
        //     &self.animations,
        //     &self.instances,
        // );

        self.point_lights
            .render(ctx, &self.ressources.camera, &self.ressources.lights);

        if let Some(skybox) = &self.config.skybox {
            self.skybox.render(ctx, &self.ressources.camera, skybox);
        }

        self.fxaa.render(ctx);

        self.ssao.render(ctx, &self.ressources.camera);

        self.tone_mapping
            .render(ctx, &self.config.tone_mapping, ctx.frame);
    }

    pub fn create_skybox(&self, renderer: &Renderer, pixels: &[u8]) -> Skybox {
        self.skybox
            .create_skybox(&renderer.device, &renderer.queue, pixels)
    }
}
