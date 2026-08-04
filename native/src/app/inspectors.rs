use super::*;

fn room_kind_key(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Custom => "room.type_custom",
        RoomKind::LivingRoom => "room.type_living_room",
        RoomKind::Bedroom => "room.type_bedroom",
        RoomKind::Kitchen => "room.type_kitchen",
        RoomKind::Bathroom => "room.type_bathroom",
        RoomKind::Hallway => "room.type_hallway",
        RoomKind::Office => "room.type_office",
        RoomKind::Garage => "room.type_garage",
        RoomKind::Storage => "room.type_storage",
    }
}

impl RppApp {
    pub(super) fn inspector(&mut self, root: &mut Ui) {
        Panel::right("inspector")
            .exact_size(220.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(self.secondary_panel_fill())
                    .inner_margin(Margin::same(15)),
            )
            .show(root, |ui| {
                ui.heading(egui::RichText::new(self.localizer.text("panel.properties")).size(14.0));
                ui.add_space(14.0);
                if !self.additional_selections.is_empty() {
                    ui.label(
                        egui::RichText::new(self.localizer.format(
                            "panel.multiple_selection",
                            &[("count", (self.additional_selections.len() + 1).to_string())],
                        ))
                        .size(12.0)
                        .color(ACCENT),
                    );
                    ui.add_space(8.0);
                    let mut common_color = [52, 56, 59];
                    if property_row(ui, &self.localizer.text("property.color"), |ui| {
                        ui.color_edit_button_srgb(&mut common_color)
                    })
                    .changed()
                    {
                        self.set_selected_color(common_color);
                    }
                    ui.horizontal(|ui| {
                        if ui.button(self.localizer.text("action.lock_all")).clicked() {
                            self.set_selected_locked(true);
                        }
                        if ui.button(self.localizer.text("action.unlock_all")).clicked() {
                            self.set_selected_locked(false);
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(self.localizer.text("panel.align"));
                    ui.horizontal(|ui| {
                        if ui
                            .small_button(self.localizer.text("action.align_left"))
                            .clicked()
                        {
                            self.align_selected(Alignment::Left);
                        }
                        if ui
                            .small_button(self.localizer.text("action.align_center"))
                            .clicked()
                        {
                            self.align_selected(Alignment::HorizontalCenter);
                        }
                        if ui
                            .small_button(self.localizer.text("action.align_right"))
                            .clicked()
                        {
                            self.align_selected(Alignment::Right);
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .small_button(self.localizer.text("action.align_top"))
                            .clicked()
                        {
                            self.align_selected(Alignment::Top);
                        }
                        if ui
                            .small_button(self.localizer.text("action.align_middle"))
                            .clicked()
                        {
                            self.align_selected(Alignment::VerticalCenter);
                        }
                        if ui
                            .small_button(self.localizer.text("action.align_bottom"))
                            .clicked()
                        {
                            self.align_selected(Alignment::Bottom);
                        }
                    });
                    ui.add_space(8.0);
                }

                if self.selection.is_some() && self.additional_selections.is_empty() {
                    self.selected_layer_selector(ui);
                }

                match self.selection.clone() {
                    Some(Selection::Wall(id)) => self.wall_inspector(ui, &id),
                    Some(Selection::Line(id)) => self.line_inspector(ui, &id),
                    Some(Selection::Text(id)) => self.text_inspector(ui, &id),
                    Some(Selection::Door(id)) => self.door_inspector(ui, &id),
                    Some(Selection::Image(id)) => self.image_inspector(ui, &id),
                    Some(Selection::Window(id)) => self.window_inspector(ui, &id),
                    Some(Selection::Object(id)) => self.object_inspector(ui, &id),
                    Some(Selection::Area(id)) => self.area_inspector(ui, &id),
                    Some(Selection::Room(id)) => self.room_inspector(ui, &id),
                    Some(Selection::Compass(id)) => self.compass_inspector(ui, &id),
                    Some(Selection::Arrow(id)) => self.arrow_inspector(ui, &id),
                    None => {
                        ui.label(
                            egui::RichText::new(self.localizer.text("panel.empty_selection"))
                                .size(12.0)
                                .color(Color32::from_gray(120)),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new(self.localizer.text("panel.display"))
                                .strong()
                                .size(12.0)
                                .color(ACCENT),
                        );
                        let mut settings_changed =
                            property_row(ui, &self.localizer.text("property.grid"), |ui| {
                                ui.checkbox(&mut self.grid_visible, "")
                            })
                            .changed();
                        settings_changed |=
                            property_row(ui, &self.localizer.text("property.snap"), |ui| {
                                ui.checkbox(&mut self.snap_enabled, "")
                            })
                            .changed();
                        let original_unit = self.unit;
                        property_row(ui, &self.localizer.text("property.unit"), |ui| {
                            egui::ComboBox::from_id_salt("unit")
                                .selected_text(self.unit.as_str())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.unit, Unit::Mm, "mm");
                                    ui.selectable_value(&mut self.unit, Unit::Cm, "cm");
                                    ui.selectable_value(&mut self.unit, Unit::M, "m");
                                    ui.selectable_value(&mut self.unit, Unit::In, "in");
                                    ui.selectable_value(&mut self.unit, Unit::Ft, "ft");
                                })
                                .response
                        });
                        settings_changed |= self.unit != original_unit;
                        let mut spacing = self.unit.from_centimeters(self.grid_size);
                        let spacing_response =
                            property_row(ui, &self.localizer.text("property.spacing"), |ui| {
                                ui.add(
                                    egui::DragValue::new(&mut spacing)
                                        .range(0.01..=50_000.0)
                                        .speed(1.0)
                                        .suffix(self.unit.suffix()),
                                )
                            });
                        if spacing_response.changed() {
                            self.grid_size = self.unit.to_centimeters(spacing).clamp(0.1, 5000.0);
                            settings_changed = true;
                        }
                        ui.collapsing(self.localizer.text("panel.defaults"), |ui| {
                            ui.label(self.localizer.text("default.wall_thickness"));
                            settings_changed |= distance_drag(
                                ui,
                                &mut self.default_wall_thickness,
                                self.unit,
                                0.1,
                                500.0,
                            )
                            .changed();
                            ui.label(self.localizer.text("default.door_width"));
                            settings_changed |= distance_drag(
                                ui,
                                &mut self.default_door_width,
                                self.unit,
                                10.0,
                                1000.0,
                            )
                            .changed();
                            ui.label(self.localizer.text("default.window_width"));
                            settings_changed |= distance_drag(
                                ui,
                                &mut self.default_window_width,
                                self.unit,
                                10.0,
                                1000.0,
                            )
                            .changed();
                            ui.label(self.localizer.text("default.text_size"));
                            settings_changed |= distance_drag(
                                ui,
                                &mut self.default_text_size,
                                self.unit,
                                1.0,
                                500.0,
                            )
                            .changed();
                        });
                        if settings_changed {
                            self.persist_settings();
                        }
                        ui.add_space(18.0);
                        ui.label(
                            egui::RichText::new(self.localizer.text("panel.image_library"))
                                .strong()
                                .size(12.0)
                                .color(ACCENT),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if let Some(texture) = self.textures.get(BUILTIN_EMERGENCY_EXIT_ID) {
                                ui.image((texture.id(), Vec2::splat(40.0)));
                            }
                            ui.vertical(|ui| {
                                ui.label(self.localizer.text("library.emergency_exit"));
                                ui.label(
                                    egui::RichText::new("40 × 40 cm · built-in")
                                        .size(10.0)
                                        .color(self.muted_text_color()),
                                );
                                if ui
                                    .small_button(self.localizer.text("action.place_image"))
                                    .clicked()
                                {
                                    self.reuse_asset(BUILTIN_EMERGENCY_EXIT_ID);
                                }
                            });
                        });
                        ui.separator();
                        if ui
                            .button(self.localizer.text("action.import_image"))
                            .clicked()
                        {
                            self.import_image();
                        }
                        let assets: Vec<(String, String, String, usize, u32, u32)> = self
                            .assets
                            .iter()
                            .filter_map(|asset| {
                                decode_image(&asset.bytes).ok().map(|(_, width, height)| {
                                    (
                                        asset.id.clone(),
                                        asset.name.clone(),
                                        asset.extension.clone(),
                                        asset.bytes.len(),
                                        width,
                                        height,
                                    )
                                })
                            })
                            .collect();
                        for (asset_id, name, extension, bytes, width, height) in assets {
                            ui.horizontal(|ui| {
                                if let Some(texture) = self.textures.get(&asset_id) {
                                    let ratio = width as f32 / height.max(1) as f32;
                                    let size = if ratio >= 1.0 {
                                        Vec2::new(48.0, 48.0 / ratio)
                                    } else {
                                        Vec2::new(48.0 * ratio, 48.0)
                                    };
                                    ui.image((texture.id(), size));
                                }
                                ui.vertical(|ui| {
                                    ui.label(&name).on_hover_text(&name);
                                    let (placed_width, placed_height) =
                                        default_image_dimensions(width, height);
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{extension} · {width}×{height}px · {:.1} KiB\n{:.1} × {:.1} cm",
                                            bytes as f32 / 1024.0,
                                            placed_width,
                                            placed_height
                                        ))
                                        .size(9.5)
                                        .color(self.muted_text_color()),
                                    );
                                    ui.horizontal(|ui| {
                                        if ui
                                            .small_button("×")
                                            .on_hover_text(
                                                self.localizer.text("action.remove_asset"),
                                            )
                                            .clicked()
                                        {
                                            self.remove_asset(&asset_id);
                                        }
                                        if ui
                                            .small_button(
                                                self.localizer.text("action.place_image"),
                                            )
                                            .clicked()
                                        {
                                            self.reuse_asset(&asset_id);
                                        }
                                    });
                                });
                            });
                        }
                    }
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(self.localizer.text("hint.undo"))
                            .size(11.0)
                            .color(self.muted_text_color()),
                    );
                    ui.label(
                        egui::RichText::new(self.localizer.text("hint.pan"))
                            .size(11.0)
                            .color(self.muted_text_color()),
                    );
                    ui.label(
                        egui::RichText::new(self.localizer.text("hint.zoom"))
                            .size(11.0)
                            .color(self.muted_text_color()),
                    );
                    ui.label(
                        egui::RichText::new(self.localizer.text("panel.navigation"))
                            .strong()
                            .size(11.0)
                            .color(Color32::from_gray(80)),
                    );
                });
            });
    }

    fn line_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.lines.iter().position(|line| line.id == id) else {
            self.selection = None;
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.line"))
                .strong()
                .color(ACCENT),
        );
        let length = self.plan.lines[index].length();
        property_row(ui, &self.localizer.text("property.length"), |ui| {
            ui.label(format_distance(length, self.unit))
        });
        let mut color = self.plan.lines[index].color;
        let response = property_row(ui, &self.localizer.text("property.color"), |ui| {
            ui.color_edit_button_srgb(&mut color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.lines[index].color = color;
            self.commit(before);
        }
        let mut locked = self.plan.lines[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.lines[index].locked = locked;
            self.commit(before);
        }
        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(self.localizer.text("action.delete_line"))
                        .size(12.0)
                        .color(Color32::from_rgb(160, 50, 50)),
                )
                .frame(false),
            )
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn text_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.texts.iter().position(|text| text.id == id) else {
            self.selection = None;
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.text"))
                .strong()
                .color(ACCENT),
        );
        let mut content = self.plan.texts[index].text.clone();
        let response = ui.text_edit_multiline(&mut content);
        if response.changed() {
            let before = self.snapshot();
            self.plan.texts[index].text = content;
            self.commit(before);
        }
        let mut x = self.plan.texts[index].x;
        let response = property_row(ui, &self.localizer.text("property.x"), |ui| {
            distance_drag(ui, &mut x, self.unit, -1_000_000.0, 1_000_000.0)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.texts[index].x = x;
            self.commit(before);
        }
        let mut y = self.plan.texts[index].y;
        let response = property_row(ui, &self.localizer.text("property.y"), |ui| {
            distance_drag(ui, &mut y, self.unit, -1_000_000.0, 1_000_000.0)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.texts[index].y = y;
            self.commit(before);
        }
        let mut size = self.plan.texts[index].size;
        let response = property_row(ui, &self.localizer.text("property.text_size"), |ui| {
            distance_drag(ui, &mut size, self.unit, 1.0, 500.0)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.texts[index].size = size;
            self.commit(before);
        }
        let mut rotation = self.plan.texts[index].rotation;
        let response = property_row(ui, &self.localizer.text("property.rotation"), |ui| {
            ui.add(egui::DragValue::new(&mut rotation).speed(1.0).suffix("°"))
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.texts[index].rotation = rotation.rem_euclid(360.0);
            self.commit(before);
        }
        let mut color = self.plan.texts[index].color;
        let response = property_row(ui, &self.localizer.text("property.color"), |ui| {
            ui.color_edit_button_srgb(&mut color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.texts[index].color = color;
            self.commit(before);
        }
        property_row(ui, &self.localizer.text("property.font"), |ui| {
            ui.label("Arial")
        });
        let mut bold = self.plan.texts[index].bold;
        let response = property_row(ui, &self.localizer.text("property.bold"), |ui| {
            ui.checkbox(&mut bold, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.texts[index].bold = bold;
            self.commit(before);
        }
        let mut locked = self.plan.texts[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.texts[index].locked = locked;
            self.commit(before);
        }
        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(self.localizer.text("action.delete_text"))
                        .size(12.0)
                        .color(Color32::from_rgb(160, 50, 50)),
                )
                .frame(false),
            )
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn area_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.areas.iter().position(|area| area.id == id) else {
            self.selection = None;
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.area"))
                .strong()
                .color(ACCENT),
        );
        property_row(ui, &self.localizer.text("property.area"), |ui| {
            ui.label(format_area(
                self.plan.areas[index].area_square_centimeters(),
                self.unit,
            ))
        });
        let mut fill = self.plan.areas[index].fill_color;
        let response = property_row(ui, &self.localizer.text("property.fill_color"), |ui| {
            ui.color_edit_button_srgb(&mut fill)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.areas[index].fill_color = fill;
            self.default_area_fill_color = fill;
            self.persist_settings();
            self.commit(before);
        }
        let mut stroke = self.plan.areas[index].stroke_color;
        let response = property_row(ui, &self.localizer.text("property.stroke_color"), |ui| {
            ui.color_edit_button_srgb(&mut stroke)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.areas[index].stroke_color = stroke;
            self.commit(before);
        }
        let mut locked = self.plan.areas[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.areas[index].locked = locked;
            self.commit(before);
        }
        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .button(self.localizer.text("action.delete_area"))
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn room_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.rooms.iter().position(|room| room.id == id) else {
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.room"))
                .size(12.0)
                .color(ACCENT),
        );
        let source_key = match self.plan.rooms[index].source {
            RoomSource::Automatic => "room.source_automatic",
            RoomSource::Manual => "room.source_manual",
        };
        property_row(ui, &self.localizer.text("property.source"), |ui| {
            ui.label(self.localizer.text(source_key))
        });

        let mut name = self.plan.rooms[index].name.clone();
        if property_row(ui, &self.localizer.text("property.name"), |ui| {
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0))
        })
        .changed()
        {
            let before = self.snapshot();
            self.plan.rooms[index].name = name;
            self.commit(before);
        }

        let mut kind = self.plan.rooms[index].kind;
        property_row(ui, &self.localizer.text("property.type"), |ui| {
            egui::ComboBox::from_id_salt("room-kind")
                .selected_text(self.localizer.text(room_kind_key(kind)))
                .show_ui(ui, |ui| {
                    for value in [
                        RoomKind::Custom,
                        RoomKind::LivingRoom,
                        RoomKind::Bedroom,
                        RoomKind::Kitchen,
                        RoomKind::Bathroom,
                        RoomKind::Hallway,
                        RoomKind::Office,
                        RoomKind::Garage,
                        RoomKind::Storage,
                    ] {
                        ui.selectable_value(
                            &mut kind,
                            value,
                            self.localizer.text(room_kind_key(value)),
                        );
                    }
                })
                .response
        });
        if kind != self.plan.rooms[index].kind {
            let before = self.snapshot();
            self.plan.rooms[index].kind = kind;
            self.commit(before);
        }

        property_row(ui, &self.localizer.text("property.area"), |ui| {
            ui.label(format_area(
                self.plan.rooms[index].area_square_centimeters(),
                self.unit,
            ))
        });
        property_row(ui, &self.localizer.text("property.perimeter"), |ui| {
            ui.label(format_distance(
                self.plan.rooms[index].perimeter_centimeters(),
                self.unit,
            ))
        });

        let mut fill = self.plan.rooms[index].fill_color;
        if property_row(ui, &self.localizer.text("property.fill_color"), |ui| {
            ui.color_edit_button_srgb(&mut fill)
        })
        .changed()
        {
            let before = self.snapshot();
            self.plan.rooms[index].fill_color = fill;
            self.commit(before);
        }
        let mut visible = self.plan.rooms[index].fill_visible;
        if property_row(ui, &self.localizer.text("property.fill_visible"), |ui| {
            ui.checkbox(&mut visible, "")
        })
        .changed()
        {
            let before = self.snapshot();
            self.plan.rooms[index].fill_visible = visible;
            self.commit(before);
        }
        let mut locked = self.plan.rooms[index].locked;
        if property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        })
        .changed()
        {
            let before = self.snapshot();
            self.plan.rooms[index].locked = locked;
            self.commit(before);
        }
        if self.plan.rooms[index].source == RoomSource::Manual
            && ui
                .button(self.localizer.text("action.delete_room"))
                .clicked()
        {
            let before = self.snapshot();
            let room_id = self.plan.rooms[index].id.clone();
            self.plan.rooms.remove(index);
            self.plan.remove_layer_assignment(&room_id);
            self.selection = None;
            self.commit(before);
            self.status = self.localizer.text("status.room_deleted");
        }
    }

    fn compass_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self
            .plan
            .compasses
            .iter()
            .position(|compass| compass.id == id)
        else {
            self.selection = None;
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.compass"))
                .strong()
                .color(ACCENT),
        );
        for (label, property) in [
            ("property.x", 0_u8),
            ("property.y", 1),
            ("property.size", 2),
            ("property.rotation", 3),
        ] {
            let mut value = match property {
                0 => self.plan.compasses[index].x,
                1 => self.plan.compasses[index].y,
                2 => self.plan.compasses[index].size,
                _ => self.plan.compasses[index].rotation,
            };
            let response = property_row(ui, &self.localizer.text(label), |ui| {
                if property == 3 {
                    ui.add(egui::DragValue::new(&mut value).speed(1.0).suffix("°"))
                } else {
                    distance_drag(ui, &mut value, self.unit, -1_000_000.0, 1_000_000.0)
                }
            });
            if response.changed() {
                let before = self.snapshot();
                match property {
                    0 => self.plan.compasses[index].x = value,
                    1 => self.plan.compasses[index].y = value,
                    2 => self.plan.compasses[index].size = value.max(1.0),
                    _ => self.plan.compasses[index].rotation = value.rem_euclid(360.0),
                }
                self.commit(before);
            }
        }
        let mut color = self.plan.compasses[index].color;
        let response = property_row(ui, &self.localizer.text("property.color"), |ui| {
            ui.color_edit_button_srgb(&mut color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.compasses[index].color = color;
            self.commit(before);
        }
        let mut locked = self.plan.compasses[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.compasses[index].locked = locked;
            self.commit(before);
        }
        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .button(self.localizer.text("action.delete_compass"))
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn wall_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.walls.iter().position(|wall| wall.id == id) else {
            self.selection = None;
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.wall"))
                .strong()
                .size(12.0)
                .color(ACCENT),
        );
        ui.add_space(10.0);

        let current_length = self.plan.walls[index].length();
        let mut length = current_length;
        let response = property_row(ui, &self.localizer.text("property.length"), |ui| {
            distance_drag(ui, &mut length, self.unit, MIN_WALL, 100_000.0)
        });
        if response.changed() && current_length > 0.0 {
            let before = self.snapshot();
            let wall = &mut self.plan.walls[index];
            let ratio = length / current_length;
            wall.x2 = wall.x1 + (wall.x2 - wall.x1) * ratio;
            wall.y2 = wall.y1 + (wall.y2 - wall.y1) * ratio;
            self.repair_openings_for_wall(id);
            self.commit(before);
        }

        let wall_snapshot = self.plan.walls[index].clone();
        let mut angle = (wall_snapshot.y2 - wall_snapshot.y1)
            .atan2(wall_snapshot.x2 - wall_snapshot.x1)
            .to_degrees();
        let response = property_row(ui, &self.localizer.text("property.angle"), |ui| {
            ui.add(
                egui::DragValue::new(&mut angle)
                    .range(-180.0..=180.0)
                    .speed(0.5)
                    .suffix("°"),
            )
        });
        if response.changed() {
            let before = self.snapshot();
            let radians = angle.to_radians();
            let wall = &mut self.plan.walls[index];
            wall.x2 = wall.x1 + current_length * radians.cos();
            wall.y2 = wall.y1 + current_length * radians.sin();
            self.repair_openings_for_wall(id);
            self.commit(before);
        }

        for (label_key, endpoint) in [
            ("property.start_x", 0_u8),
            ("property.start_y", 1),
            ("property.end_x", 2),
            ("property.end_y", 3),
        ] {
            let mut value = match endpoint {
                0 => self.plan.walls[index].x1,
                1 => self.plan.walls[index].y1,
                2 => self.plan.walls[index].x2,
                _ => self.plan.walls[index].y2,
            };
            let response = property_row(ui, &self.localizer.text(label_key), |ui| {
                distance_drag(ui, &mut value, self.unit, -1_000_000.0, 1_000_000.0)
            });
            if response.changed() && value.is_finite() {
                let before = self.snapshot();
                let wall = &mut self.plan.walls[index];
                match endpoint {
                    0 => wall.x1 = value,
                    1 => wall.y1 = value,
                    2 => wall.x2 = value,
                    _ => wall.y2 = value,
                }
                self.repair_openings_for_wall(id);
                self.commit(before);
            }
        }

        let mut thickness = self.plan.walls[index].thickness;
        let response = property_row(ui, &self.localizer.text("property.thickness"), |ui| {
            distance_drag(ui, &mut thickness, self.unit, 0.1, 200.0)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.walls[index].thickness = thickness;
            self.commit(before);
        }

        let original_kind = self.plan.walls[index].kind;
        let mut kind = original_kind;
        let interior = self.localizer.text("wall.interior");
        let exterior = self.localizer.text("wall.exterior");
        property_row(ui, &self.localizer.text("property.type"), |ui| {
            egui::ComboBox::from_id_salt(("wall-kind", id))
                .selected_text(match kind {
                    WallKind::Interior => &interior,
                    WallKind::Exterior => &exterior,
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut kind, WallKind::Interior, &interior);
                    ui.selectable_value(&mut kind, WallKind::Exterior, &exterior);
                })
                .response
        });
        if kind != original_kind {
            let before = self.snapshot();
            self.plan.walls[index].kind = kind;
            self.commit(before);
        }

        let mut color = self.plan.walls[index].color;
        let response = property_row(ui, &self.localizer.text("property.color"), |ui| {
            ui.color_edit_button_srgb(&mut color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.walls[index].color = color;
            self.default_wall_color = color;
            self.persist_settings();
            self.commit(before);
        }
        let mut locked = self.plan.walls[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.walls[index].locked = locked;
            self.commit(before);
        }

        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .button(self.localizer.text("action.split_middle"))
            .clicked()
        {
            self.split_selected_wall();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(self.localizer.text("action.delete_wall"))
                        .size(12.0)
                        .color(Color32::from_rgb(160, 50, 50)),
                )
                .frame(false),
            )
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn door_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.doors.iter().position(|door| door.id == id) else {
            self.selection = None;
            return;
        };
        let wall_id = self.plan.doors[index].wall_id.clone();
        let wall_length = self
            .plan
            .walls
            .iter()
            .find(|wall| wall.id == wall_id)
            .map(Wall::length)
            .unwrap_or(0.0);
        ui.label(
            egui::RichText::new(self.localizer.text("selection.door"))
                .strong()
                .size(12.0)
                .color(ACCENT),
        );
        ui.add_space(10.0);

        let mut width = self.plan.doors[index].width;
        let response = property_row(ui, &self.localizer.text("property.width"), |ui| {
            distance_drag(ui, &mut width, self.unit, 20.0, wall_length.max(20.0))
        });
        if response.changed() && wall_length > 0.0 {
            width = width.min(wall_length);
            let margin = width / (2.0 * wall_length);
            let position = self.plan.doors[index].t.clamp(margin, 1.0 - margin);
            if self.opening_available(&wall_id, position, width, Some(id)) {
                let before = self.snapshot();
                self.plan.doors[index].width = width;
                self.plan.doors[index].t = position;
                self.commit(before);
            } else {
                self.status = self.localizer.text("status.opening_overlap");
            }
        }

        let mut position = self.plan.doors[index].t * 100.0;
        let margin = if wall_length > 0.0 {
            self.plan.doors[index].width / (2.0 * wall_length) * 100.0
        } else {
            0.0
        };
        let response = property_row(ui, &self.localizer.text("property.position"), |ui| {
            ui.add(
                egui::DragValue::new(&mut position)
                    .range(margin..=(100.0 - margin).max(margin))
                    .speed(0.5)
                    .suffix(" %"),
            )
        });
        if response.changed() {
            let candidate = (position / 100.0).clamp(0.0, 1.0);
            if self.opening_available(&wall_id, candidate, self.plan.doors[index].width, Some(id)) {
                let before = self.snapshot();
                self.plan.doors[index].t = candidate;
                self.commit(before);
            } else {
                self.status = self.localizer.text("status.opening_overlap");
            }
        }

        let original_kind = self.plan.doors[index].kind;
        let mut kind = original_kind;
        let single = self.localizer.text("door.single");
        let double = self.localizer.text("door.double");
        let sliding = self.localizer.text("door.sliding");
        property_row(ui, &self.localizer.text("property.type"), |ui| {
            egui::ComboBox::from_id_salt(("door-kind", id))
                .selected_text(match kind {
                    DoorKind::Single => &single,
                    DoorKind::Double => &double,
                    DoorKind::Sliding => &sliding,
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut kind, DoorKind::Single, &single);
                    ui.selectable_value(&mut kind, DoorKind::Double, &double);
                    ui.selectable_value(&mut kind, DoorKind::Sliding, &sliding);
                })
                .response
        });
        if kind != original_kind {
            let before = self.snapshot();
            self.plan.doors[index].kind = kind;
            self.commit(before);
        }

        let original_hinge = self.plan.doors[index].hinge;
        let mut hinge = original_hinge;
        let left = self.localizer.text("side.left");
        let right = self.localizer.text("side.right");
        property_row(ui, &self.localizer.text("property.hinge"), |ui| {
            egui::ComboBox::from_id_salt(("door-hinge", id))
                .selected_text(match hinge {
                    HingeSide::Left => &left,
                    HingeSide::Right => &right,
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut hinge, HingeSide::Left, &left);
                    ui.selectable_value(&mut hinge, HingeSide::Right, &right);
                })
                .response
        });
        if hinge != original_hinge {
            let before = self.snapshot();
            self.plan.doors[index].hinge = hinge;
            self.commit(before);
        }

        let original_swing = self.plan.doors[index].swing;
        let mut swing = original_swing;
        let inward = self.localizer.text("swing.inward");
        let outward = self.localizer.text("swing.outward");
        property_row(ui, &self.localizer.text("property.swing"), |ui| {
            egui::ComboBox::from_id_salt(("door-swing", id))
                .selected_text(match swing {
                    SwingSide::Inward => &inward,
                    SwingSide::Outward => &outward,
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut swing, SwingSide::Inward, &inward);
                    ui.selectable_value(&mut swing, SwingSide::Outward, &outward);
                })
                .response
        });
        if swing != original_swing {
            let before = self.snapshot();
            self.plan.doors[index].swing = swing;
            self.commit(before);
        }

        let mut color = self.plan.doors[index].color;
        let response = property_row(ui, &self.localizer.text("property.color"), |ui| {
            ui.color_edit_button_srgb(&mut color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.doors[index].color = color;
            self.commit(before);
        }
        let mut locked = self.plan.doors[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.doors[index].locked = locked;
            self.commit(before);
        }

        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(self.localizer.text("action.delete_door"))
                        .size(12.0)
                        .color(Color32::from_rgb(160, 50, 50)),
                )
                .frame(false),
            )
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn window_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.windows.iter().position(|window| window.id == id) else {
            self.selection = None;
            return;
        };
        let wall_id = self.plan.windows[index].wall_id.clone();
        let wall_length = self
            .plan
            .walls
            .iter()
            .find(|wall| wall.id == wall_id)
            .map(Wall::length)
            .unwrap_or(0.0);
        ui.label(
            egui::RichText::new(self.localizer.text("selection.window"))
                .strong()
                .size(12.0)
                .color(ACCENT),
        );
        ui.add_space(10.0);

        let mut width = self.plan.windows[index].width;
        let response = property_row(ui, &self.localizer.text("property.width"), |ui| {
            distance_drag(ui, &mut width, self.unit, 20.0, wall_length.max(20.0))
        });
        if response.changed() && wall_length > 0.0 {
            let margin = width / (2.0 * wall_length);
            let position = self.plan.windows[index].t.clamp(margin, 1.0 - margin);
            if self.opening_available(&wall_id, position, width, Some(id)) {
                let before = self.snapshot();
                self.plan.windows[index].width = width;
                self.plan.windows[index].t = position;
                self.commit(before);
            } else {
                self.status = self.localizer.text("status.opening_overlap");
            }
        }

        let mut position = self.plan.windows[index].t * 100.0;
        let margin = if wall_length > 0.0 {
            self.plan.windows[index].width / (2.0 * wall_length) * 100.0
        } else {
            0.0
        };
        let response = property_row(ui, &self.localizer.text("property.position"), |ui| {
            ui.add(
                egui::DragValue::new(&mut position)
                    .range(margin..=(100.0 - margin).max(margin))
                    .speed(0.5)
                    .suffix(" %"),
            )
        });
        if response.changed() {
            let candidate = (position / 100.0).clamp(0.0, 1.0);
            if self.opening_available(
                &wall_id,
                candidate,
                self.plan.windows[index].width,
                Some(id),
            ) {
                let before = self.snapshot();
                self.plan.windows[index].t = candidate;
                self.commit(before);
            } else {
                self.status = self.localizer.text("status.opening_overlap");
            }
        }

        let original_kind = self.plan.windows[index].kind;
        let mut kind = original_kind;
        let standard = self.localizer.text("window.standard");
        let bay = self.localizer.text("window.bay");
        property_row(ui, &self.localizer.text("property.type"), |ui| {
            egui::ComboBox::from_id_salt(("window-kind", id))
                .selected_text(match kind {
                    WindowKind::Standard => &standard,
                    WindowKind::Bay => &bay,
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut kind, WindowKind::Standard, &standard);
                    ui.selectable_value(&mut kind, WindowKind::Bay, &bay);
                })
                .response
        });
        if kind != original_kind {
            let before = self.snapshot();
            self.plan.windows[index].kind = kind;
            self.commit(before);
        }

        let mut color = self.plan.windows[index].color;
        let response = property_row(ui, &self.localizer.text("property.color"), |ui| {
            ui.color_edit_button_srgb(&mut color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.windows[index].color = color;
            self.commit(before);
        }
        let mut locked = self.plan.windows[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.windows[index].locked = locked;
            self.commit(before);
        }

        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(self.localizer.text("action.delete_window"))
                        .size(12.0)
                        .color(Color32::from_rgb(160, 50, 50)),
                )
                .frame(false),
            )
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn object_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.objects.iter().position(|object| object.id == id) else {
            self.selection = None;
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.object"))
                .strong()
                .size(12.0)
                .color(ACCENT),
        );
        ui.add_space(10.0);

        let mut name = self.plan.objects[index].name.clone();
        let response = property_row(ui, &self.localizer.text("property.name"), |ui| {
            ui.text_edit_singleline(&mut name)
        });
        if response.changed() && name.chars().count() <= 512 {
            let before = self.snapshot();
            self.plan.objects[index].name = name;
            self.commit(before);
        }

        let original_kind = self.plan.objects[index].kind;
        let mut kind = original_kind;
        let kinds = [
            (
                ObjectKind::Rectangle,
                self.localizer.text("object.rectangle"),
            ),
            (ObjectKind::Bed, self.localizer.text("object.bed")),
            (ObjectKind::Table, self.localizer.text("object.table")),
            (ObjectKind::Sofa, self.localizer.text("object.sofa")),
            (ObjectKind::Wardrobe, self.localizer.text("object.wardrobe")),
            (ObjectKind::Desk, self.localizer.text("object.desk")),
        ];
        let selected_kind = kinds
            .iter()
            .find(|(value, _)| *value == kind)
            .map(|(_, label)| label.as_str())
            .unwrap_or("Rectangle");
        property_row(ui, &self.localizer.text("property.type"), |ui| {
            egui::ComboBox::from_id_salt(("object-kind", id))
                .selected_text(selected_kind)
                .show_ui(ui, |ui| {
                    for (value, label) in &kinds {
                        ui.selectable_value(&mut kind, *value, label);
                    }
                })
                .response
        });
        if kind != original_kind {
            let before = self.snapshot();
            self.plan.objects[index].kind = kind;
            self.commit(before);
        }

        for (label_key, property) in [
            ("property.x", 0_u8),
            ("property.y", 1),
            ("property.width", 2),
            ("property.height", 3),
            ("property.rotation", 4),
        ] {
            let mut value = match property {
                0 => self.plan.objects[index].x,
                1 => self.plan.objects[index].y,
                2 => self.plan.objects[index].width,
                3 => self.plan.objects[index].height,
                _ => self.plan.objects[index].rotation,
            };
            let response = property_row(ui, &self.localizer.text(label_key), |ui| {
                if property == 4 {
                    ui.add(
                        egui::DragValue::new(&mut value)
                            .speed(0.5)
                            .range(-180.0..=180.0)
                            .suffix("°"),
                    )
                } else {
                    distance_drag(
                        ui,
                        &mut value,
                        self.unit,
                        if property == 2 || property == 3 {
                            1.0
                        } else {
                            -1_000_000.0
                        },
                        1_000_000.0,
                    )
                }
            });
            if response.changed() && value.is_finite() {
                let before = self.snapshot();
                let object = &mut self.plan.objects[index];
                match property {
                    0 => object.x = value,
                    1 => object.y = value,
                    2 => object.width = value,
                    3 => object.height = value,
                    _ => object.rotation = value,
                }
                self.commit(before);
            }
        }

        let mut fill_color = self.plan.objects[index].fill_color;
        let response = property_row(ui, &self.localizer.text("property.fill_color"), |ui| {
            ui.color_edit_button_srgb(&mut fill_color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.objects[index].fill_color = fill_color;
            self.commit(before);
        }
        let mut stroke_color = self.plan.objects[index].stroke_color;
        let response = property_row(ui, &self.localizer.text("property.stroke_color"), |ui| {
            ui.color_edit_button_srgb(&mut stroke_color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.objects[index].stroke_color = stroke_color;
            self.commit(before);
        }

        let mut locked = self.plan.objects[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.objects[index].locked = locked;
            self.commit(before);
        }

        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(self.localizer.text("action.delete_object"))
                        .size(12.0)
                        .color(Color32::from_rgb(160, 50, 50)),
                )
                .frame(false),
            )
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn arrow_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.arrows.iter().position(|arrow| arrow.id == id) else {
            self.selection = None;
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.arrow"))
                .strong()
                .size(12.0)
                .color(ACCENT),
        );
        ui.add_space(10.0);

        for (key, is_length) in [("property.length", true), ("property.width", false)] {
            let mut value = if is_length {
                self.plan.arrows[index].length
            } else {
                self.plan.arrows[index].width
            };
            let response = property_row(ui, &self.localizer.text(key), |ui| {
                distance_drag(ui, &mut value, self.unit, 1.0, 100_000.0)
            });
            if response.changed() && value.is_finite() {
                let before = self.snapshot();
                if is_length {
                    self.plan.arrows[index].length = value;
                } else {
                    self.plan.arrows[index].width = value;
                }
                self.commit(before);
            }
        }

        let original_style = self.plan.arrows[index].style;
        let mut style = original_style;
        property_row(ui, &self.localizer.text("property.style"), |ui| {
            egui::ComboBox::from_id_salt(("arrow-style", id))
                .selected_text(
                    self.localizer
                        .text(&format!("arrow_style.{}", style.as_str())),
                )
                .show_ui(ui, |ui| {
                    for value in [
                        ArrowStyle::Open,
                        ArrowStyle::OpenBold,
                        ArrowStyle::Hollow,
                        ArrowStyle::Solid,
                    ] {
                        ui.selectable_value(
                            &mut style,
                            value,
                            self.localizer
                                .text(&format!("arrow_style.{}", value.as_str())),
                        );
                    }
                })
                .response
        });
        if style != original_style {
            let before = self.snapshot();
            self.plan.arrows[index].style = style;
            self.commit(before);
        }

        let mut orientation = self.plan.arrows[index].orientation as i32;
        let orientation_suffix = format!(" / 63  ({:.3}°)", orientation as f32 * 5.625);
        let response = property_row(ui, &self.localizer.text("property.orientation"), |ui| {
            ui.add(
                egui::DragValue::new(&mut orientation)
                    .range(0..=63)
                    .suffix(&orientation_suffix),
            )
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.arrows[index].orientation = orientation.clamp(0, 63) as u8;
            self.commit(before);
        }
        let (dial_rect, dial_response) =
            ui.allocate_exact_size(Vec2::splat(112.0), Sense::click_and_drag());
        let center = dial_rect.center();
        let radius = 46.0;
        ui.painter().circle(
            center,
            radius,
            ui.visuals().faint_bg_color,
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
        for step in 0..64 {
            let angle = step as f32 * std::f32::consts::TAU / 64.0;
            let outer = center + Vec2::angled(angle) * radius;
            let inner = center
                + Vec2::angled(angle)
                    * if step % 8 == 0 {
                        radius - 7.0
                    } else {
                        radius - 3.5
                    };
            ui.painter().line_segment(
                [inner, outer],
                Stroke::new(1.0, ui.visuals().weak_text_color()),
            );
        }
        let angle = self.plan.arrows[index].angle_degrees().to_radians();
        let tip = center + Vec2::angled(angle) * (radius - 9.0);
        ui.painter()
            .line_segment([center, tip], Stroke::new(2.0, ACCENT));
        ui.painter().circle_filled(tip, 4.5, ACCENT);
        if (dial_response.dragged() || dial_response.clicked())
            && let Some(pointer) = dial_response.interact_pointer_pos()
        {
            let vector = pointer - center;
            let degrees = vector.angle().to_degrees().rem_euclid(360.0);
            let step = (degrees / PlanArrow::DEGREES_PER_STEP).round() as u8 % 64;
            if step != self.plan.arrows[index].orientation {
                let before = self.snapshot();
                self.plan.arrows[index].orientation = step;
                self.commit(before);
            }
        }

        let mut color = self.plan.arrows[index].color;
        let response = property_row(ui, &self.localizer.text("property.color"), |ui| {
            ui.color_edit_button_srgb(&mut color)
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.arrows[index].color = color;
            self.commit(before);
        }
        let mut locked = self.plan.arrows[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.arrows[index].locked = locked;
            self.commit(before);
        }

        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(self.localizer.text("action.delete_arrow"))
                        .size(12.0)
                        .color(Color32::from_rgb(160, 50, 50)),
                )
                .frame(false),
            )
            .clicked()
        {
            self.remove_selected();
        }
    }

    fn image_inspector(&mut self, ui: &mut Ui, id: &str) {
        let Some(index) = self.plan.images.iter().position(|image| image.id == id) else {
            self.selection = None;
            return;
        };
        ui.label(
            egui::RichText::new(self.localizer.text("selection.image"))
                .strong()
                .size(12.0)
                .color(ACCENT),
        );
        if let Some(asset) = self
            .assets
            .iter()
            .find(|asset| asset.id == self.plan.images[index].asset_id)
        {
            ui.label(
                egui::RichText::new(&asset.name)
                    .size(11.0)
                    .color(self.muted_text_color()),
            );
        }
        ui.add_space(10.0);

        for (label_key, property) in [
            ("property.x", 0_u8),
            ("property.y", 1),
            ("property.width", 2),
            ("property.height", 3),
            ("property.rotation", 4),
        ] {
            let mut value = match property {
                0 => self.plan.images[index].x,
                1 => self.plan.images[index].y,
                2 => self.plan.images[index].width,
                3 => self.plan.images[index].height,
                _ => self.plan.images[index].rotation,
            };
            let response = property_row(ui, &self.localizer.text(label_key), |ui| {
                if property == 4 {
                    ui.add(
                        egui::DragValue::new(&mut value)
                            .speed(0.5)
                            .range(-180.0..=180.0)
                            .suffix("°"),
                    )
                } else {
                    distance_drag(
                        ui,
                        &mut value,
                        self.unit,
                        if property == 2 || property == 3 {
                            1.0
                        } else {
                            -1_000_000.0
                        },
                        1_000_000.0,
                    )
                }
            });
            if response.changed() && value.is_finite() {
                let before = self.snapshot();
                let image = &mut self.plan.images[index];
                match property {
                    0 => image.x = value,
                    1 => image.y = value,
                    2 => image.width = value,
                    3 => image.height = value,
                    _ => image.rotation = value,
                }
                self.commit(before);
            }
        }

        let mut locked = self.plan.images[index].locked;
        let response = property_row(ui, &self.localizer.text("property.locked"), |ui| {
            ui.checkbox(&mut locked, "")
        });
        if response.changed() {
            let before = self.snapshot();
            self.plan.images[index].locked = locked;
            self.commit(before);
        }

        ui.add_space(12.0);
        if ui.button(self.localizer.text("action.duplicate")).clicked() {
            self.duplicate_selected();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(self.localizer.text("action.delete_image"))
                        .size(12.0)
                        .color(Color32::from_rgb(160, 50, 50)),
                )
                .frame(false),
            )
            .clicked()
        {
            self.remove_selected();
        }
    }
}
