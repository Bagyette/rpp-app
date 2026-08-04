use super::*;
use crate::room_detection::{
    detect_room_polygons, point_in_polygon, polygon_center, stable_room_id,
};

impl RppApp {
    pub(super) fn refresh_automatic_rooms(&mut self) {
        let previous = self
            .plan
            .rooms
            .iter()
            .filter(|room| room.source == RoomSource::Automatic)
            .cloned()
            .collect::<Vec<_>>();
        let manual = self
            .plan
            .rooms
            .iter()
            .filter(|room| room.source == RoomSource::Manual)
            .cloned()
            .collect::<Vec<_>>();
        let mut automatic = Vec::new();

        for points in detect_room_polygons(&self.plan.walls) {
            let id = stable_room_id(&points);
            let center = polygon_center(&points);
            let metadata = previous.iter().find(|room| room.id == id).or_else(|| {
                previous
                    .iter()
                    .filter(|room| {
                        point_in_polygon(center, &room.points)
                            || point_in_polygon(polygon_center(&room.points), &points)
                    })
                    .min_by(|a, b| {
                        polygon_center(&a.points)
                            .distance(center)
                            .total_cmp(&polygon_center(&b.points).distance(center))
                    })
            });
            automatic.push(Room {
                id,
                name: metadata.map_or_else(String::new, |room| room.name.clone()),
                kind: metadata.map_or(RoomKind::Custom, |room| room.kind),
                source: RoomSource::Automatic,
                points,
                fill_color: metadata.map_or([220, 230, 239], |room| room.fill_color),
                fill_visible: metadata.is_none_or(|room| room.fill_visible),
                locked: metadata.is_some_and(|room| room.locked),
            });
        }

        let valid_ids = manual
            .iter()
            .chain(automatic.iter())
            .map(|room| room.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        self.plan.layer_assignments.retain(|assignment| {
            !assignment.item_id.starts_with("room-auto-")
                || valid_ids.contains(assignment.item_id.as_str())
        });
        self.plan.rooms = manual;
        self.plan.rooms.extend(automatic);
    }

    pub(super) fn handle_room_events(
        &mut self,
        ctx: &egui::Context,
        response: &Response,
        rect: Rect,
    ) {
        let finish = ctx.input(|input| input.key_pressed(Key::Enter))
            || response.double_clicked_by(PointerButton::Primary)
            || response.clicked_by(PointerButton::Secondary);
        if finish {
            self.finish_room();
            return;
        }
        if response.clicked_by(PointerButton::Primary)
            && let Some(position) = response.interact_pointer_pos()
        {
            let raw = self.screen_to_world(rect, position);
            let point = if ctx.input(|input| input.modifiers.shift) {
                self.snap_point(raw)
            } else {
                raw
            };
            if self
                .room_draft
                .last()
                .is_none_or(|previous| previous.distance(point) >= 1.0)
            {
                self.room_draft.push(point);
            }
        }
    }

    fn finish_room(&mut self) {
        let points = normalized_polygon_points(&std::mem::take(&mut self.room_draft));
        if points.len() < 3 {
            return;
        }
        let before = self.snapshot();
        let room = Room {
            id: make_id("room"),
            name: String::new(),
            kind: RoomKind::Custom,
            source: RoomSource::Manual,
            points,
            fill_color: [220, 230, 239],
            fill_visible: true,
            locked: false,
        };
        let id = room.id.clone();
        self.plan.rooms.push(room);
        self.assign_to_active_layer(&id);
        self.selection = Some(Selection::Room(id));
        self.status = self.localizer.text("status.room_added");
        self.commit(before);
        self.tool = Tool::Select;
    }

    pub(super) fn begin_room_drag(&mut self, pointer: Point) -> bool {
        let room = self
            .plan
            .rooms
            .iter()
            .rev()
            .find(|room| {
                room.source == RoomSource::Manual
                    && !room.locked
                    && self.plan.item_editable(&room.id)
                    && point_in_polygon(pointer, &room.points)
            })
            .cloned();
        let Some(room) = room else {
            return false;
        };
        self.selection = Some(Selection::Room(room.id.clone()));
        self.room_drag = Some(RoomDrag {
            room_id: room.id.clone(),
            pointer_start: pointer,
            original: room,
            before: self.snapshot(),
        });
        true
    }

    pub(super) fn update_room_drag(&mut self, pointer: Point) {
        let Some(drag) = self.room_drag.clone() else {
            return;
        };
        let delta = Point::new(
            pointer.x - drag.pointer_start.x,
            pointer.y - drag.pointer_start.y,
        );
        if let Some(room) = self
            .plan
            .rooms
            .iter_mut()
            .find(|room| room.id == drag.room_id)
        {
            for (point, original) in room.points.iter_mut().zip(&drag.original.points) {
                point.x = original.x + delta.x;
                point.y = original.y + delta.y;
            }
        }
    }
}
