use super::{DEFAULT_LAYER_ID, Layer, RppApp, Selection, Ui, egui, make_id, property_row};

impl RppApp {
    pub(super) fn assign_to_active_layer(&mut self, item_id: &str) {
        let layer_id = if self
            .plan
            .layers
            .iter()
            .any(|layer| layer.id == self.active_layer_id && layer.visible && !layer.locked)
        {
            self.active_layer_id.clone()
        } else {
            DEFAULT_LAYER_ID.to_owned()
        };
        self.plan.assign_to_layer(item_id, &layer_id);
    }

    pub(super) fn layers_window(&mut self, ctx: &egui::Context) {
        if !self.layers_open {
            return;
        }
        let mut open = self.layers_open;
        egui::Window::new(self.localizer.text("layers.title"))
            .open(&mut open)
            .default_width(360.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(self.localizer.text("layers.add")).clicked() {
                        let before = self.snapshot();
                        let id = make_id("layer");
                        self.plan.layers.push(Layer {
                            id: id.clone(),
                            name: self.localizer.format(
                                "layers.new_name",
                                &[("number", self.plan.layers.len().to_string())],
                            ),
                            visible: true,
                            locked: false,
                            opacity: 1.0,
                        });
                        self.active_layer_id = id;
                        self.commit(before);
                    }
                    ui.label(self.localizer.text("layers.help"));
                });
                ui.separator();

                let mut move_request = None;
                let mut delete_request = None;
                let mut changed = false;
                let before_edit = self.snapshot();
                for index in 0..self.plan.layers.len() {
                    let layer_id = self.plan.layers[index].id.clone();
                    let is_default = layer_id == DEFAULT_LAYER_ID;
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.active_layer_id, layer_id.clone(), "");
                        changed |= ui
                            .checkbox(&mut self.plan.layers[index].visible, "")
                            .changed();
                        changed |= ui
                            .checkbox(&mut self.plan.layers[index].locked, "")
                            .changed();
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut self.plan.layers[index].name)
                                    .desired_width(130.0),
                            )
                            .changed();
                        let opacity = &mut self.plan.layers[index].opacity;
                        changed |= ui
                            .add(egui::Slider::new(opacity, 0.0..=1.0).show_value(false))
                            .changed();
                        if ui.small_button("↑").clicked() && index > 0 {
                            move_request = Some((index, index - 1));
                        }
                        if ui.small_button("↓").clicked() && index + 1 < self.plan.layers.len() {
                            move_request = Some((index, index + 1));
                        }
                        if !is_default && ui.small_button("×").clicked() {
                            delete_request = Some(layer_id);
                        }
                    });
                }
                if changed {
                    for layer in &mut self.plan.layers {
                        if layer.name.trim().is_empty() {
                            layer.name = self.localizer.text("layers.unnamed");
                        }
                    }
                    self.selection = None;
                    self.additional_selections.clear();
                    self.commit(before_edit);
                }
                if let Some((from, to)) = move_request {
                    let before = self.snapshot();
                    self.plan.layers.swap(from, to);
                    self.commit(before);
                }
                if let Some(layer_id) = delete_request {
                    let before = self.snapshot();
                    self.plan.layers.retain(|layer| layer.id != layer_id);
                    self.plan
                        .layer_assignments
                        .retain(|assignment| assignment.layer_id != layer_id);
                    if self.active_layer_id == layer_id {
                        self.active_layer_id = DEFAULT_LAYER_ID.to_owned();
                    }
                    self.selection = None;
                    self.additional_selections.clear();
                    self.commit(before);
                }
            });
        self.layers_open = open;
    }

    pub(super) fn selected_layer_selector(&mut self, ui: &mut Ui) {
        let Some(selection) = self.selection.clone() else {
            return;
        };
        let item_id = selection_id(&selection).to_owned();
        let mut layer_id = self.plan.layer_id_for(&item_id).to_owned();
        let original = layer_id.clone();
        property_row(ui, &self.localizer.text("property.layer"), |ui| {
            egui::ComboBox::from_id_salt("selected-layer")
                .selected_text(
                    self.plan
                        .layers
                        .iter()
                        .find(|layer| layer.id == layer_id)
                        .map_or("Default", |layer| layer.name.as_str()),
                )
                .show_ui(ui, |ui| {
                    for layer in &self.plan.layers {
                        ui.selectable_value(&mut layer_id, layer.id.clone(), &layer.name);
                    }
                })
                .response
        });
        if layer_id != original {
            let before = self.snapshot();
            self.plan.assign_to_layer(&item_id, &layer_id);
            self.commit(before);
        }
    }
}

pub(super) fn selection_id(selection: &Selection) -> &str {
    match selection {
        Selection::Wall(id)
        | Selection::Line(id)
        | Selection::Text(id)
        | Selection::Door(id)
        | Selection::Image(id)
        | Selection::Window(id)
        | Selection::Object(id)
        | Selection::Area(id)
        | Selection::Room(id)
        | Selection::Compass(id)
        | Selection::Arrow(id) => id,
    }
}
