use calva::{
    egui::{egui, EguiPass},
    renderer::{AmbientPass, PointLight, ProfilerResult, Renderer, SsaoPass},
};

pub struct DemoApp {
    pub gamma: f32,
    profiler_results: Vec<ProfilerResult>,
}

impl Default for DemoApp {
    fn default() -> Self {
        Self {
            gamma: 2.2,
            profiler_results: Vec::default(),
        }
    }
}

impl DemoApp {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        renderer: &mut Renderer,
        ambient: &mut AmbientPass,
        ssao: &mut SsaoPass,
        lights: &mut [PointLight],
    ) {
        if let Some(profiler_results) = renderer.profiler() {
            self.profiler_results = profiler_results;
        }

        egui::SidePanel::right("config_panel")
            .min_width(300.0)
            .frame(egui::containers::Frame {
                inner_margin: egui::Vec2::splat(10.0).into(),
                fill: egui::Color32::from_rgba_premultiplied(0, 0, 0, 200),
                ..Default::default()
            })
            .show(ctx, |ui| {
                egui::CollapsingHeader::new("Adapter")
                    .default_open(true)
                    .show(ui, EguiPass::adapter_info_ui(&renderer.adapter_info));

                egui::CollapsingHeader::new("Profiler")
                    .default_open(true)
                    .show(ui, EguiPass::profiler_ui(&self.profiler_results));

                egui::CollapsingHeader::new("Gamma")
                    .default_open(true)
                    .show(ui, move |ui| {
                        ui.add(egui::Slider::new(&mut self.gamma, 1.0..=3.0).text("Gamma"));
                    });

                egui::CollapsingHeader::new("Ambient")
                    .default_open(true)
                    .show(ui, EguiPass::ambient_config_ui(&mut ambient.config));

                egui::CollapsingHeader::new("SSAO")
                    .default_open(true)
                    .show(ui, EguiPass::ssao_config_ui(&mut ssao.config));

                egui::CollapsingHeader::new("PointLight")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.add(egui::Slider::new(&mut lights[0].position.x, -5.0..=5.0).text("X"));
                        ui.add(egui::Slider::new(&mut lights[0].position.y, -5.0..=5.0).text("Y"));
                        ui.add(egui::Slider::new(&mut lights[0].position.z, -5.0..=5.0).text("Z"));

                        ui.add(egui::Slider::new(&mut lights[0].radius, 0.0..=30.0).text("Radius"));
                    });

                // egui::CollapsingHeader::new("ShadowLight")
                //     .default_open(true)
                //     .show(ui, |ui| {
                //         ui.add(egui::DragValue::new(&mut self.shadow_light_angle.x).speed(0.01));
                //         ui.add(egui::DragValue::new(&mut self.shadow_light_angle.z).speed(0.01));
                //     });

                // egui::CollapsingHeader::new("Animation")
                //     .default_open(true)
                //     .show(ui, |ui| {
                //         egui::ComboBox::from_label("")
                //             .selected_text(self.animation.clone())
                //             .show_ui(ui, |ui| {
                //                 for name in &self.animations {
                //                     ui.selectable_value(&mut self.animation, name.clone(), name);
                //                 }
                //             });
                //     });
            });
    }
}
