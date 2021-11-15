#[derive(Default)]
pub struct MyApp {
    pub value: f32,
}

impl epi::App for MyApp {
    fn name(&self) -> &str {
        "egui template"
    }

    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        egui::SidePanel::right("side_panel_right")
            .frame(egui::containers::Frame {
                margin: (10.0, 10.0).into(),
                fill: egui::Color32::from_rgba_premultiplied(0, 0, 0, 200),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.heading("Side Panel");
                ui.add(
                    egui::DragValue::new(&mut self.value)
                        .clamp_range(-1.0..=1.0)
                        .speed(0.01),
                );
            });

        // egui::SidePanel::left("side_panel_left")
        //     .frame(egui::containers::Frame {
        //         margin: (10.0, 10.0).into(),
        //         fill: egui::Color32::from_rgba_premultiplied(0, 0, 0, 200),
        //         ..Default::default()
        //     })
        //     .show(ctx, |ui| {
        //         ui.heading("Side Panel");
        //         ui.add(egui::Slider::new(&mut self.value, 0.0..=10.0).text("value"));
        //     });
    }
}
