use anyhow::Result;

use crate::{
    AmbientLightPass, AmbientLightPassInputs, AnimatePass, DirectionalLightPass,
    DirectionalLightPassInputs, FxaaPass, FxaaPassInputs, GeometryPass, GeometryPassOutputs,
    HierarchicalDepthPass, PointLightsPass, PointLightsPassInputs, RenderContext, Renderer,
    Resource, ResourcesManager, SkyboxPass, SkyboxPassInputs, SsaoPass, SsaoPassInputs,
    ToneMappingPass, ToneMappingPassInputs,
};

pub struct Engine {
    pub resources: ResourcesManager,

    animate: AnimatePass,
    pub geometry: GeometryPass,
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

        let geometry_outputs = resources.read::<GeometryPassOutputs>();

        let ambient_light = AmbientLightPass::new(
            &resources,
            AmbientLightPassInputs {
                albedo: &geometry_outputs.albedo_metallic,
                emissive: &geometry_outputs.emissive,
            },
        );

        let directional_light = DirectionalLightPass::new(
            &resources,
            DirectionalLightPassInputs {
                albedo_metallic: &geometry_outputs.albedo_metallic,
                normal_roughness: &geometry_outputs.normal_roughness,
                depth: &geometry_outputs.depth,
                output: &ambient_light.outputs.output,
            },
        );

        let point_lights = PointLightsPass::new(
            &resources,
            PointLightsPassInputs {
                albedo_metallic: &geometry_outputs.albedo_metallic,
                normal_roughness: &geometry_outputs.normal_roughness,
                depth: &geometry_outputs.depth,
                output: &ambient_light.outputs.output,
            },
        );

        let skybox = SkyboxPass::new(
            &resources,
            SkyboxPassInputs {
                depth: &geometry_outputs.depth,
                output: &ambient_light.outputs.output,
            },
        );

        let fxaa = FxaaPass::new(
            &resources,
            FxaaPassInputs {
                input: &ambient_light.outputs.output,
            },
        );

        let ssao = SsaoPass::new(
            &resources,
            SsaoPassInputs {
                normal: &geometry_outputs.normal_roughness,
                depth: &geometry_outputs.depth,
                output: &fxaa.outputs.output,
            },
        );

        let tone_mapping = ToneMappingPass::new(
            &resources,
            ToneMappingPassInputs {
                format: renderer.surface_config.format,
                input: &fxaa.outputs.output,
            },
        );

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

    pub fn resize(&mut self, renderer: &Renderer) {
        // Manual update required until we remove all these rebinds
        // by moving all inputs/outmuts to resources
        self.resources
            .write::<GeometryPassOutputs>()
            .update(&self.resources)
            .unwrap();

        let geometry_outputs = self.resources.read::<GeometryPassOutputs>();

        self.hierarchical_depth.rebind();

        self.ambient_light.rebind(AmbientLightPassInputs {
            albedo: &geometry_outputs.albedo_metallic,
            emissive: &geometry_outputs.emissive,
        });

        self.directional_light.rebind(DirectionalLightPassInputs {
            albedo_metallic: &geometry_outputs.albedo_metallic,
            normal_roughness: &geometry_outputs.normal_roughness,
            depth: &geometry_outputs.depth,
            output: &self.ambient_light.outputs.output,
        });

        self.point_lights.rebind(PointLightsPassInputs {
            albedo_metallic: &geometry_outputs.albedo_metallic,
            normal_roughness: &geometry_outputs.normal_roughness,
            depth: &geometry_outputs.depth,
            output: &self.ambient_light.outputs.output,
        });

        self.skybox.rebind(SkyboxPassInputs {
            depth: &geometry_outputs.depth,
            output: &self.ambient_light.outputs.output,
        });

        self.fxaa.rebind(FxaaPassInputs {
            input: &self.ambient_light.outputs.output,
        });

        self.ssao.rebind(SsaoPassInputs {
            normal: &geometry_outputs.normal_roughness,
            depth: &geometry_outputs.depth,
            output: &self.fxaa.outputs.output,
        });

        self.tone_mapping.rebind(ToneMappingPassInputs {
            format: renderer.surface_config.format,
            input: &self.fxaa.outputs.output,
        });
    }

    pub fn update(&mut self) -> Result<()> {
        self.resources.update()
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
