use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::{Cursor, Read, Write};
use std::str::FromStr;
use std::{fmt, str};

use miniz_oxide::inflate::decompress_to_vec_zlib_with_limit;
use serde::{Deserialize, Serialize};

use crate::model::{
    AreaShape, ArrowStyle, CompassRose, Door, DoorKind, HingeSide, ImageObject, Layer,
    LayerAssignment, ObjectKind, PlanArrow, PlanFile, PlanLine, PlanObject, Point, ProjectAsset,
    Room, RoomKind, RoomSource, SwingSide, TextObject, Wall, WallKind, Window, WindowKind,
    default_layers, default_line_color, default_object_fill_color, default_object_stroke_color,
    default_opening_color, default_text_color, default_wall_color, default_wall_thickness,
    default_window_color,
};

const TEXT_HEADER: &str = "RPP-PLAN 2";
const LEGACY_TEXT_HEADER: &str = "RPP-PLAN 1";
const CONTAINER_MAGIC: &[u8; 4] = b"PK\x03\x04";
const BINARY_MAGIC: &[u8; 8] = b"RPPBIN\x1A\0";
const BINARY_VERSION: u16 = 1;
const BINARY_HEADER_LEN: usize = 8 + 2 + 4 + 4;
const MAX_PLAN_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PROJECT_BYTES: usize = 256 * 1024 * 1024;
const MAX_ASSET_BYTES: usize = 32 * 1024 * 1024;
const MAX_ASSETS: usize = 256;

#[derive(Debug)]
pub enum FormatError {
    UnknownFormat,
    UnsupportedVersion(u16),
    InvalidHeader,
    TooLarge,
    Corrupted,
    InvalidPlan,
    Syntax { line: usize, message: String },
}

impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormat => write!(formatter, "unknown file format"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported RPP version ({version})")
            }
            Self::InvalidHeader => write!(formatter, "invalid RPP header"),
            Self::TooLarge => write!(formatter, "RPP file is too large"),
            Self::Corrupted => write!(formatter, "corrupted RPP file"),
            Self::InvalidPlan => write!(formatter, "invalid plan content"),
            Self::Syntax { line, message } => {
                write!(formatter, "line {line}: {message}")
            }
        }
    }
}

impl Error for FormatError {}

pub struct DecodedPlan {
    pub plan: PlanFile,
    pub assets: Vec<ProjectAsset>,
    pub needs_conversion: bool,
}

#[derive(Clone)]
struct AssetDescriptor {
    id: String,
    extension: String,
    name: String,
}

/// Writes the proprietary RPP-PLAN text format.
#[cfg(test)]
pub fn encode_plan(plan: &PlanFile) -> Result<Vec<u8>, FormatError> {
    encode_plan_data(plan, &[])
}

fn encode_plan_data(plan: &PlanFile, assets: &[ProjectAsset]) -> Result<Vec<u8>, FormatError> {
    validate_plan(plan)?;
    validate_assets(plan, assets)?;

    let mut output = String::from(
        "RPP-PLAN 2\n\
         # Readable RPP format. All dimensions are in centimeters.\n\
         UNITS cm\n\n\
         # WALL <id> <x1> <y1> <x2> <y2> <thickness> <type> <locked> <color>\n",
    );

    for wall in &plan.walls {
        output.push_str(&format!(
            "WALL {} {} {} {} {} {} {} {} {}\n",
            wall.id,
            number(wall.x1),
            number(wall.y1),
            number(wall.x2),
            number(wall.y2),
            number(wall.thickness),
            wall.kind.as_str(),
            wall.locked,
            color_hex(wall.color),
        ));
    }

    output.push_str("\n# LINE <id> <x1> <y1> <x2> <y2> <locked> <color>\n");
    for line in &plan.lines {
        output.push_str(&format!(
            "LINE {} {} {} {} {} {} {}\n",
            line.id,
            number(line.x1),
            number(line.y1),
            number(line.x2),
            number(line.y2),
            line.locked,
            color_hex(line.color),
        ));
    }

    output.push_str(
        "\n# DOOR <id> <wall-id> <position> <width> <type> <hinge> <swing> <locked> <color>\n",
    );
    for door in &plan.doors {
        output.push_str(&format!(
            "DOOR {} {} {} {} {} {} {} {} {}\n",
            door.id,
            door.wall_id,
            number(door.t),
            number(door.width),
            door.kind.as_str(),
            door.hinge.as_str(),
            door.swing.as_str(),
            door.locked,
            color_hex(door.color),
        ));
    }

    output.push_str("\n# WINDOW <id> <wall-id> <position> <width> <type> <locked> <color>\n");
    for window in &plan.windows {
        output.push_str(&format!(
            "WINDOW {} {} {} {} {} {} {}\n",
            window.id,
            window.wall_id,
            number(window.t),
            number(window.width),
            window.kind.as_str(),
            window.locked,
            color_hex(window.color),
        ));
    }

    output.push_str(
        "\n# OBJECT <id> <type> <x> <y> <width> <height> <rotation> <locked> <fill-color> <stroke-color>\n",
    );
    for object in &plan.objects {
        output.push_str(&format!(
            "OBJECT {} {} {} {} {} {} {} {} {} {}\n",
            object.id,
            object.kind.as_str(),
            number(object.x),
            number(object.y),
            number(object.width),
            number(object.height),
            number(object.rotation),
            object.locked,
            color_hex(object.fill_color),
            color_hex(object.stroke_color),
        ));
        if !object.name.trim().is_empty() {
            let name = serde_json::to_string(&object.name).map_err(|_| FormatError::InvalidPlan)?;
            output.push_str(&format!("OBJECT_NAME {} {}\n", object.id, name));
        }
    }

    output.push_str("\n# ASSET <id> <extension> <name>\n");
    for asset in assets {
        output.push_str(&format!(
            "ASSET {} {} {}\n",
            asset.id, asset.extension, asset.name
        ));
    }

    output.push_str("\n# IMAGE <id> <asset-id> <x> <y> <width> <height> <rotation> <locked>\n");
    for image in &plan.images {
        output.push_str(&format!(
            "IMAGE {} {} {} {} {} {} {} {}\n",
            image.id,
            image.asset_id,
            number(image.x),
            number(image.y),
            number(image.width),
            number(image.height),
            number(image.rotation),
            image.locked,
        ));
    }

    output.push_str(
        "\n# TEXT <id> <x> <y> <size> <rotation> <locked> <color> <bold> <JSON string>\n",
    );
    for text in &plan.texts {
        let content = serde_json::to_string(&text.text).map_err(|_| FormatError::InvalidPlan)?;
        output.push_str(&format!(
            "TEXT {} {} {} {} {} {} {} {} {}\n",
            text.id,
            number(text.x),
            number(text.y),
            number(text.size),
            number(text.rotation),
            text.locked,
            color_hex(text.color),
            text.bold,
            content,
        ));
    }

    output.push_str("\n# AREA <id> <locked> <fill-color> <stroke-color> <x1> <y1> <x2> <y2> ...\n");
    for area in &plan.areas {
        output.push_str(&format!(
            "AREA {} {} {} {}",
            area.id,
            area.locked,
            color_hex(area.fill_color),
            color_hex(area.stroke_color)
        ));
        for point in &area.points {
            output.push_str(&format!(" {} {}", number(point.x), number(point.y)));
        }
        output.push('\n');
    }

    output.push_str(
        "\n# ROOM <id> <source> <type> <locked> <fill-visible> <fill-color> <x1> <y1> ...\n",
    );
    for room in &plan.rooms {
        output.push_str(&format!(
            "ROOM {} {} {} {} {} {}",
            room.id,
            room.source.as_str(),
            room.kind.as_str(),
            room.locked,
            room.fill_visible,
            color_hex(room.fill_color),
        ));
        for point in &room.points {
            output.push_str(&format!(" {} {}", number(point.x), number(point.y)));
        }
        output.push('\n');
        if !room.name.is_empty() {
            let name = serde_json::to_string(&room.name).map_err(|_| FormatError::InvalidPlan)?;
            output.push_str(&format!("ROOM_NAME {} {}\n", room.id, name));
        }
    }

    output.push_str("\n# LAYER <id> <visible> <locked> <opacity> <JSON name>\n");
    for layer in &plan.layers {
        let name = serde_json::to_string(&layer.name).map_err(|_| FormatError::InvalidPlan)?;
        output.push_str(&format!(
            "LAYER {} {} {} {} {}\n",
            layer.id,
            layer.visible,
            layer.locked,
            number(layer.opacity),
            name,
        ));
    }
    output.push_str("\n# ASSIGN <item-id> <layer-id>\n");
    for assignment in &plan.layer_assignments {
        output.push_str(&format!(
            "ASSIGN {} {}\n",
            assignment.item_id, assignment.layer_id
        ));
    }

    output.push_str("\n# COMPASS <id> <x> <y> <size> <rotation> <locked> <color>\n");
    for compass in &plan.compasses {
        output.push_str(&format!(
            "COMPASS {} {} {} {} {} {} {}\n",
            compass.id,
            number(compass.x),
            number(compass.y),
            number(compass.size),
            number(compass.rotation),
            compass.locked,
            color_hex(compass.color),
        ));
    }

    output.push_str(
        "\n# ARROW <id> <x> <y> <length> <width> <style> <orientation-step> <locked> <color>\n",
    );
    for arrow in &plan.arrows {
        output.push_str(&format!(
            "ARROW {} {} {} {} {} {} {} {} {}\n",
            arrow.id,
            number(arrow.x),
            number(arrow.y),
            number(arrow.length),
            number(arrow.width),
            arrow.style.as_str(),
            arrow.orientation,
            arrow.locked,
            color_hex(arrow.color),
        ));
    }

    if output.len() > MAX_PLAN_BYTES {
        return Err(FormatError::TooLarge);
    }
    Ok(output.into_bytes())
}

