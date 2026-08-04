use super::*;

impl RppApp {
    pub(super) fn paint_canvas(&self, painter: &Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, self.canvas_fill());
        self.paint_grid(painter, rect);

        for layer in &self.plan.layers {
            if !layer.visible {
                continue;
            }
            let layer_id = layer.id.as_str();
            for room in &self.plan.rooms {
                if self.plan.layer_id_for(&room.id) != layer_id || !self.plan.item_visible(&room.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Room(room.id.clone()));
                self.paint_room(painter, rect, room, selected);
            }
            for area in &self.plan.areas {
                if self.plan.layer_id_for(&area.id) != layer_id || !self.plan.item_visible(&area.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Area(area.id.clone()));
                self.paint_area(painter, rect, area, selected);
            }
            for image in &self.plan.images {
                if self.plan.layer_id_for(&image.id) != layer_id
                    || !self.plan.item_visible(&image.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Image(image.id.clone()));
                self.paint_image(painter, rect, image, selected);
            }
            for object in &self.plan.objects {
                if self.plan.layer_id_for(&object.id) != layer_id
                    || !self.plan.item_visible(&object.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Object(object.id.clone()));
                self.paint_object(painter, rect, object, selected);
            }
            for line in &self.plan.lines {
                if self.plan.layer_id_for(&line.id) != layer_id || !self.plan.item_visible(&line.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Line(line.id.clone()));
                let color = if selected {
                    ACCENT
                } else {
                    Color32::from_rgb(line.color[0], line.color[1], line.color[2])
                        .gamma_multiply(self.plan.item_opacity(&line.id))
                };
                painter.line_segment(
                    [
                        self.world_to_screen(rect, line.start()),
                        self.world_to_screen(rect, line.end()),
                    ],
                    Stroke::new(if selected { 2.5 } else { 1.5 }, color),
                );
            }
            for wall in &self.plan.walls {
                if self.plan.layer_id_for(&wall.id) != layer_id || !self.plan.item_visible(&wall.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Wall(wall.id.clone()));
                self.paint_wall(painter, rect, wall, selected);
            }
            for door in &self.plan.doors {
                if self.plan.layer_id_for(&door.id) != layer_id || !self.plan.item_visible(&door.id)
                {
                    continue;
                }
                let Some(wall) = self.plan.walls.iter().find(|wall| wall.id == door.wall_id) else {
                    continue;
                };
                if !self.plan.item_visible(&wall.id) {
                    continue;
                }
                let selected = self.is_selected(&Selection::Door(door.id.clone()));
                self.paint_door(painter, rect, door, wall, selected);
            }
            for window in &self.plan.windows {
                if self.plan.layer_id_for(&window.id) != layer_id
                    || !self.plan.item_visible(&window.id)
                {
                    continue;
                }
                let Some(wall) = self
                    .plan
                    .walls
                    .iter()
                    .find(|wall| wall.id == window.wall_id)
                else {
                    continue;
                };
                if !self.plan.item_visible(&wall.id) {
                    continue;
                }
                let selected = self.is_selected(&Selection::Window(window.id.clone()));
                self.paint_window(painter, rect, window, wall, selected);
            }
            for text in &self.plan.texts {
                if self.plan.layer_id_for(&text.id) != layer_id || !self.plan.item_visible(&text.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Text(text.id.clone()));
                self.paint_text(painter, rect, text, selected);
            }
            for compass in &self.plan.compasses {
                if self.plan.layer_id_for(&compass.id) != layer_id
                    || !self.plan.item_visible(&compass.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Compass(compass.id.clone()));
                self.paint_compass(painter, rect, compass, selected);
            }
            for arrow in &self.plan.arrows {
                if self.plan.layer_id_for(&arrow.id) != layer_id
                    || !self.plan.item_visible(&arrow.id)
                {
                    continue;
                }
                let selected = self.is_selected(&Selection::Arrow(arrow.id.clone()));
                self.paint_arrow(painter, rect, arrow, selected);
            }
        }
        if let Some(draft) = &self.draft {
            let start = self.world_to_screen(rect, draft.start);
            let end = self.world_to_screen(rect, draft.end);
            painter.line_segment(
                [start, end],
                Stroke::new(WALL_WIDTH * self.zoom, ACCENT.gamma_multiply(0.75)),
            );
            painter.circle(start, 4.0, Color32::WHITE, Stroke::new(2.0, ACCENT));
            painter.circle(end, 4.0, Color32::WHITE, Stroke::new(2.0, ACCENT));
        }
        if let Some(draft) = &self.line_draft {
            let start = self.world_to_screen(rect, draft.start);
            let end = self.world_to_screen(rect, draft.end);
            painter.line_segment([start, end], Stroke::new(1.8, ACCENT));
            painter.circle(start, 3.0, Color32::WHITE, Stroke::new(1.5, ACCENT));
            painter.circle(end, 3.0, Color32::WHITE, Stroke::new(1.5, ACCENT));
        }
        if let Some(draft) = &self.arrow_draft {
            let arrow = PlanArrow {
                id: String::new(),
                x: draft.start.x,
                y: draft.start.y,
                length: draft.start.distance(draft.end),
                width: 20.0,
                style: ArrowStyle::Open,
                orientation: arrow_orientation_step(draft.start, draft.end),
                color: default_arrow_color(),
                locked: false,
            };
            self.paint_arrow(painter, rect, &arrow, true);
        }
        if let Some(measurement) = &self.measurement {
            let start = self.world_to_screen(rect, measurement.start);
            let end = self.world_to_screen(rect, measurement.end);
            painter.line_segment([start, end], Stroke::new(1.5, ACCENT));
            painter.circle(start, 4.0, Color32::WHITE, Stroke::new(1.5, ACCENT));
            painter.circle(end, 4.0, Color32::WHITE, Stroke::new(1.5, ACCENT));
            let label = format_distance(measurement.start.distance(measurement.end), self.unit);
            let midpoint = start.lerp(end, 0.5);
            let galley = painter.layout_no_wrap(label, FontId::proportional(12.0), Color32::WHITE);
            let label_rect = Rect::from_center_size(midpoint, galley.size() + Vec2::new(12.0, 6.0));
            painter.rect_filled(label_rect, 0.0, ACCENT);
            painter.galley(midpoint - galley.size() / 2.0, galley, Color32::WHITE);
        }
        if let Some(selection_box) = &self.selection_box {
            let selection_rect = Rect::from_two_pos(
                self.world_to_screen(rect, selection_box.start),
                self.world_to_screen(rect, selection_box.end),
            );
            painter.rect_filled(selection_rect, 0.0, ACCENT.gamma_multiply(0.08));
            painter.rect_stroke(
                selection_rect,
                0.0,
                Stroke::new(1.2, ACCENT),
                StrokeKind::Inside,
            );
        }
        if !self.area_draft.is_empty() {
            let mut points: Vec<Pos2> = self
                .area_draft
                .iter()
                .map(|point| self.world_to_screen(rect, *point))
                .collect();
            if let Some(pointer) = painter.ctx().pointer_hover_pos() {
                points.push(pointer);
            }
            painter.add(Shape::line(points, Stroke::new(1.8, ACCENT)));
            for point in &self.area_draft {
                painter.circle_filled(self.world_to_screen(rect, *point), 3.0, ACCENT);
            }
        }
        if !self.room_draft.is_empty() {
            let mut points: Vec<Pos2> = self
                .room_draft
                .iter()
                .map(|point| self.world_to_screen(rect, *point))
                .collect();
            if let Some(pointer) = painter.ctx().pointer_hover_pos() {
                points.push(pointer);
            }
            painter.add(Shape::line(points, Stroke::new(1.8, ACCENT)));
            for point in &self.room_draft {
                painter.circle_filled(self.world_to_screen(rect, *point), 3.0, ACCENT);
            }
        }

