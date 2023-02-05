use crate::{GeometryPass, LightsManager, RenderContext, Renderer};

mod ambient;
mod point_lights;

use ambient::*;
use point_lights::*;

pub struct LightingPass {
    ambient: AmbientPass,
    point_lights: PointLightsPass,
}

impl LightingPass {
    pub fn new(renderer: &Renderer, geometry: &GeometryPass) -> Self {
        Self {
            ambient: AmbientPass::new(renderer, geometry),
            point_lights: PointLightsPass::new(renderer, geometry),
        }
    }

    pub fn rebind(&mut self, renderer: &Renderer, geometry: &GeometryPass) {
        self.ambient.rebind(renderer, geometry);
        self.point_lights.rebind(renderer, geometry);
    }

    pub fn render(
        &self,
        ctx: &mut RenderContext,
        gamma: f32,
        ambient_factor: f32,
        lights: &LightsManager,
    ) {
        self.ambient.render(ctx, gamma, ambient_factor);
        self.point_lights.render(ctx, gamma, lights);
    }
}