pub fn encode_project(plan: &PlanFile, assets: &[ProjectAsset]) -> Result<Vec<u8>, FormatError> {
    if assets.len() > MAX_ASSETS {
        return Err(FormatError::TooLarge);
    }
    let plan_data = encode_plan_data(plan, assets)?;
    let expanded_size = assets
        .iter()
        .try_fold(plan_data.len(), |total, asset| {
            total.checked_add(asset.bytes.len())
        })
        .ok_or(FormatError::TooLarge)?;
    if expanded_size > MAX_PROJECT_BYTES {
        return Err(FormatError::TooLarge);
    }
    let cursor = Cursor::new(Vec::new());
    let mut archive = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    archive
        .start_file("plan.data", options)
        .map_err(|_| FormatError::Corrupted)?;
    archive
        .write_all(&plan_data)
        .map_err(|_| FormatError::Corrupted)?;

    for asset in assets {
        if asset.bytes.len() > MAX_ASSET_BYTES {
            return Err(FormatError::TooLarge);
        }
        let path = format!("assets/{}.{}", asset.id, asset.extension);
        let compression = if asset.extension == "bmp" {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        let asset_options = zip::write::SimpleFileOptions::default()
            .compression_method(compression)
            .unix_permissions(0o644);
        archive
            .start_file(path, asset_options)
            .map_err(|_| FormatError::Corrupted)?;
        archive
            .write_all(&asset.bytes)
            .map_err(|_| FormatError::Corrupted)?;
    }

    let bytes = archive
        .finish()
        .map_err(|_| FormatError::Corrupted)?
        .into_inner();
    if bytes.len() > MAX_PROJECT_BYTES {
        return Err(FormatError::TooLarge);
    }
    Ok(bytes)
}

pub fn decode_plan(bytes: &[u8]) -> Result<DecodedPlan, FormatError> {
    if bytes.len() > MAX_PROJECT_BYTES {
        return Err(FormatError::TooLarge);
    }
    if bytes.starts_with(CONTAINER_MAGIC) {
        return decode_container(bytes);
    }
    if bytes.len() > MAX_PLAN_BYTES {
        return Err(FormatError::TooLarge);
    }

    if bytes.starts_with(BINARY_MAGIC) {
        return decode_legacy_binary(bytes).map(|plan| DecodedPlan {
            plan,
            assets: Vec::new(),
            needs_conversion: true,
        });
    }

    if bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'{')
    {
        let plan = serde_json::from_slice::<PlanFile>(bytes).map_err(|_| FormatError::Corrupted)?;
        validate_plan(&plan)?;
        validate_assets(&plan, &[])?;
        return Ok(DecodedPlan {
            plan,
            assets: Vec::new(),
            needs_conversion: true,
        });
    }

    let text = str::from_utf8(bytes).map_err(|_| FormatError::UnknownFormat)?;
    let (plan, descriptors) = decode_text(text)?;
    if !descriptors.is_empty() || !plan.images.is_empty() {
        return Err(FormatError::InvalidPlan);
    }
    Ok(DecodedPlan {
        plan,
        assets: Vec::new(),
        needs_conversion: true,
    })
}