        if self.plan.walls.is_empty() && self.draft.is_none() {
            let hint = Rect::from_center_size(rect.center(), Vec2::new(190.0, 58.0));
            painter.rect(
                hint,
                CornerRadius::ZERO,
                Color32::from_white_alpha(245),
                Stroke::new(1.0, Color32::from_gray(215)),
                StrokeKind::Outside,
            );
            painter.text(
                hint.center_top() + Vec2::new(0.0, 15.0),
                Align2::CENTER_TOP,
                self.localizer.text("hint.first_wall"),
                FontId::proportional(14.0),
                INK,
            );
            painter.text(
                hint.center_bottom() - Vec2::new(0.0, 17.0),
                Align2::CENTER_BOTTOM,
                self.localizer.text("hint.first_wall_detail"),
                FontId::proportional(12.0),
                Color32::from_gray(115),
            );
        }
    }

    fn paint_image(&self, painter: &Painter, rect: Rect, image: &ImageObject, selected: bool) {
        let opacity = self.plan.item_opacity(&image.id);
        let angle = image.rotation.to_radians();
        let cosine = angle.cos();
        let sine = angle.sin();
        let corners = [
            (-image.width / 2.0, -image.height / 2.0),
            (image.width / 2.0, -image.height / 2.0),
            (image.width / 2.0, image.height / 2.0),
            (-image.width / 2.0, image.height / 2.0),
        ];
        let positions: Vec<Pos2> = corners
            .iter()
            .map(|(x, y)| {
                let world = Point::new(
                    image.x + x * cosine - y * sine,
                    image.y + x * sine + y * cosine,
                );
                self.world_to_screen(rect, world)
            })
            .collect();
        let Some(texture) = self.textures.get(&image.asset_id) else {
            painter.add(Shape::convex_polygon(
                positions.clone(),
                Color32::from_gray(225),
                Stroke::new(1.5, Color32::from_gray(125)),
            ));
            painter.line_segment(
                [positions[0], positions[2]],
                Stroke::new(1.5, Color32::from_gray(125)),
            );
            painter.line_segment(
                [positions[1], positions[3]],
                Stroke::new(1.5, Color32::from_gray(125)),
            );
            return;
        };
        let mut mesh = Mesh::with_texture(texture.id());
        let uvs = [
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
        ];
        for index in 0..4 {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: positions[index],
                uv: uvs[index],
                color: Color32::from_white_alpha((255.0 * opacity) as u8),
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(Shape::mesh(mesh));
        if selected {
            painter.add(Shape::closed_line(positions, Stroke::new(2.0, ACCENT)));
        }
    }

    fn paint_area(&self, painter: &Painter, rect: Rect, area: &AreaShape, selected: bool) {
        let opacity = self.plan.item_opacity(&area.id);
        let normalized = normalized_polygon_points(&area.points);
        let points: Vec<Pos2> = normalized
            .iter()
            .map(|point| self.world_to_screen(rect, *point))
            .collect();
        if points.len() < 3 {
            return;
        }
        let fill = Color32::from_rgba_unmultiplied(
            area.fill_color[0],
            area.fill_color[1],
            area.fill_color[2],
            (120.0 * opacity) as u8,
        );
        let stroke = if selected {
            ACCENT
        } else {
            Color32::from_rgb(
                area.stroke_color[0],
                area.stroke_color[1],
                area.stroke_color[2],
            )
            .gamma_multiply(opacity)
        };
        let mut mesh = Mesh::default();
        for triangle in triangulate_polygon(&points) {
            let base = mesh.vertices.len() as u32;
            for point in triangle {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: point,
                    uv: Pos2::ZERO,
                    color: fill,
                });
            }
            mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        if !mesh.indices.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
        painter.add(Shape::closed_line(
            points,
            Stroke::new(if selected { 2.0 } else { 1.3 }, stroke),
        ));
        let center = area_center(area);
        painter.text(
            self.world_to_screen(rect, center),
            Align2::CENTER_CENTER,
            format_area(area.area_square_centimeters(), self.unit),
            FontId::proportional(12.0),
            stroke,
        );
    }

    fn paint_room(&self, painter: &Painter, rect: Rect, room: &Room, selected: bool) {
        let points = normalized_polygon_points(&room.points)
            .iter()
            .map(|point| self.world_to_screen(rect, *point))
            .collect::<Vec<_>>();
        if points.len() < 3 {
            return;
        }
        let opacity = self.plan.item_opacity(&room.id);
        let fill_alpha = if room.fill_visible {
            (78.0 * opacity).round() as u8
        } else {
            0
        };
        let fill = Color32::from_rgba_unmultiplied(
            room.fill_color[0],
            room.fill_color[1],
            room.fill_color[2],
            fill_alpha,
        );
        if room.fill_visible {
            let mut mesh = Mesh::default();
            for triangle in triangulate_polygon(&points) {
                let base = mesh.vertices.len() as u32;
                for point in triangle {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: point,
                        uv: Pos2::ZERO,
                        color: fill,
                    });
                }
                mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
            }
            if !mesh.indices.is_empty() {
                painter.add(Shape::mesh(mesh));
            }
        }
        let outline = if selected {
            ACCENT
        } else {
            Color32::from_rgba_unmultiplied(60, 105, 140, (150.0 * opacity) as u8)
        };
        painter.add(Shape::closed_line(
            points,
            Stroke::new(if selected { 2.0 } else { 1.0 }, outline),
        ));
        let center = crate::room_detection::polygon_center(&room.points);
        let title = if room.name.trim().is_empty() {
            self.localizer.text("room.default_name")
        } else {
            room.name.trim().to_owned()
        };
        painter.text(
            self.world_to_screen(rect, center),
            Align2::CENTER_CENTER,
            format!(
                "{}\n{}",
                title,
                format_area(room.area_square_centimeters(), self.unit)
            ),
            FontId::proportional(12.0),
            outline,
        );
    }

    fn paint_compass(&self, painter: &Painter, rect: Rect, compass: &CompassRose, selected: bool) {
        let center = self.world_to_screen(rect, Point::new(compass.x, compass.y));
        let radius = compass.size * self.zoom * 0.5;
        let angle = compass.rotation.to_radians() - std::f32::consts::FRAC_PI_2;
        let north = center + Vec2::angled(angle) * radius;
        let south = center - Vec2::angled(angle) * radius * 0.7;
        let color = if selected {
            ACCENT
        } else {
            Color32::from_rgb(compass.color[0], compass.color[1], compass.color[2])
                .gamma_multiply(self.plan.item_opacity(&compass.id))
        };
        painter.circle_stroke(center, radius, Stroke::new(1.3, color));
        painter.line_segment([south, north], Stroke::new(2.0, color));
        let wing = radius * 0.22;
        painter.add(Shape::convex_polygon(
            vec![
                north,
                north - Vec2::angled(angle - 0.55) * wing,
                north - Vec2::angled(angle + 0.55) * wing,
            ],
            color,
            Stroke::NONE,
        ));
        painter.text(
            north + Vec2::angled(angle) * 5.0,
            Align2::CENTER_BOTTOM,
            "N",
            FontId::proportional((14.0 * self.zoom).clamp(10.0, 22.0)),
            color,
        );
    }

    fn paint_arrow(&self, painter: &Painter, rect: Rect, arrow: &PlanArrow, selected: bool) {
        let angle = arrow.angle_degrees().to_radians();
        let direction = Point::new(angle.cos(), angle.sin());
        let normal = Point::new(-direction.y, direction.x);
        let tail = Point::new(arrow.x, arrow.y);
        let tip = arrow.end();
        let head_length = (arrow.width * 1.5).min(arrow.length * 0.55);
        let head_base = Point::new(
            tip.x - direction.x * head_length,
            tip.y - direction.y * head_length,
        );
        let head_left = Point::new(
            head_base.x + normal.x * arrow.width * 0.5,
            head_base.y + normal.y * arrow.width * 0.5,
        );
        let head_right = Point::new(
            head_base.x - normal.x * arrow.width * 0.5,
            head_base.y - normal.y * arrow.width * 0.5,
        );
        let color = if selected {
            ACCENT
        } else {
            Color32::from_rgb(arrow.color[0], arrow.color[1], arrow.color[2])
                .gamma_multiply(self.plan.item_opacity(&arrow.id))
        };
        let thin = (arrow.width * 0.08 * self.zoom).max(1.2);
        let bold = (arrow.width * 0.18 * self.zoom).max(2.0);

        match arrow.style {
            ArrowStyle::Open => {
                painter.line_segment(
                    [
                        self.world_to_screen(rect, tail),
                        self.world_to_screen(rect, tip),
                    ],
                    Stroke::new(thin, color),
                );
                painter.add(Shape::line(
                    vec![
                        self.world_to_screen(rect, head_left),
                        self.world_to_screen(rect, tip),
                        self.world_to_screen(rect, head_right),
                    ],
                    Stroke::new(thin, color),
                ));
            }
            ArrowStyle::OpenBold => {
                painter.line_segment(
                    [
                        self.world_to_screen(rect, tail),
                        self.world_to_screen(rect, tip),
                    ],
                    Stroke::new(bold, color),
                );
                painter.add(Shape::line(
                    vec![
                        self.world_to_screen(rect, head_left),
                        self.world_to_screen(rect, tip),
                        self.world_to_screen(rect, head_right),
                    ],
                    Stroke::new(bold, color),
                ));
            }
            ArrowStyle::Hollow => {
                painter.line_segment(
                    [
                        self.world_to_screen(rect, tail),
                        self.world_to_screen(rect, head_base),
                    ],
                    Stroke::new(bold, color),
                );
                painter.add(Shape::closed_line(
                    vec![
                        self.world_to_screen(rect, head_left),
                        self.world_to_screen(rect, tip),
                        self.world_to_screen(rect, head_right),
                    ],
                    Stroke::new(thin, color),
                ));
            }
            ArrowStyle::Solid => {
                let shaft_half = arrow.width * 0.18;
                let shaft_left = Point::new(
                    tail.x + normal.x * shaft_half,
                    tail.y + normal.y * shaft_half,
                );
                let shaft_right = Point::new(
                    tail.x - normal.x * shaft_half,
                    tail.y - normal.y * shaft_half,
                );
                let base_left = Point::new(
                    head_base.x + normal.x * shaft_half,
                    head_base.y + normal.y * shaft_half,
                );
                let base_right = Point::new(
                    head_base.x - normal.x * shaft_half,
                    head_base.y - normal.y * shaft_half,
                );
                // The solid arrow is concave as one outline. Feeding that outline to
                // `convex_polygon` makes egui triangulate it as a self-crossing blob.
                // Draw two genuinely convex parts instead: shaft, then arrowhead.
                painter.add(Shape::convex_polygon(
                    vec![
                        self.world_to_screen(rect, shaft_left),
                        self.world_to_screen(rect, base_left),
                        self.world_to_screen(rect, base_right),
                        self.world_to_screen(rect, shaft_right),
                    ],
                    color,
                    Stroke::NONE,
                ));
                painter.add(Shape::convex_polygon(
                    vec![
                        self.world_to_screen(rect, head_left),
                        self.world_to_screen(rect, tip),
                        self.world_to_screen(rect, head_right),
                    ],
                    color,
                    Stroke::NONE,
                ));
            }
        }
    }

    fn paint_text(&self, painter: &Painter, rect: Rect, text: &TextObject, selected: bool) {
        let center = self.world_to_screen(rect, Point::new(text.x, text.y));
        let font_size = (text.size * self.zoom).clamp(6.0, 240.0);
        let color = if selected {
            ACCENT
        } else {
            Color32::from_rgb(text.color[0], text.color[1], text.color[2])
                .gamma_multiply(self.plan.item_opacity(&text.id))
        };
        let galley = painter.layout(
            text.text.clone(),
            FontId::new(font_size, arial_font_family(text.bold)),
            color,
            f32::INFINITY,
        );
        let top_left = center - galley.size() / 2.0;
        painter.add(
            TextShape::new(top_left, galley.clone(), color)
                .with_angle_and_anchor(text.rotation.to_radians(), Align2::CENTER_CENTER),
        );
        if selected {
            let corners = rotated_rectangle(
                text.x,
                text.y,
                galley.size().x / self.zoom,
                galley.size().y / self.zoom,
                text.rotation,
            )
            .map(|point| self.world_to_screen(rect, point));
            painter.add(Shape::closed_line(
                corners.to_vec(),
                Stroke::new(1.2, ACCENT),
            ));
        }
    }

    fn paint_object(&self, painter: &Painter, rect: Rect, object: &PlanObject, selected: bool) {
        let opacity = self.plan.item_opacity(&object.id);
        let positions = rotated_rectangle(
            object.x,
            object.y,
            object.width,
            object.height,
            object.rotation,
        )
        .map(|point| self.world_to_screen(rect, point));
        let stroke = Stroke::new(
            if selected { 2.0 } else { 1.3 },
            if selected {
                ACCENT
            } else {
                Color32::from_rgb(
                    object.stroke_color[0],
                    object.stroke_color[1],
                    object.stroke_color[2],
                )
                .gamma_multiply(opacity)
            },
        );
        painter.add(Shape::convex_polygon(
            positions.to_vec(),
            Color32::from_rgba_unmultiplied(
                object.fill_color[0],
                object.fill_color[1],
                object.fill_color[2],
                (210.0 * opacity) as u8,
            ),
            stroke,
        ));
        let label_key = match object.kind {
            ObjectKind::Rectangle => "object.rectangle",
            ObjectKind::Bed => "object.bed",
            ObjectKind::Table => "object.table",
            ObjectKind::Sofa => "object.sofa",
            ObjectKind::Wardrobe => "object.wardrobe",
            ObjectKind::Desk => "object.desk",
        };
        let fallback = self.localizer.text(label_key);
        painter.text(
            self.world_to_screen(rect, Point::new(object.x, object.y)),
            Align2::CENTER_CENTER,
            object.display_name(&fallback),
            FontId::proportional((12.0 * self.zoom).clamp(9.0, 16.0)),
            Color32::from_rgb(
                object.stroke_color[0],
                object.stroke_color[1],
                object.stroke_color[2],
            )
            .gamma_multiply(opacity),
        );
        if selected && self.tool == Tool::Select {
            for handle in object_resize_handles(object) {
                painter.rect_filled(
                    Rect::from_center_size(self.world_to_screen(rect, handle), Vec2::splat(7.0)),
                    0.0,
                    Color32::WHITE,
                );
                painter.rect_stroke(
                    Rect::from_center_size(self.world_to_screen(rect, handle), Vec2::splat(7.0)),
                    0.0,
                    Stroke::new(1.2, ACCENT),
                    StrokeKind::Outside,
                );
            }
        }
    }

    fn paint_window(
        &self,
        painter: &Painter,
        rect: Rect,
        window: &Window,
        wall: &Wall,
        selected: bool,
    ) {
        let dx = wall.x2 - wall.x1;
        let dy = wall.y2 - wall.y1;
        let length = wall.length();
        if length <= f32::EPSILON {
            return;
        }
        let ux = dx / length;
        let uy = dy / length;
        let center = wall.point_at(window.t);
        let start = Point::new(
            center.x - ux * window.width / 2.0,
            center.y - uy * window.width / 2.0,
        );
        let end = Point::new(
            center.x + ux * window.width / 2.0,
            center.y + uy * window.width / 2.0,
        );
        painter.line_segment(
            [
                self.world_to_screen(rect, start),
                self.world_to_screen(rect, end),
            ],
            Stroke::new(
                (wall.thickness + 4.0) * self.zoom,
                self.canvas_fill()
                    .gamma_multiply(self.plan.item_opacity(&window.id)),
            ),
        );
        let color = if selected {
            ACCENT
        } else {
            Color32::from_rgb(window.color[0], window.color[1], window.color[2])
                .gamma_multiply(self.plan.item_opacity(&window.id))
        };
        let offsets: &[f32] = if window.kind == WindowKind::Bay {
            &[-0.35, 0.0, 0.35]
        } else {
            &[-0.25, 0.25]
        };
        for offset in offsets {
            let distance = wall.thickness * offset;
            let line_start = Point::new(start.x - uy * distance, start.y + ux * distance);
            let line_end = Point::new(end.x - uy * distance, end.y + ux * distance);
            painter.line_segment(
                [
                    self.world_to_screen(rect, line_start),
                    self.world_to_screen(rect, line_end),
                ],
                Stroke::new(1.6 * self.zoom, color),
            );
        }
    }

    fn paint_grid(&self, painter: &Painter, rect: Rect) {
        if !self.grid_visible {
            return;
        }
        let top_left = self.screen_to_world(rect, rect.min);
        let bottom_right = self.screen_to_world(rect, rect.max);
        let grid = self.grid_size.max(1.0);
        let start_x = (top_left.x / grid).floor() as i32 - 1;
        let end_x = (bottom_right.x / grid).ceil() as i32 + 1;
        let start_y = (top_left.y / grid).floor() as i32 - 1;
        let end_y = (bottom_right.y / grid).ceil() as i32 + 1;

        for i in start_x..=end_x {
            let x = i as f32 * grid;
            let major = i % 5 == 0;
            let a = self.world_to_screen(rect, Point::new(x, top_left.y));
            let b = self.world_to_screen(rect, Point::new(x, bottom_right.y));
            painter.line_segment(
                [a, b],
                Stroke::new(if major { 1.1 } else { 0.65 }, self.grid_color(major)),
            );
        }
        for i in start_y..=end_y {
            let y = i as f32 * grid;
            let major = i % 5 == 0;
            let a = self.world_to_screen(rect, Point::new(top_left.x, y));
            let b = self.world_to_screen(rect, Point::new(bottom_right.x, y));
            painter.line_segment(
                [a, b],
                Stroke::new(if major { 1.1 } else { 0.65 }, self.grid_color(major)),
            );
        }
    }

    fn paint_wall(&self, painter: &Painter, rect: Rect, wall: &Wall, selected: bool) {
        let start = self.world_to_screen(rect, wall.start());
        let end = self.world_to_screen(rect, wall.end());
        let color = if selected {
            ACCENT
        } else {
            Color32::from_rgb(wall.color[0], wall.color[1], wall.color[2])
                .gamma_multiply(self.plan.item_opacity(&wall.id))
        };
        painter.line_segment([start, end], Stroke::new(wall.thickness * self.zoom, color));

        let midpoint = start.lerp(end, 0.5);
        let direction = end - start;
        let normal = if direction.length_sq() > 0.0 {
            Vec2::new(-direction.y, direction.x).normalized()
        } else {
            Vec2::ZERO
        };
        let label_pos = midpoint - normal * 20.0;
        let label = format_distance(wall.length(), self.unit);
        let galley =
            painter.layout_no_wrap(label, FontId::proportional(12.0), Color32::from_gray(80));
        let label_rect = Rect::from_center_size(label_pos, galley.size() + Vec2::new(10.0, 5.0));
        painter.rect_filled(label_rect, 0.0, Color32::from_white_alpha(235));
        painter.galley(
            label_pos - galley.size() / 2.0,
            galley,
            Color32::from_gray(80),
        );

        if selected && self.tool == Tool::Select {
            painter.circle(start, 7.0, Color32::WHITE, Stroke::new(2.0, ACCENT));
            painter.circle(end, 7.0, Color32::WHITE, Stroke::new(2.0, ACCENT));
        }
    }

    fn paint_door(&self, painter: &Painter, rect: Rect, door: &Door, wall: &Wall, selected: bool) {
        let dx = wall.x2 - wall.x1;
        let dy = wall.y2 - wall.y1;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= f32::EPSILON {
            return;
        }
        let ux = dx / length;
        let uy = dy / length;
        let center = wall.point_at(door.t);
        let left = Point::new(
            center.x - ux * door.width / 2.0,
            center.y - uy * door.width / 2.0,
        );
        let right = Point::new(
            center.x + ux * door.width / 2.0,
            center.y + uy * door.width / 2.0,
        );
        let color = if selected {
            ACCENT
        } else {
            Color32::from_rgb(door.color[0], door.color[1], door.color[2])
                .gamma_multiply(self.plan.item_opacity(&door.id))
        };

        painter.line_segment(
            [
                self.world_to_screen(rect, left),
                self.world_to_screen(rect, right),
            ],
            Stroke::new(
                (wall.thickness + 4.0) * self.zoom,
                self.canvas_fill()
                    .gamma_multiply(self.plan.item_opacity(&door.id)),
            ),
        );

        if door.kind == DoorKind::Sliding {
            let offset = wall.thickness * 0.28;
            let middle = center;
            let panel_a_end = Point::new(middle.x, middle.y);
            let panel_b_start = Point::new(middle.x, middle.y);
            painter.line_segment(
                [
                    self.world_to_screen(
                        rect,
                        Point::new(left.x - uy * offset, left.y + ux * offset),
                    ),
                    self.world_to_screen(
                        rect,
                        Point::new(panel_a_end.x - uy * offset, panel_a_end.y + ux * offset),
                    ),
                ],
                Stroke::new(2.5 * self.zoom, color),
            );
            painter.line_segment(
                [
                    self.world_to_screen(
                        rect,
                        Point::new(panel_b_start.x + uy * offset, panel_b_start.y - ux * offset),
                    ),
                    self.world_to_screen(
                        rect,
                        Point::new(right.x + uy * offset, right.y - ux * offset),
                    ),
                ],
                Stroke::new(2.5 * self.zoom, color),
            );
            return;
        }

        let normal_sign = match door.swing {
            SwingSide::Inward => 1.0,
            SwingSide::Outward => -1.0,
        };
        let leaves: Vec<(Point, Vec2, f32)> = if door.kind == DoorKind::Double {
            vec![
                (left, Vec2::new(ux, uy), door.width / 2.0),
                (right, Vec2::new(-ux, -uy), door.width / 2.0),
            ]
        } else {
            match door.hinge {
                HingeSide::Left => vec![(left, Vec2::new(ux, uy), door.width)],
                HingeSide::Right => vec![(right, Vec2::new(-ux, -uy), door.width)],
            }
        };

        for (hinge, closed_direction, leaf_width) in leaves {
            let open_direction = Vec2::new(-uy * normal_sign, ux * normal_sign);
            let leaf_end = Point::new(
                hinge.x + open_direction.x * leaf_width,
                hinge.y + open_direction.y * leaf_width,
            );
            painter.line_segment(
                [
                    self.world_to_screen(rect, hinge),
                    self.world_to_screen(rect, leaf_end),
                ],
                Stroke::new(3.0 * self.zoom, color),
            );

            let start_angle = closed_direction.y.atan2(closed_direction.x);
            let mut delta = open_direction.y.atan2(open_direction.x) - start_angle;
            while delta > std::f32::consts::PI {
                delta -= std::f32::consts::TAU;
            }
            while delta < -std::f32::consts::PI {
                delta += std::f32::consts::TAU;
            }
            let segments = 18;
            let mut points = Vec::with_capacity(segments + 1);
            for i in 0..=segments {
                let angle = start_angle + delta * i as f32 / segments as f32;
                let point = Point::new(
                    hinge.x + angle.cos() * leaf_width,
                    hinge.y + angle.sin() * leaf_width,
                );
                points.push(self.world_to_screen(rect, point));
            }
            painter.add(Shape::line(
                points,
                Stroke::new(1.3 * self.zoom, color.gamma_multiply(0.7)),
            ));
            painter.circle_filled(self.world_to_screen(rect, hinge), 4.0, color);
        }
    }

    pub(super) fn screen_to_world(&self, rect: Rect, point: Pos2) -> Point {
        let local = point - rect.min - self.pan;
        Point::new(local.x / self.zoom, local.y / self.zoom)
    }

    pub(super) fn world_to_screen(&self, rect: Rect, point: Point) -> Pos2 {
        rect.min + self.pan + Vec2::new(point.x, point.y) * self.zoom
    }
}
