use crate::{light::DirectionalLight, PointLight, RenderContext, Renderer};

pub mod ambient;
pub mod geometry;
pub mod point_lights;
pub mod shadow;
pub mod skybox;
pub mod ssao;

pub use ambient::Ambient;
pub use geometry::Geometry;
pub use point_lights::PointLights;
pub use shadow::ShadowLight;
pub use skybox::Skybox;
pub use ssao::Ssao;

pub struct DefaultGraph {
    pub gbuffer: Geometry,
    pub skybox: Skybox,
    pub ambient: Ambient,
    pub shadows: ShadowLight,
    pub point_lights: PointLights,
    pub ssao: Ssao,
}

impl DefaultGraph {
    pub fn new(renderer: &Renderer, skybox: (u32, &[u8])) -> Self {
        let gbuffer = Geometry::new(renderer);
        let skybox = Skybox::new(renderer, skybox.0, skybox.1);
        let ambient = Ambient::new(renderer, &gbuffer.albedo_metallic);
        let shadows = ShadowLight::new(
            renderer,
            &gbuffer.albedo_metallic,
            &gbuffer.normal_roughness,
            &gbuffer.depth,
        );
        let point_lights = PointLights::new(
            renderer,
            &gbuffer.albedo_metallic,
            &gbuffer.normal_roughness,
            &gbuffer.depth,
        );
        let ssao = Ssao::new(renderer, &gbuffer.normal_roughness, &gbuffer.depth);

        Self {
            gbuffer,
            skybox,
            ambient,
            shadows,
            point_lights,
            ssao,
        }
    }

    pub fn render<'s: 'ctx, 'ctx, 'data: 'ctx>(
        &'s self,
        ctx: &'ctx mut RenderContext,

        draw_geometry: impl FnOnce(&mut dyn FnMut(geometry::DrawCallArgs<'data>)),
        draw_shadows: impl FnOnce(&mut dyn FnMut(shadow::DrawCallArgs<'data>)),
        light: &DirectionalLight,
        splits: [f32; 4],
        lights: &[PointLight],
    ) {
        self.gbuffer.render(ctx, draw_geometry);

        self.skybox.render(ctx);
        self.ambient.render(ctx);
        self.shadows.render(ctx, splits, light, draw_shadows);

        self.point_lights.render(ctx, lights);
        self.ssao.render(ctx);
    }
}