fn decode_text(text: &str) -> Result<(PlanFile, Vec<AssetDescriptor>), FormatError> {
    let mut plan = PlanFile::default();
    let mut assets = Vec::new();
    let mut header_seen = false;
    let mut units_seen = false;
    let mut unit_scale = 1.0_f32;

    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = raw_line.trim();
        let line = if trimmed.starts_with("TEXT ")
            || trimmed.starts_with("OBJECT_NAME ")
            || trimmed.starts_with("ROOM_NAME ")
            || trimmed.starts_with("LAYER ")
        {
            trimmed
        } else {
            trimmed.split('#').next().unwrap_or_default().trim()
        };
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line == TEXT_HEADER || line == LEGACY_TEXT_HEADER {
                header_seen = true;
                continue;
            }
            if let Some(version) = line.strip_prefix("RPP-PLAN ") {
                let version = version.parse::<u16>().unwrap_or(u16::MAX);
                return Err(FormatError::UnsupportedVersion(version));
            }
            return Err(FormatError::UnknownFormat);
        }

        if line.starts_with("TEXT ") {
            let Some(content_start) = line.find('"') else {
                return syntax(line_number, "TEXT content must be a JSON string");
            };
            let metadata: Vec<&str> = line[..content_start].split_whitespace().collect();
            let (id, x, y, size, rotation, locked, color, bold) = match metadata.as_slice() {
                ["TEXT", id, x, y, size, rotation, locked] => (
                    *id,
                    *x,
                    *y,
                    *size,
                    *rotation,
                    *locked,
                    default_text_color(),
                    false,
                ),
                ["TEXT", id, x, y, size, rotation, locked, color] => (
                    *id,
                    *x,
                    *y,
                    *size,
                    *rotation,
                    *locked,
                    parse_color(color, line_number)?,
                    false,
                ),
                ["TEXT", id, x, y, size, rotation, locked, color, bold] => (
                    *id,
                    *x,
                    *y,
                    *size,
                    *rotation,
                    *locked,
                    parse_color(color, line_number)?,
                    parse_bool(bold, line_number)?,
                ),
                _ => {
                    return syntax(
                        line_number,
                        "expected syntax: TEXT <id> <x> <y> <size> <rotation> <locked> <color> <bold> <JSON string>",
                    );
                }
            };
            let content = serde_json::from_str::<String>(&line[content_start..]).map_err(|_| {
                FormatError::Syntax {
                    line: line_number,
                    message: "invalid TEXT JSON string".to_owned(),
                }
            })?;
            plan.texts.push(TextObject {
                id: parse_id(id, line_number)?,
                text: content,
                x: parse_number(x, line_number)?,
                y: parse_number(y, line_number)?,
                size: parse_number(size, line_number)?,
                rotation: parse_number(rotation, line_number)?,
                color,
                bold,
                locked: parse_bool(locked, line_number)?,
            });
            continue;
        }

        if line.starts_with("OBJECT_NAME ") {
            let Some(content_start) = line.find('"') else {
                return syntax(line_number, "OBJECT_NAME content must be a JSON string");
            };
            let metadata: Vec<&str> = line[..content_start].split_whitespace().collect();
            let ["OBJECT_NAME", id] = metadata.as_slice() else {
                return syntax(
                    line_number,
                    "expected syntax: OBJECT_NAME <object-id> <JSON string>",
                );
            };
            let name = serde_json::from_str::<String>(&line[content_start..]).map_err(|_| {
                FormatError::Syntax {
                    line: line_number,
                    message: "invalid OBJECT_NAME JSON string".to_owned(),
                }
            })?;
            let Some(object) = plan.objects.iter_mut().find(|object| object.id == *id) else {
                return syntax(line_number, "OBJECT_NAME references an unknown object");
            };
            object.name = name;
            continue;
        }

        if line.starts_with("ROOM_NAME ") {
            let Some(content_start) = line.find('"') else {
                return syntax(line_number, "ROOM_NAME content must be a JSON string");
            };
            let metadata: Vec<&str> = line[..content_start].split_whitespace().collect();
            let ["ROOM_NAME", id] = metadata.as_slice() else {
                return syntax(
                    line_number,
                    "expected syntax: ROOM_NAME <room-id> <JSON string>",
                );
            };
            let name = serde_json::from_str::<String>(&line[content_start..]).map_err(|_| {
                FormatError::Syntax {
                    line: line_number,
                    message: "invalid ROOM_NAME JSON string".to_owned(),
                }
            })?;
            let Some(room) = plan.rooms.iter_mut().find(|room| room.id == *id) else {
                return syntax(line_number, "ROOM_NAME references an unknown room");
            };
            room.name = name;
            continue;
        }

        if line.starts_with("LAYER ") {
            let Some(content_start) = line.find('"') else {
                return syntax(line_number, "LAYER name must be a JSON string");
            };
            let metadata: Vec<&str> = line[..content_start].split_whitespace().collect();
            let ["LAYER", id, visible, locked, opacity] = metadata.as_slice() else {
                return syntax(
                    line_number,
                    "expected syntax: LAYER <id> <visible> <locked> <opacity> <JSON name>",
                );
            };
            let layer = Layer {
                id: parse_id(id, line_number)?,
                name: serde_json::from_str::<String>(&line[content_start..]).map_err(|_| {
                    FormatError::Syntax {
                        line: line_number,
                        message: "invalid LAYER JSON string".to_owned(),
                    }
                })?,
                visible: parse_bool(visible, line_number)?,
                locked: parse_bool(locked, line_number)?,
                opacity: parse_number(opacity, line_number)?,
            };
            if let Some(existing) = plan.layers.iter_mut().find(|item| item.id == layer.id) {
                *existing = layer;
            } else {
                plan.layers.push(layer);
            }
            continue;
        }

        if line.starts_with("AREA ") {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 11 || !(fields.len() - 5).is_multiple_of(2) {
                return syntax(
                    line_number,
                    "expected syntax: AREA <id> <locked> <fill-color> <stroke-color> <x1> <y1> <x2> <y2> <x3> <y3> ...",
                );
            }
            let mut points = Vec::with_capacity((fields.len() - 5) / 2);
            for pair in fields[5..].as_chunks::<2>().0 {
                points.push(Point::new(
                    parse_number(pair[0], line_number)?,
                    parse_number(pair[1], line_number)?,
                ));
            }
            plan.areas.push(AreaShape {
                id: parse_id(fields[1], line_number)?,
                locked: parse_bool(fields[2], line_number)?,
                fill_color: parse_color(fields[3], line_number)?,
                stroke_color: parse_color(fields[4], line_number)?,
                points,
            });
            continue;
        }

        if line.starts_with("ROOM ") {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 13 || !(fields.len() - 7).is_multiple_of(2) {
                return syntax(
                    line_number,
                    "expected syntax: ROOM <id> <source> <type> <locked> <fill-visible> <fill-color> <x1> <y1> <x2> <y2> <x3> <y3> ...",
                );
            }
            let mut points = Vec::with_capacity((fields.len() - 7) / 2);
            for pair in fields[7..].as_chunks::<2>().0 {
                points.push(Point::new(
                    parse_number(pair[0], line_number)?,
                    parse_number(pair[1], line_number)?,
                ));
            }
            plan.rooms.push(Room {
                id: parse_id(fields[1], line_number)?,
                name: String::new(),
                source: parse_room_source(fields[2], line_number)?,
                kind: parse_room_kind(fields[3], line_number)?,
                locked: parse_bool(fields[4], line_number)?,
                fill_visible: parse_bool(fields[5], line_number)?,
                fill_color: parse_color(fields[6], line_number)?,
                points,
            });
            continue;
        }

        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.as_slice() {
            ["UNITS", unit] if !units_seen => {
                unit_scale = match *unit {
                    "cm" => 1.0,
                    "mm" => 0.1,
                    "m" => 100.0,
                    "in" => 2.54,
                    "ft" => 30.48,
                    _ => {
                        return syntax(line_number, "supported units are cm, mm, m, in, and ft");
                    }
                };
                units_seen = true;
            }
            ["ASSIGN", item_id, layer_id] => {
                plan.layer_assignments.push(LayerAssignment {
                    item_id: parse_id(item_id, line_number)?,
                    layer_id: parse_id(layer_id, line_number)?,
                });
            }
            ["WALL", id, x1, y1, x2, y2] => {
                plan.walls.push(Wall {
                    id: parse_id(id, line_number)?,
                    x1: parse_number(x1, line_number)?,
                    y1: parse_number(y1, line_number)?,
                    x2: parse_number(x2, line_number)?,
                    y2: parse_number(y2, line_number)?,
                    thickness: default_wall_thickness(),
                    kind: WallKind::default(),
                    color: default_wall_color(),
                    locked: false,
                });
            }
            ["WALL", id, x1, y1, x2, y2, thickness, kind, locked] => {
                plan.walls.push(Wall {
                    id: parse_id(id, line_number)?,
                    x1: parse_number(x1, line_number)?,
                    y1: parse_number(y1, line_number)?,
                    x2: parse_number(x2, line_number)?,
                    y2: parse_number(y2, line_number)?,
                    thickness: parse_number(thickness, line_number)?,
                    kind: parse_wall_kind(kind, line_number)?,
                    color: default_wall_color(),
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["WALL", id, x1, y1, x2, y2, thickness, kind, locked, color] => {
                plan.walls.push(Wall {
                    id: parse_id(id, line_number)?,
                    x1: parse_number(x1, line_number)?,
                    y1: parse_number(y1, line_number)?,
                    x2: parse_number(x2, line_number)?,
                    y2: parse_number(y2, line_number)?,
                    thickness: parse_number(thickness, line_number)?,
                    kind: parse_wall_kind(kind, line_number)?,
                    color: parse_color(color, line_number)?,
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["LINE", id, x1, y1, x2, y2, locked] => {
                plan.lines.push(PlanLine {
                    id: parse_id(id, line_number)?,
                    x1: parse_number(x1, line_number)?,
                    y1: parse_number(y1, line_number)?,
                    x2: parse_number(x2, line_number)?,
                    y2: parse_number(y2, line_number)?,
                    color: default_line_color(),
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["LINE", id, x1, y1, x2, y2, locked, color] => {
                plan.lines.push(PlanLine {
                    id: parse_id(id, line_number)?,
                    x1: parse_number(x1, line_number)?,
                    y1: parse_number(y1, line_number)?,
                    x2: parse_number(x2, line_number)?,
                    y2: parse_number(y2, line_number)?,
                    color: parse_color(color, line_number)?,
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["DOOR", id, wall_id, position, width] => {
                plan.doors.push(Door {
                    id: parse_id(id, line_number)?,
                    wall_id: parse_id(wall_id, line_number)?,
                    t: parse_number(position, line_number)?,
                    width: parse_number(width, line_number)?,
                    kind: DoorKind::default(),
                    hinge: HingeSide::default(),
                    swing: SwingSide::default(),
                    color: default_opening_color(),
                    locked: false,
                });
            }
            [
                "DOOR",
                id,
                wall_id,
                position,
                width,
                kind,
                hinge,
                swing,
                locked,
            ] => {
                plan.doors.push(Door {
                    id: parse_id(id, line_number)?,
                    wall_id: parse_id(wall_id, line_number)?,
                    t: parse_number(position, line_number)?,
                    width: parse_number(width, line_number)?,
                    kind: parse_door_kind(kind, line_number)?,
                    hinge: parse_hinge_side(hinge, line_number)?,
                    swing: parse_swing_side(swing, line_number)?,
                    color: default_opening_color(),
                    locked: parse_bool(locked, line_number)?,
                });
            }
            [
                "DOOR",
                id,
                wall_id,
                position,
                width,
                kind,
                hinge,
                swing,
                locked,
                color,
            ] => {
                plan.doors.push(Door {
                    id: parse_id(id, line_number)?,
                    wall_id: parse_id(wall_id, line_number)?,
                    t: parse_number(position, line_number)?,
                    width: parse_number(width, line_number)?,
                    kind: parse_door_kind(kind, line_number)?,
                    hinge: parse_hinge_side(hinge, line_number)?,
                    swing: parse_swing_side(swing, line_number)?,
                    color: parse_color(color, line_number)?,
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["WINDOW", id, wall_id, position, width, kind, locked] => {
                plan.windows.push(Window {
                    id: parse_id(id, line_number)?,
                    wall_id: parse_id(wall_id, line_number)?,
                    t: parse_number(position, line_number)?,
                    width: parse_number(width, line_number)?,
                    kind: parse_window_kind(kind, line_number)?,
                    color: default_window_color(),
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["WINDOW", id, wall_id, position, width, kind, locked, color] => {
                plan.windows.push(Window {
                    id: parse_id(id, line_number)?,
                    wall_id: parse_id(wall_id, line_number)?,
                    t: parse_number(position, line_number)?,
                    width: parse_number(width, line_number)?,
                    kind: parse_window_kind(kind, line_number)?,
                    color: parse_color(color, line_number)?,
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["OBJECT", id, kind, x, y, width, height, rotation, locked] => {
                plan.objects.push(PlanObject {
                    id: parse_id(id, line_number)?,
                    kind: parse_object_kind(kind, line_number)?,
                    name: String::new(),
                    x: parse_number(x, line_number)?,
                    y: parse_number(y, line_number)?,
                    width: parse_number(width, line_number)?,
                    height: parse_number(height, line_number)?,
                    rotation: parse_number(rotation, line_number)?,
                    fill_color: default_object_fill_color(),
                    stroke_color: default_object_stroke_color(),
                    locked: parse_bool(locked, line_number)?,
                });
            }
            [
                "OBJECT",
                id,
                kind,
                x,
                y,
                width,
                height,
                rotation,
                locked,
                fill_color,
                stroke_color,
            ] => {
                plan.objects.push(PlanObject {
                    id: parse_id(id, line_number)?,
                    kind: parse_object_kind(kind, line_number)?,
                    name: String::new(),
                    x: parse_number(x, line_number)?,
                    y: parse_number(y, line_number)?,
                    width: parse_number(width, line_number)?,
                    height: parse_number(height, line_number)?,
                    rotation: parse_number(rotation, line_number)?,
                    fill_color: parse_color(fill_color, line_number)?,
                    stroke_color: parse_color(stroke_color, line_number)?,
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["ASSET", id, extension, name] => {
                assets.push(AssetDescriptor {
                    id: parse_id(id, line_number)?,
                    extension: parse_extension(extension, line_number)?,
                    name: parse_id(name, line_number)?,
                });
            }
            ["IMAGE", id, asset_id, x, y, width, height, rotation, locked] => {
                plan.images.push(ImageObject {
                    id: parse_id(id, line_number)?,
                    asset_id: parse_id(asset_id, line_number)?,
                    x: parse_number(x, line_number)?,
                    y: parse_number(y, line_number)?,
                    width: parse_number(width, line_number)?,
                    height: parse_number(height, line_number)?,
                    rotation: parse_number(rotation, line_number)?,
                    locked: parse_bool(locked, line_number)?,
                });
            }
            ["COMPASS", id, x, y, size, rotation, locked, color] => {
                plan.compasses.push(CompassRose {
                    id: parse_id(id, line_number)?,
                    x: parse_number(x, line_number)?,
                    y: parse_number(y, line_number)?,
                    size: parse_number(size, line_number)?,
                    rotation: parse_number(rotation, line_number)?,
                    locked: parse_bool(locked, line_number)?,
                    color: parse_color(color, line_number)?,
                });
            }
            [
                "ARROW",
                id,
                x,
                y,
                length,
                width,
                style,
                orientation,
                locked,
                color,
            ] => {
                plan.arrows.push(PlanArrow {
                    id: parse_id(id, line_number)?,
                    x: parse_number(x, line_number)?,
                    y: parse_number(y, line_number)?,
                    length: parse_number(length, line_number)?,
                    width: parse_number(width, line_number)?,
                    style: parse_arrow_style(style, line_number)?,
                    orientation: orientation.parse::<u8>().map_err(|_| FormatError::Syntax {
                        line: line_number,
                        message: "arrow orientation must be an integer from 0 to 63".to_owned(),
                    })?,
                    locked: parse_bool(locked, line_number)?,
                    color: parse_color(color, line_number)?,
                });
            }
            ["WALL", ..] => {
                return syntax(
                    line_number,
                    "expected syntax: WALL <id> <x1> <y1> <x2> <y2>",
                );
            }
            ["LINE", ..] => {
                return syntax(
                    line_number,
                    "expected syntax: LINE <id> <x1> <y1> <x2> <y2> <locked> <color>",
                );
            }
            ["DOOR", ..] => {
                return syntax(
                    line_number,
                    "expected syntax: DOOR <id> <wall-id> <position> <width>",
                );
            }
            ["WINDOW", ..] => {
                return syntax(
                    line_number,
                    "expected syntax: WINDOW <id> <wall-id> <position> <width> <type> <locked>",
                );
            }
            ["OBJECT", ..] => {
                return syntax(
                    line_number,
                    "expected syntax: OBJECT <id> <type> <x> <y> <width> <height> <rotation> <locked> <fill-color> <stroke-color>",
                );
            }
            ["ASSET", ..] => {
                return syntax(
                    line_number,
                    "expected syntax: ASSET <id> <extension> <name>",
                );
            }
            ["IMAGE", ..] => {
                return syntax(
                    line_number,
                    "expected syntax: IMAGE <id> <asset-id> <x> <y> <width> <height> <rotation> <locked>",
                );
            }
            ["ARROW", ..] => {
                return syntax(
                    line_number,
                    "expected syntax: ARROW <id> <x> <y> <length> <width> <style> <orientation-step> <locked> <color>",
                );
            }
            ["ROOM", ..] => {
                return syntax(line_number, "invalid ROOM instruction");
            }
            ["LAYER", ..] => {
                return syntax(line_number, "invalid LAYER instruction");
            }
            ["ASSIGN", ..] => {
                return syntax(line_number, "expected syntax: ASSIGN <item-id> <layer-id>");
            }
            _ => return syntax(line_number, "unknown RPP instruction"),
        }
    }

    if !header_seen {
        return Err(FormatError::UnknownFormat);
    }
    if !units_seen {
        return syntax(1, "a UNITS line is required");
    }
    scale_plan(&mut plan, unit_scale);
    validate_plan(&plan)?;
    Ok((plan, assets))
}

fn decode_container(bytes: &[u8]) -> Result<DecodedPlan, FormatError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|_| FormatError::Corrupted)?;
    if archive.len() > MAX_ASSETS + 1 {
        return Err(FormatError::TooLarge);
    }

    let mut total_unpacked = 0_u64;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|_| FormatError::Corrupted)?;
        total_unpacked = total_unpacked
            .checked_add(file.size())
            .ok_or(FormatError::TooLarge)?;
        if total_unpacked > MAX_PROJECT_BYTES as u64 {
            return Err(FormatError::TooLarge);
        }
    }

    let plan_data = {
        let file = archive
            .by_name("plan.data")
            .map_err(|_| FormatError::InvalidHeader)?;
        if file.size() as usize > MAX_PLAN_BYTES {
            return Err(FormatError::TooLarge);
        }
        let mut data = Vec::with_capacity(file.size() as usize);
        file.take(MAX_PLAN_BYTES as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| FormatError::Corrupted)?;
        if data.len() > MAX_PLAN_BYTES {
            return Err(FormatError::TooLarge);
        }
        data
    };
    let text = str::from_utf8(&plan_data).map_err(|_| FormatError::Corrupted)?;
    let (plan, descriptors) = decode_text(text)?;
    if descriptors.len() > MAX_ASSETS {
        return Err(FormatError::TooLarge);
    }

    let mut assets = Vec::with_capacity(descriptors.len());
    let mut descriptor_ids = HashSet::new();
    for descriptor in descriptors {
        if !descriptor_ids.insert(descriptor.id.clone()) {
            return Err(FormatError::InvalidPlan);
        }
        let path = format!("assets/{}.{}", descriptor.id, descriptor.extension);
        let file = archive
            .by_name(&path)
            .map_err(|_| FormatError::InvalidPlan)?;
        if file.size() as usize > MAX_ASSET_BYTES {
            return Err(FormatError::TooLarge);
        }
        let mut data = Vec::with_capacity(file.size() as usize);
        file.take(MAX_ASSET_BYTES as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| FormatError::Corrupted)?;
        if data.len() > MAX_ASSET_BYTES {
            return Err(FormatError::TooLarge);
        }
        assets.push(ProjectAsset {
            id: descriptor.id,
            name: descriptor.name,
            extension: descriptor.extension,
            bytes: data,
        });
    }
    validate_assets(&plan, &assets)?;
    Ok(DecodedPlan {
        plan,
        assets,
        needs_conversion: false,
    })
}

fn decode_legacy_binary(bytes: &[u8]) -> Result<PlanFile, FormatError> {
    if bytes.len() < BINARY_HEADER_LEN {
        return Err(FormatError::InvalidHeader);
    }

    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != BINARY_VERSION {
        return Err(FormatError::UnsupportedVersion(version));
    }

    let serialized_len = u32::from_le_bytes(
        bytes[10..14]
            .try_into()
            .map_err(|_| FormatError::InvalidHeader)?,
    ) as usize;
    let compressed_len = u32::from_le_bytes(
        bytes[14..18]
            .try_into()
            .map_err(|_| FormatError::InvalidHeader)?,
    ) as usize;

    if serialized_len > MAX_PLAN_BYTES || compressed_len > MAX_PLAN_BYTES {
        return Err(FormatError::TooLarge);
    }
    if bytes.len() != BINARY_HEADER_LEN + compressed_len {
        return Err(FormatError::InvalidHeader);
    }

    let serialized = decompress_to_vec_zlib_with_limit(&bytes[BINARY_HEADER_LEN..], serialized_len)
        .map_err(|_| FormatError::Corrupted)?;
    if serialized.len() != serialized_len {
        return Err(FormatError::Corrupted);
    }

    let legacy =
        postcard::from_bytes::<LegacyPlanFile>(&serialized).map_err(|_| FormatError::Corrupted)?;
    let plan = legacy.into_current();
    validate_plan(&plan)?;
    Ok(plan)
}

pub fn validate_plan(plan: &PlanFile) -> Result<(), FormatError> {
    if plan.version != 1
        || plan.walls.len() > 100_000
        || plan.lines.len() > 100_000
        || plan.doors.len() > 100_000
        || plan.windows.len() > 100_000
        || plan.objects.len() > 100_000
        || plan.images.len() > 100_000
        || plan.texts.len() > 100_000
        || plan.areas.len() > 100_000
        || plan.compasses.len() > 100_000
        || plan.arrows.len() > 100_000
        || plan.rooms.len() > 100_000
        || plan.layers.is_empty()
        || plan.layers.len() > 1_000
        || plan.layer_assignments.len() > 500_000
    {
        return Err(FormatError::InvalidPlan);
    }

    let mut object_ids = HashSet::with_capacity(
        plan.walls.len()
            + plan.lines.len()
            + plan.doors.len()
            + plan.windows.len()
            + plan.objects.len()
            + plan.images.len()
            + plan.texts.len()
            + plan.areas.len()
            + plan.compasses.len()
            + plan.arrows.len()
            + plan.rooms.len(),
    );
    let mut wall_ids = HashSet::with_capacity(plan.walls.len());
    let mut wall_lengths = HashMap::with_capacity(plan.walls.len());
    for wall in &plan.walls {
        let length = wall.length();
        if !valid_id(&wall.id)
            || !object_ids.insert(wall.id.as_str())
            || !wall_ids.insert(wall.id.as_str())
            || !wall.x1.is_finite()
            || !wall.y1.is_finite()
            || !wall.x2.is_finite()
            || !wall.y2.is_finite()
            || !wall.thickness.is_finite()
            || wall.thickness <= 0.0
            || !length.is_finite()
            || length <= 0.0
            || [wall.x1, wall.y1, wall.x2, wall.y2, wall.thickness]
                .iter()
                .any(|value| value.abs() > 1_000_000_000.0)
        {
            return Err(FormatError::InvalidPlan);
        }
        wall_lengths.insert(wall.id.as_str(), length);
    }

    for line in &plan.lines {
        let length = line.length();
        if !valid_id(&line.id)
            || !object_ids.insert(line.id.as_str())
            || !line.x1.is_finite()
            || !line.y1.is_finite()
            || !line.x2.is_finite()
            || !line.y2.is_finite()
            || !length.is_finite()
            || length <= 0.0
            || [line.x1, line.y1, line.x2, line.y2]
                .iter()
                .any(|value| value.abs() > 1_000_000_000.0)
        {
            return Err(FormatError::InvalidPlan);
        }
    }

    for door in &plan.doors {
        let wall_length = wall_lengths.get(door.wall_id.as_str()).copied();
        if !valid_id(&door.id)
            || !object_ids.insert(door.id.as_str())
            || !wall_ids.contains(door.wall_id.as_str())
            || !door.t.is_finite()
            || !(0.0..=1.0).contains(&door.t)
            || !door.width.is_finite()
            || door.width <= 0.0
            || door.width > 1_000_000_000.0
            || wall_length.is_none_or(|length| {
                let margin = door.width / (2.0 * length);
                door.width > length || door.t < margin || door.t > 1.0 - margin
            })
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    for window in &plan.windows {
        let wall_length = wall_lengths.get(window.wall_id.as_str()).copied();
        if !valid_id(&window.id)
            || !object_ids.insert(window.id.as_str())
            || !wall_ids.contains(window.wall_id.as_str())
            || !window.t.is_finite()
            || !(0.0..=1.0).contains(&window.t)
            || !window.width.is_finite()
            || window.width <= 0.0
            || window.width > 1_000_000_000.0
            || wall_length.is_none_or(|length| {
                let margin = window.width / (2.0 * length);
                window.width > length || window.t < margin || window.t > 1.0 - margin
            })
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    for object in &plan.objects {
        if !valid_id(&object.id)
            || !object_ids.insert(object.id.as_str())
            || !object.x.is_finite()
            || !object.y.is_finite()
            || !object.width.is_finite()
            || !object.height.is_finite()
            || !object.rotation.is_finite()
            || object.name.len() > 512
            || object.width <= 0.0
            || object.height <= 0.0
            || [object.x, object.y, object.width, object.height]
                .iter()
                .any(|value| value.abs() > 1_000_000_000.0)
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    for image in &plan.images {
        if !valid_id(&image.id)
            || !valid_id(&image.asset_id)
            || !object_ids.insert(image.id.as_str())
            || !image.x.is_finite()
            || !image.y.is_finite()
            || !image.width.is_finite()
            || !image.height.is_finite()
            || !image.rotation.is_finite()
            || image.width <= 0.0
            || image.height <= 0.0
            || [image.x, image.y, image.width, image.height]
                .iter()
                .any(|value| value.abs() > 1_000_000_000.0)
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    for text in &plan.texts {
        if !valid_id(&text.id)
            || !object_ids.insert(text.id.as_str())
            || text.text.len() > 10_000
            || !text.x.is_finite()
            || !text.y.is_finite()
            || !text.size.is_finite()
            || !text.rotation.is_finite()
            || !(1.0..=10_000.0).contains(&text.size)
            || [text.x, text.y, text.size]
                .iter()
                .any(|value| value.abs() > 1_000_000_000.0)
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    for area in &plan.areas {
        if !valid_id(&area.id)
            || !object_ids.insert(area.id.as_str())
            || !(3..=10_000).contains(&area.points.len())
            || area.points.iter().any(|point| {
                !point.x.is_finite()
                    || !point.y.is_finite()
                    || point.x.abs() > 1_000_000_000.0
                    || point.y.abs() > 1_000_000_000.0
            })
            || !area.area_square_centimeters().is_finite()
            || area.area_square_centimeters() <= 0.0
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    for compass in &plan.compasses {
        if !valid_id(&compass.id)
            || !object_ids.insert(compass.id.as_str())
            || !compass.x.is_finite()
            || !compass.y.is_finite()
            || !compass.size.is_finite()
            || !compass.rotation.is_finite()
            || compass.x.abs() > 1_000_000_000.0
            || compass.y.abs() > 1_000_000_000.0
            || !(1.0..=100_000.0).contains(&compass.size)
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    for arrow in &plan.arrows {
        if !valid_id(&arrow.id)
            || !object_ids.insert(arrow.id.as_str())
            || !arrow.x.is_finite()
            || !arrow.y.is_finite()
            || !arrow.length.is_finite()
            || !arrow.width.is_finite()
            || arrow.x.abs() > 1_000_000_000.0
            || arrow.y.abs() > 1_000_000_000.0
            || !(1.0..=1_000_000.0).contains(&arrow.length)
            || !(0.1..=1_000_000.0).contains(&arrow.width)
            || arrow.orientation >= PlanArrow::ORIENTATION_COUNT
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    for room in &plan.rooms {
        if !valid_id(&room.id)
            || !object_ids.insert(room.id.as_str())
            || room.name.len() > 512
            || !(3..=10_000).contains(&room.points.len())
            || room.points.iter().any(|point| {
                !point.x.is_finite()
                    || !point.y.is_finite()
                    || point.x.abs() > 1_000_000_000.0
                    || point.y.abs() > 1_000_000_000.0
            })
            || !room.area_square_centimeters().is_finite()
            || room.area_square_centimeters() <= 0.0
            || !room.perimeter_centimeters().is_finite()
        {
            return Err(FormatError::InvalidPlan);
        }
    }

    let mut layer_ids = HashSet::with_capacity(plan.layers.len());
    for layer in &plan.layers {
        if !valid_id(&layer.id)
            || !layer_ids.insert(layer.id.as_str())
            || layer.name.is_empty()
            || layer.name.len() > 256
            || !layer.opacity.is_finite()
            || !(0.0..=1.0).contains(&layer.opacity)
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    if !layer_ids.contains(crate::model::DEFAULT_LAYER_ID) {
        return Err(FormatError::InvalidPlan);
    }
    let mut assigned_items = HashSet::with_capacity(plan.layer_assignments.len());
    for assignment in &plan.layer_assignments {
        if !object_ids.contains(assignment.item_id.as_str())
            || !layer_ids.contains(assignment.layer_id.as_str())
            || !assigned_items.insert(assignment.item_id.as_str())
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    Ok(())
}

fn validate_assets(plan: &PlanFile, assets: &[ProjectAsset]) -> Result<(), FormatError> {
    if assets.len() > MAX_ASSETS {
        return Err(FormatError::TooLarge);
    }
    let mut asset_ids = HashSet::with_capacity(assets.len());
    let mut decoded_bytes = 0_u64;
    for asset in assets {
        decoded_bytes = decoded_bytes
            .checked_add(
                crate::images::decoded_byte_len(&asset.bytes).ok_or(FormatError::InvalidPlan)?,
            )
            .ok_or(FormatError::TooLarge)?;
        if decoded_bytes > crate::images::MAX_DECODED_BYTES {
            return Err(FormatError::TooLarge);
        }
        if !valid_id(&asset.id)
            || !valid_id(&asset.name)
            || !valid_extension(&asset.extension)
            || asset.bytes.is_empty()
            || asset.bytes.len() > MAX_ASSET_BYTES
            || !asset_ids.insert(asset.id.as_str())
            || !valid_image_bytes(&asset.bytes)
        {
            return Err(FormatError::InvalidPlan);
        }
    }
    if plan.images.iter().any(|image| {
        !asset_ids.contains(image.asset_id.as_str()) && image.asset_id != "builtin-emergency-exit"
    }) {
        return Err(FormatError::InvalidPlan);
    }
    Ok(())
}

fn number(value: f32) -> String {
    value.to_string()
}

fn color_hex(color: [u8; 3]) -> String {
    format!("{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

fn parse_color(value: &str, line: usize) -> Result<[u8; 3], FormatError> {
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return syntax(
            line,
            "a color must use six hexadecimal digits such as 4b4b4b",
        );
    }
    Ok([
        u8::from_str_radix(&value[0..2], 16).unwrap_or_default(),
        u8::from_str_radix(&value[2..4], 16).unwrap_or_default(),
        u8::from_str_radix(&value[4..6], 16).unwrap_or_default(),
    ])
}

fn parse_number(value: &str, line: usize) -> Result<f32, FormatError> {
    let number = f32::from_str(value).map_err(|_| FormatError::Syntax {
        line,
        message: format!("\"{value}\" is not a number"),
    })?;
    if !number.is_finite() {
        return syntax(line, "infinite values and NaN are not allowed");
    }
    Ok(number)
}

fn parse_id(value: &str, line: usize) -> Result<String, FormatError> {
    if valid_id(value) {
        Ok(value.to_owned())
    } else {
        syntax(
            line,
            "an identifier may only contain letters, digits, dots, hyphens, or underscores",
        )
    }
}

fn parse_bool(value: &str, line: usize) -> Result<bool, FormatError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => syntax(line, "a boolean must be true or false"),
    }
}

fn parse_extension(value: &str, line: usize) -> Result<String, FormatError> {
    if valid_extension(value) {
        Ok(value.to_owned())
    } else {
        syntax(
            line,
            "an image extension must be png, jpg, jpeg, gif, bmp, or webp",
        )
    }
}

fn parse_wall_kind(value: &str, line: usize) -> Result<WallKind, FormatError> {
    match value {
        "interior" => Ok(WallKind::Interior),
        "exterior" => Ok(WallKind::Exterior),
        _ => syntax(line, "a wall type must be interior or exterior"),
    }
}

fn parse_door_kind(value: &str, line: usize) -> Result<DoorKind, FormatError> {
    match value {
        "single" => Ok(DoorKind::Single),
        "double" => Ok(DoorKind::Double),
        "sliding" => Ok(DoorKind::Sliding),
        _ => syntax(line, "a door type must be single, double, or sliding"),
    }
}

fn parse_window_kind(value: &str, line: usize) -> Result<WindowKind, FormatError> {
    match value {
        "standard" => Ok(WindowKind::Standard),
        "bay" => Ok(WindowKind::Bay),
        _ => syntax(line, "a window type must be standard or bay"),
    }
}

fn parse_object_kind(value: &str, line: usize) -> Result<ObjectKind, FormatError> {
    match value {
        "rectangle" => Ok(ObjectKind::Rectangle),
        "bed" => Ok(ObjectKind::Bed),
        "table" => Ok(ObjectKind::Table),
        "sofa" => Ok(ObjectKind::Sofa),
        "wardrobe" => Ok(ObjectKind::Wardrobe),
        "desk" => Ok(ObjectKind::Desk),
        _ => syntax(
            line,
            "an object type must be rectangle, bed, table, sofa, wardrobe, or desk",
        ),
    }
}

fn parse_room_source(value: &str, line: usize) -> Result<RoomSource, FormatError> {
    match value {
        "automatic" => Ok(RoomSource::Automatic),
        "manual" => Ok(RoomSource::Manual),
        _ => syntax(line, "room source must be automatic or manual"),
    }
}

fn parse_room_kind(value: &str, line: usize) -> Result<RoomKind, FormatError> {
    match value {
        "custom" => Ok(RoomKind::Custom),
        "living-room" => Ok(RoomKind::LivingRoom),
        "bedroom" => Ok(RoomKind::Bedroom),
        "kitchen" => Ok(RoomKind::Kitchen),
        "bathroom" => Ok(RoomKind::Bathroom),
        "hallway" => Ok(RoomKind::Hallway),
        "office" => Ok(RoomKind::Office),
        "garage" => Ok(RoomKind::Garage),
        "storage" => Ok(RoomKind::Storage),
        _ => syntax(line, "unknown room type"),
    }
}

fn parse_arrow_style(value: &str, line: usize) -> Result<ArrowStyle, FormatError> {
    match value {
        "open" => Ok(ArrowStyle::Open),
        "open-bold" => Ok(ArrowStyle::OpenBold),
        "hollow" => Ok(ArrowStyle::Hollow),
        "solid" => Ok(ArrowStyle::Solid),
        _ => syntax(
            line,
            "an arrow style must be open, open-bold, hollow, or solid",
        ),
    }
}

fn scale_plan(plan: &mut PlanFile, factor: f32) {
    if (factor - 1.0).abs() < f32::EPSILON {
        return;
    }
    for wall in &mut plan.walls {
        wall.x1 *= factor;
        wall.y1 *= factor;
        wall.x2 *= factor;
        wall.y2 *= factor;
        wall.thickness *= factor;
    }
    for line in &mut plan.lines {
        line.x1 *= factor;
        line.y1 *= factor;
        line.x2 *= factor;
        line.y2 *= factor;
    }
    for door in &mut plan.doors {
        door.width *= factor;
    }
    for window in &mut plan.windows {
        window.width *= factor;
    }
    for object in &mut plan.objects {
        object.x *= factor;
        object.y *= factor;
        object.width *= factor;
        object.height *= factor;
    }
    for image in &mut plan.images {
        image.x *= factor;
        image.y *= factor;
        image.width *= factor;
        image.height *= factor;
    }
    for text in &mut plan.texts {
        text.x *= factor;
        text.y *= factor;
        text.size *= factor;
    }
    for area in &mut plan.areas {
        for point in &mut area.points {
            point.x *= factor;
            point.y *= factor;
        }
    }
    for room in &mut plan.rooms {
        for point in &mut room.points {
            point.x *= factor;
            point.y *= factor;
        }
    }
    for compass in &mut plan.compasses {
        compass.x *= factor;
        compass.y *= factor;
        compass.size *= factor;
    }
    for arrow in &mut plan.arrows {
        arrow.x *= factor;
        arrow.y *= factor;
        arrow.length *= factor;
        arrow.width *= factor;
    }
}

fn parse_hinge_side(value: &str, line: usize) -> Result<HingeSide, FormatError> {
    match value {
        "left" => Ok(HingeSide::Left),
        "right" => Ok(HingeSide::Right),
        _ => syntax(line, "a hinge must be left or right"),
    }
}

fn parse_swing_side(value: &str, line: usize) -> Result<SwingSide, FormatError> {
    match value {
        "inward" => Ok(SwingSide::Inward),
        "outward" => Ok(SwingSide::Outward),
        _ => syntax(line, "a swing direction must be inward or outward"),
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_extension(value: &str) -> bool {
    matches!(value, "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp")
}

fn valid_image_bytes(bytes: &[u8]) -> bool {
    crate::images::valid_dimensions(bytes)
}

fn syntax<T>(line: usize, message: impl Into<String>) -> Result<T, FormatError> {
    Err(FormatError::Syntax {
        line,
        message: message.into(),
    })
}

#[derive(Deserialize, Serialize)]
struct LegacyPlanFile {
    version: u32,
    walls: Vec<LegacyWall>,
    doors: Vec<LegacyDoor>,
}

#[derive(Deserialize, Serialize)]
struct LegacyWall {
    id: String,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

#[derive(Deserialize, Serialize)]
struct LegacyDoor {
    id: String,
    #[serde(rename = "wallId")]
    wall_id: String,
    t: f32,
    width: f32,
}

impl LegacyPlanFile {
    fn into_current(self) -> PlanFile {
        PlanFile {
            version: self.version,
            walls: self
                .walls
                .into_iter()
                .map(|wall| Wall {
                    id: wall.id,
                    x1: wall.x1,
                    y1: wall.y1,
                    x2: wall.x2,
                    y2: wall.y2,
                    thickness: default_wall_thickness(),
                    kind: WallKind::default(),
                    color: default_wall_color(),
                    locked: false,
                })
                .collect(),
            lines: Vec::new(),
            doors: self
                .doors
                .into_iter()
                .map(|door| Door {
                    id: door.id,
                    wall_id: door.wall_id,
                    t: door.t,
                    width: door.width,
                    kind: DoorKind::default(),
                    hinge: HingeSide::default(),
                    swing: SwingSide::default(),
                    color: default_opening_color(),
                    locked: false,
                })
                .collect(),
            windows: Vec::new(),
            objects: Vec::new(),
            images: Vec::new(),
            texts: Vec::new(),
            areas: Vec::new(),
            compasses: Vec::new(),
            arrows: Vec::new(),
            rooms: Vec::new(),
            layers: default_layers(),
            layer_assignments: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use image::ImageEncoder;
    use miniz_oxide::deflate::compress_to_vec_zlib;

    use super::*;

    fn sample_plan() -> PlanFile {
        PlanFile {
            version: 1,
            walls: vec![Wall {
                id: "test-wall".to_owned(),
                x1: 20.0,
                y1: 40.0,
                x2: 420.0,
                y2: 40.0,
                thickness: 14.0,
                kind: WallKind::Interior,
                color: default_wall_color(),
                locked: false,
            }],
            lines: vec![PlanLine {
                id: "test-line".to_owned(),
                x1: 20.0,
                y1: 80.0,
                x2: 220.0,
                y2: 80.0,
                color: [20, 90, 180],
                locked: false,
            }],
            doors: vec![Door {
                id: "test-door".to_owned(),
                wall_id: "test-wall".to_owned(),
                t: 0.5,
                width: 90.0,
                kind: DoorKind::Single,
                hinge: HingeSide::Left,
                swing: SwingSide::Inward,
                color: default_opening_color(),
                locked: false,
            }],
            windows: Vec::new(),
            objects: vec![PlanObject {
                id: "test-object".to_owned(),
                name: "Custom table".to_owned(),
                kind: ObjectKind::Table,
                x: 180.0,
                y: 220.0,
                width: 120.0,
                height: 70.0,
                rotation: 15.0,
                fill_color: default_object_fill_color(),
                stroke_color: default_object_stroke_color(),
                locked: false,
            }],
            images: Vec::new(),
            texts: vec![TextObject {
                id: "test-text".to_owned(),
                text: "Color #f4efe6\nLiving room".to_owned(),
                x: 200.0,
                y: 140.0,
                size: 18.0,
                rotation: -10.0,
                color: [180, 40, 70],
                bold: true,
                locked: false,
            }],
            areas: vec![AreaShape {
                id: "test-area".to_owned(),
                points: vec![
                    Point::new(20.0, 100.0),
                    Point::new(220.0, 100.0),
                    Point::new(220.0, 200.0),
                    Point::new(20.0, 200.0),
                ],
                fill_color: [214, 230, 210],
                stroke_color: [70, 110, 70],
                locked: false,
            }],
            compasses: vec![CompassRose {
                id: "test-compass".to_owned(),
                x: 300.0,
                y: 160.0,
                size: 80.0,
                rotation: 15.0,
                color: [52, 56, 59],
                locked: false,
            }],
            arrows: vec![PlanArrow {
                id: "test-arrow".to_owned(),
                x: 40.0,
                y: 240.0,
                length: 100.0,
                width: 20.0,
                style: ArrowStyle::Solid,
                orientation: 9,
                color: [52, 56, 59],
                locked: false,
            }],
            rooms: vec![Room {
                id: "test-room".to_owned(),
                name: "Office #1".to_owned(),
                kind: RoomKind::Office,
                source: RoomSource::Manual,
                points: vec![
                    Point::new(20.0, 300.0),
                    Point::new(220.0, 300.0),
                    Point::new(220.0, 420.0),
                    Point::new(20.0, 420.0),
                ],
                fill_color: [220, 230, 239],
                fill_visible: true,
                locked: false,
            }],
            layers: vec![
                Layer::default(),
                Layer {
                    id: "furniture".to_owned(),
                    name: "Furniture".to_owned(),
                    visible: true,
                    locked: false,
                    opacity: 0.8,
                },
            ],
            layer_assignments: vec![LayerAssignment {
                item_id: "test-object".to_owned(),
                layer_id: "furniture".to_owned(),
            }],
        }
    }

    fn sample_asset() -> ProjectAsset {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[20, 80, 160, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .expect("PNG encoding");
        ProjectAsset {
            id: "asset-logo".to_owned(),
            name: "logo".to_owned(),
            extension: "png".to_owned(),
            bytes,
        }
    }

    #[test]
    fn readable_text_round_trip() {
        let expected = sample_plan();
        let encoded = encode_plan(&expected).expect("encoding");
        let text = str::from_utf8(&encoded).expect("utf-8");
        let decoded = decode_plan(&encoded).expect("decoding");

        assert_eq!(decoded.plan, expected);
        assert!(decoded.needs_conversion);
        assert!(text.starts_with(TEXT_HEADER));
        assert!(text.contains("WALL test-wall 20 40 420 40 14 interior false 34383b"));
        assert!(text.contains("LINE test-line 20 80 220 80 false 145ab4"));
        assert!(text.contains("DOOR test-door test-wall 0.5 90 single left inward false 464646"));
        assert!(text.contains(
            r##"TEXT test-text 200 140 18 -10 false b42846 true "Color #f4efe6\nLiving room""##
        ));
        assert!(text.contains("AREA test-area false d6e6d2 466e46"));
        assert!(text.contains("ROOM test-room manual office false true dce6ef"));
        assert!(text.contains("ROOM_NAME test-room \"Office #1\""));
        assert!(text.contains("LAYER furniture true false 0.8 \"Furniture\""));
        assert!(text.contains("ASSIGN test-object furniture"));
        assert!(text.contains("COMPASS test-compass 300 160 80 15 false 34383b"));
        assert!(text.contains("OBJECT_NAME test-object \"Custom table\""));
        assert!(text.contains("ARROW test-arrow 40 240 100 20 solid 9 false 34383b"));
    }

    #[test]
    fn object_names_preserve_hash_characters() {
        let input = "\
RPP-PLAN 2
UNITS cm
OBJECT object-1 rectangle 10 20 100 60 0 false e4e6e5 666666
OBJECT_NAME object-1 \"Desk #1\"
";
        let (plan, _) = decode_text(input).expect("object name with hash");

        assert_eq!(plan.objects[0].name, "Desk #1");
    }

    #[test]
    fn arbitrary_small_inputs_never_panic_the_decoder() {
        let mut state = 0x6d2b_79f5_u32;
        for length in 0..512 {
            let mut input = vec![0_u8; length];
            for byte in &mut input {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *byte = state as u8;
            }
            let result = std::panic::catch_unwind(|| decode_plan(&input));
            assert!(result.is_ok(), "decoder panicked for {length} random bytes");
        }
    }

    #[test]
    fn documented_complete_example_stays_parseable() {
        let documentation = include_str!("../../RPP_FORMAT.md");
        let section = documentation
            .split("## 3. Complete example")
            .nth(1)
            .expect("complete example section");
        let example = section
            .split("```text")
            .nth(1)
            .and_then(|content| content.split("```").next())
            .expect("complete RPP-PLAN example");
        let (plan, assets) = decode_text(example.trim()).expect("documented example");

        assert_eq!(plan.walls.len(), 4);
        assert_eq!(plan.doors.len(), 1);
        assert_eq!(plan.windows.len(), 1);
        assert_eq!(plan.arrows.len(), 1);
        assert_eq!(plan.rooms.len(), 1);
        assert_eq!(plan.layers.len(), 2);
        assert_eq!(plan.layer_assignments.len(), 1);
        assert_eq!(assets.len(), 1);
        assert_eq!(plan.images.len(), 2);
    }

    #[test]
    fn non_metric_units_are_converted_to_canonical_centimeters() {
        let input = b"RPP-PLAN 2\nUNITS in\nWALL wall-1 0 0 10 0 1 interior false 34383b\n";
        let decoded = decode_plan(input).expect("inch plan");
        let wall = &decoded.plan.walls[0];
        assert!((wall.length() - 25.4).abs() < 0.001);
        assert!((wall.thickness - 2.54).abs() < 0.001);
    }

    #[test]
    fn built_in_assets_do_not_need_archive_bytes() {
        let mut plan = PlanFile::default();
        plan.images.push(ImageObject {
            id: "exit-instance".to_owned(),
            asset_id: "builtin-emergency-exit".to_owned(),
            x: 20.0,
            y: 20.0,
            width: 40.0,
            height: 40.0,
            rotation: 0.0,
            locked: false,
        });
        let encoded = encode_project(&plan, &[]).expect("built-in project");
        let decoded = decode_plan(&encoded).expect("built-in project decoding");
        assert!(decoded.assets.is_empty());
        assert_eq!(decoded.plan.images[0].asset_id, "builtin-emergency-exit");
    }

    #[test]
    fn accepts_comments_and_blank_lines() {
        let text = "\
            # Handwritten plan\n\
            RPP-PLAN 1\n\
            UNITS cm\n\n\
            WALL living-room-north 0 0 400 0 # top wall\n\
            DOOR entrance living-room-north 0.5 90\n";
        let decoded = decode_plan(text.as_bytes()).expect("commented plan");

        assert_eq!(decoded.plan.walls.len(), 1);
        assert_eq!(decoded.plan.doors.len(), 1);
    }

    #[test]
    fn opens_legacy_json() {
        let expected = sample_plan();
        let json = serde_json::to_vec_pretty(&expected).expect("json");
        let decoded = decode_plan(&json).expect("legacy JSON");

        assert_eq!(decoded.plan, expected);
        assert!(decoded.needs_conversion);
    }

    #[test]
    fn opens_legacy_binary() {
        let mut expected = sample_plan();
        expected.lines.clear();
        expected.texts.clear();
        expected.areas.clear();
        expected.compasses.clear();
        expected.objects.clear();
        expected.arrows.clear();
        expected.rooms.clear();
        expected.layers = default_layers();
        expected.layer_assignments.clear();
        let legacy = LegacyPlanFile {
            version: 1,
            walls: vec![LegacyWall {
                id: "test-wall".to_owned(),
                x1: 20.0,
                y1: 40.0,
                x2: 420.0,
                y2: 40.0,
            }],
            doors: vec![LegacyDoor {
                id: "test-door".to_owned(),
                wall_id: "test-wall".to_owned(),
                t: 0.5,
                width: 90.0,
            }],
        };
        let serialized = postcard::to_stdvec(&legacy).expect("postcard");
        let compressed = compress_to_vec_zlib(&serialized, 9);
        let mut binary = Vec::new();
        binary.extend_from_slice(BINARY_MAGIC);
        binary.extend_from_slice(&BINARY_VERSION.to_le_bytes());
        binary.extend_from_slice(&(serialized.len() as u32).to_le_bytes());
        binary.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        binary.extend_from_slice(&compressed);

        let decoded = decode_plan(&binary).expect("legacy RPPBIN");
        assert_eq!(decoded.plan, expected);
        assert!(decoded.needs_conversion);
    }

    #[test]
    fn rejects_invalid_reference() {
        let text = "\
            RPP-PLAN 1\n\
            UNITS cm\n\
            DOOR entrance missing-wall 0.5 90\n";
        assert!(decode_plan(text.as_bytes()).is_err());
    }

    #[test]
    fn rejects_bad_syntax_with_line_number() {
        let text = "RPP-PLAN 1\nUNITS cm\nWALL incomplete 0 0\n";
        assert!(matches!(
            decode_plan(text.as_bytes()),
            Err(FormatError::Syntax { line: 3, .. })
        ));
    }

    #[test]
    fn rejects_opening_outside_its_wall() {
        let text = "\
            RPP-PLAN 1\n\
            UNITS cm\n\
            WALL short 0 0 100 0\n\
            DOOR impossible short 0.1 90\n";
        assert!(matches!(
            decode_plan(text.as_bytes()),
            Err(FormatError::InvalidPlan)
        ));
    }

    #[test]
    fn rejects_zero_length_wall() {
        let text = "RPP-PLAN 1\nUNITS cm\nWALL point 10 10 10 10\n";
        assert!(matches!(
            decode_plan(text.as_bytes()),
            Err(FormatError::InvalidPlan)
        ));
    }

    #[test]
    fn opens_shipped_example() {
        let decoded =
            decode_plan(include_bytes!("../../example-room.data")).expect("valid bundled example");
        assert_eq!(decoded.plan.walls.len(), 4);
        assert_eq!(decoded.plan.doors.len(), 1);
    }

    #[test]
    fn opens_shipped_project_container() {
        let decoded =
            decode_plan(include_bytes!("../../example-room.rpp")).expect("valid project package");
        assert_eq!(decoded.plan.walls.len(), 4);
        assert_eq!(decoded.plan.windows.len(), 1);
        assert_eq!(decoded.plan.objects.len(), 1);
        assert!(!decoded.needs_conversion);
    }

    #[test]
    fn accepts_diagonal_and_negative_coordinates() {
        let text = "\
            RPP-PLAN 1\n\
            # UTF-8 characters: éàç 👋\n\
            UNITS cm\n\
            WALL diagonal -250.5 -40 120.25 300 12 interior false\n";
        let decoded = decode_plan(text.as_bytes()).expect("valid geometry");
        assert!(decoded.plan.walls[0].length() > 0.0);
    }

    #[test]
    fn project_container_round_trip_embeds_reusable_asset() {
        let mut plan = sample_plan();
        plan.images = vec![
            ImageObject {
                id: "image-one".to_owned(),
                asset_id: "asset-logo".to_owned(),
                x: 100.0,
                y: 120.0,
                width: 80.0,
                height: 80.0,
                rotation: 0.0,
                locked: false,
            },
            ImageObject {
                id: "image-two".to_owned(),
                asset_id: "asset-logo".to_owned(),
                x: 220.0,
                y: 120.0,
                width: 40.0,
                height: 40.0,
                rotation: 45.0,
                locked: true,
            },
        ];
        let asset = sample_asset();

        let encoded =
            encode_project(&plan, std::slice::from_ref(&asset)).expect("project encoding");
        assert!(encoded.starts_with(CONTAINER_MAGIC));
        let decoded = decode_plan(&encoded).expect("project decoding");

        assert_eq!(decoded.plan, plan);
        assert_eq!(decoded.assets, vec![asset]);
        assert!(!decoded.needs_conversion);
    }

    #[test]
    fn project_round_trip_keeps_windows_and_objects() {
        let mut plan = sample_plan();
        plan.windows.push(Window {
            id: "living-window".to_owned(),
            wall_id: "test-wall".to_owned(),
            t: 0.82,
            width: 50.0,
            kind: WindowKind::Bay,
            color: default_window_color(),
            locked: true,
        });
        plan.objects.push(PlanObject {
            id: "desk-one".to_owned(),
            name: String::new(),
            kind: ObjectKind::Desk,
            x: -120.0,
            y: 240.0,
            width: 140.0,
            height: 70.0,
            rotation: 30.0,
            fill_color: [230, 210, 120],
            stroke_color: [80, 60, 20],
            locked: false,
        });

        let encoded = encode_project(&plan, &[]).expect("project encoding");
        let decoded = decode_plan(&encoded).expect("project decoding");

        assert_eq!(decoded.plan, plan);
    }

    #[test]
    fn zip_expansion_budget_is_checked_before_reading_payloads() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "plan.data",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .expect("entry");
        writer
            .write_all(b"RPP-PLAN 2\nUNITS cm\n")
            .expect("content");
        let mut bytes = writer.finish().expect("zip").into_inner();
        let central = bytes
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .expect("central header");
        bytes[central + 24..central + 28]
            .copy_from_slice(&((MAX_PROJECT_BYTES as u32) + 1).to_le_bytes());
        assert!(matches!(decode_plan(&bytes), Err(FormatError::TooLarge)));
    }

    #[test]
    fn legacy_payload_cannot_expand_past_its_declared_size() {
        let payload = miniz_oxide::deflate::compress_to_vec_zlib(&vec![0_u8; 100_000], 9);
        let mut bytes = BINARY_MAGIC.to_vec();
        bytes.extend(BINARY_VERSION.to_le_bytes());
        bytes.extend(1_u32.to_le_bytes());
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
        assert!(decode_plan(&bytes).is_err());
    }

    #[test]
    fn legacy_json_with_missing_assets_or_default_layer_is_rejected() {
        let mut plan = PlanFile::default();
        plan.images.push(ImageObject {
            id: "image".into(),
            asset_id: "missing".into(),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            rotation: 0.0,
            locked: false,
        });
        assert!(decode_plan(&serde_json::to_vec(&plan).expect("JSON")).is_err());
        plan.images.clear();
        plan.layers[0].id = "custom-only".into();
        assert!(decode_plan(&serde_json::to_vec(&plan).expect("JSON")).is_err());
    }

    #[test]
    fn non_finite_or_out_of_bounds_polygon_geometry_is_rejected() {
        let mut plan = PlanFile::default();
        plan.areas.push(AreaShape {
            id: "area".into(),
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(f32::MAX, 0.0),
                Point::new(0.0, f32::MAX),
            ],
            fill_color: [0, 0, 0],
            stroke_color: [0, 0, 0],
            locked: false,
        });
        assert!(validate_plan(&plan).is_err());
    }
}
