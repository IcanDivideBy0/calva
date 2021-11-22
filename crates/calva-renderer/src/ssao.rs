use crate::GeometryBuffer;
use crate::RenderContext;
use crate::Renderer;

pub struct SsaoPass {}

impl SsaoPass {
    pub fn new(_renderer: &Renderer, _gbuffer: &GeometryBuffer) -> Self {
        let _noise = {
            let mut pixels = Vec::with_capacity(16 * 3);
            for _ in 0..16 {
                pixels.extend_from_slice(&[
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>() * 2.0 - 1.0,
                    0.0,
                ]);
            }

            // let texture = gl.create_texture().map_err(Error::msg)?;
            // gl.bind_texture(glow::TEXTURE_2D, Some(texture));

            // gl.tex_image_2d(
            //     glow::TEXTURE_2D,                       // target
            //     0,                                      // level
            //     glow::RGB32F as i32,                    // internal_format
            //     4,                                      // width
            //     4,                                      // height
            //     0,                                      // border
            //     glow::RGB,                              // format
            //     glow::FLOAT,                            // ty
            //     Some(utils::f32_slice_as_buf(&pixels)), // pixels
            // );

            // gl.tex_parameter_i32(
            //     glow::TEXTURE_2D,
            //     glow::TEXTURE_MIN_FILTER,
            //     glow::NEAREST as i32,
            // );
            // gl.tex_parameter_i32(
            //     glow::TEXTURE_2D,
            //     glow::TEXTURE_MAG_FILTER,
            //     glow::NEAREST as i32,
            // );

            // gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            // gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);

            // gl.bind_texture(glow::TEXTURE_2D, None);

            // texture
        };

        Self {}
    }

    pub fn render(&self, _ctx: &mut RenderContext) {}
}
