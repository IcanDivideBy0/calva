use calva::egui::{egui, epi};
use calva::renderer::RendererConfigData;

#[derive(Clone)]
pub struct MyApp {
    pub shadow_light_angle: glam::Vec3,
    pub ssao_radius: f32,
    pub ssao_bias: f32,
    pub ssao_power: f32,
    pub ambient_factor: f32,

    pub animations: Vec<String>,
    pub animation: String,
}

impl epi::App for MyApp {
    fn name(&self) -> &str {
        "egui template"
    }

    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        egui::SidePanel::right("config_panel")
            .min_width(300.0)
            .frame(egui::containers::Frame {
                margin: (10.0, 10.0).into(),
                fill: egui::Color32::from_rgba_premultiplied(0, 0, 0, 200),
                ..Default::default()
            })
            .show(ctx, |ui| {
                egui::CollapsingHeader::new("ShadowLight")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.add(egui::DragValue::new(&mut self.shadow_light_angle.x).speed(0.01));
                        ui.add(egui::DragValue::new(&mut self.shadow_light_angle.z).speed(0.01));
                    });

                egui::CollapsingHeader::new("SSAO")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.add(egui::Slider::new(&mut self.ssao_radius, 0.0..=4.0).text("Radius"));
                        ui.add(egui::Slider::new(&mut self.ssao_bias, 0.0..=0.1).text("Bias"));
                        ui.add(egui::Slider::new(&mut self.ssao_power, 0.0..=8.0).text("Power"));
                    });

                egui::CollapsingHeader::new("Ambient")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Slider::new(&mut self.ambient_factor, 0.0..=1.0).text("Factor"),
                        );
                    });

                egui::CollapsingHeader::new("Animation")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ComboBox::from_label("")
                            .selected_text(self.animation.clone())
                            .show_ui(ui, |ui| {
                                for name in &self.animations {
                                    ui.selectable_value(&mut self.animation, name.clone(), name);
                                }
                            });
                    });
            });
    }
}

impl From<RendererConfigData> for MyApp {
    fn from(data: RendererConfigData) -> Self {
        Self {
            shadow_light_angle: glam::vec3(0.5, -1.0, 0.0),

            ssao_radius: data.ssao_radius,
            ssao_bias: data.ssao_bias,
            ssao_power: data.ssao_power,
            ambient_factor: data.ambient_factor,

            animations: vec![],
            animation: "idle".to_owned(),
        }
    }
}

impl From<&MyApp> for RendererConfigData {
    fn from(my_app: &MyApp) -> Self {
        Self {
            ssao_radius: my_app.ssao_radius,
            ssao_bias: my_app.ssao_bias,
            ssao_power: my_app.ssao_power,
            ambient_factor: my_app.ambient_factor,
        }
    }
}
