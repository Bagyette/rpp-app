use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Cursor;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use image::ImageFormat;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

use crate::localization::Localizer;
use crate::model::{
    ArrowStyle, Door, DoorKind, HingeSide, ObjectKind, PlanFile, Point, ProjectAsset, SwingSide,
    Wall, WindowKind,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperFormat {
    #[default]
    A4,
    Letter,
    Legal,
    Tabloid,
}

impl PaperFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::A4 => "A4",
            Self::Letter => "Letter",
            Self::Legal => "Legal",
            Self::Tabloid => "Tabloid",
        }
    }

    fn portrait_points(self) -> (f32, f32) {
        match self {
            Self::A4 => (595.276, 841.89),
            Self::Letter => (612.0, 792.0),
            Self::Legal => (612.0, 1008.0),
            Self::Tabloid => (792.0, 1224.0),
        }
    }
}

const EXPORT_MARGIN: f32 = 32.0;
const MIN_LONG_SIDE: f32 = 1200.0;
const MAX_LONG_SIDE: f32 = 4096.0;
const DEFAULT_SCALE: f32 = 1.5;

struct ExportScene {
    svg: String,
    width: u32,
    height: u32,
    view_width: f32,
    view_height: f32,
}

pub fn export_svg(
    path: &Path,
    plan: &PlanFile,
    assets: &[ProjectAsset],
    localizer: &Localizer,
    include_distances: bool,
) -> Result<(), String> {
    let scene = build_scene(plan, assets, localizer, include_distances)?;
    crate::storage::write_atomic(path, scene.svg.as_bytes()).map_err(|error| error.to_string())
}

pub fn export_png(
    path: &Path,
    plan: &PlanFile,
    assets: &[ProjectAsset],
    localizer: &Localizer,
    include_distances: bool,
) -> Result<(), String> {
    let scene = build_scene(plan, assets, localizer, include_distances)?;
    let options = usvg::Options {
        fontdb: crate::platform::font_database(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&scene.svg, &options).map_err(|error| error.to_string())?;
    let mut pixmap =
        Pixmap::new(scene.width, scene.height).ok_or_else(|| "Invalid export size".to_owned())?;
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    let png = pixmap.encode_png().map_err(|error| error.to_string())?;
    crate::storage::write_atomic(path, &png).map_err(|error| error.to_string())
}

#[cfg(test)]
pub fn export_pdf_a4(
    path: &Path,
    plan: &PlanFile,
    assets: &[ProjectAsset],
    localizer: &Localizer,
    include_distances: bool,
    drawing_scale: u32,
) -> Result<(), String> {
    export_pdf(
        path,
        plan,
        assets,
        localizer,
        include_distances,
        drawing_scale,
        PaperFormat::A4,
    )
}

pub fn export_pdf(
    path: &Path,
    plan: &PlanFile,
    assets: &[ProjectAsset],
    localizer: &Localizer,
    include_distances: bool,
    drawing_scale: u32,
    paper: PaperFormat,
) -> Result<(), String> {
    let scene = build_scene(plan, assets, localizer, include_distances)?;
    let drawing_scale = drawing_scale.max(1) as f32;
    let points_per_centimeter = 72.0 / 2.54;
    let image_width_points = scene.view_width * points_per_centimeter / drawing_scale;
    let image_height_points = scene.view_height * points_per_centimeter / drawing_scale;
    let portrait = paper.portrait_points();
    let landscape = (portrait.1, portrait.0);
    let (page_width, page_height) = if scene.view_width > scene.view_height {
        landscape
    } else {
        portrait
    };
    let page_margin = points_per_centimeter;
    if image_width_points > page_width - page_margin * 2.0
        || image_height_points > page_height - page_margin * 2.0
    {
        return Err(localizer.format(
            "export.pdf_does_not_fit",
            &[("scale", drawing_scale.to_string())],
        ));
    }

    // Keep the original viewBox inside a physical-size page. The PDF converter
    // receives paths and text, rather than a screenshot limited to 4096 pixels.
    let x = (page_width - image_width_points) * 0.5;
    let y = (page_height - image_height_points) * 0.5;
    let nested = scene.svg.replacen(
        &format!("width=\"{}\" height=\"{}\"", scene.width, scene.height),
        &format!(
            "x=\"{x}\" y=\"{y}\" width=\"{image_width_points}\" height=\"{image_height_points}\""
        ),
        1,
    );
    let page_svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{page_width}" height="{page_height}">{nested}</svg>"#
    );
    let mut options = svg2pdf::usvg::Options::default();
    let fonts = crate::platform::font_database();
    for face in fonts.faces() {
        fonts.with_face_data(face.id, |bytes, _| {
            options.fontdb_mut().load_font_data(bytes.to_vec())
        });
    }
    // Match the family fallback chosen by the canvas/raster exporter.
    options
        .fontdb_mut()
        .set_sans_serif_family(fonts.family_name(&resvg::usvg::fontdb::Family::SansSerif));
    let tree =
        svg2pdf::usvg::Tree::from_str(&page_svg, &options).map_err(|error| error.to_string())?;
    let pdf = svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|error| error.to_string())?;
    crate::storage::write_atomic(path, &pdf).map_err(|error| error.to_string())
}

fn build_scene(
    plan: &PlanFile,
    assets: &[ProjectAsset],
    localizer: &Localizer,
    include_distances: bool,
) -> Result<ExportScene, String> {
    let (minimum, maximum) =
        export_bounds(plan).ok_or_else(|| "The plan contains nothing to export".to_owned())?;
    let view_x = minimum.x - EXPORT_MARGIN;
    let view_y = minimum.y - EXPORT_MARGIN;
    let view_width = (maximum.x - minimum.x + EXPORT_MARGIN * 2.0).max(1.0);
    let view_height = (maximum.y - minimum.y + EXPORT_MARGIN * 2.0).max(1.0);
    let long_side = view_width.max(view_height);
    let pixel_long_side = (long_side * DEFAULT_SCALE).clamp(MIN_LONG_SIDE, MAX_LONG_SIDE);
    let scale = pixel_long_side / long_side;
    let width = (view_width * scale).round().max(1.0) as u32;
    let height = (view_height * scale).round().max(1.0) as u32;

    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{view_x:.3} {view_y:.3} {view_width:.3} {view_height:.3}">"#
    )
    .expect("formatting into a String cannot fail");
    writeln!(
        svg,
        r##"<rect x="{view_x:.3}" y="{view_y:.3}" width="{view_width:.3}" height="{view_height:.3}" fill="#ffffff"/>"##
    )
    .expect("formatting into a String cannot fail");

    let lookup = crate::model::LayerLookup::new(plan);
    let walls_by_id: HashMap<&str, &Wall> = plan
        .walls
        .iter()
        .map(|wall| (wall.id.as_str(), wall))
        .collect();
    for layer in &plan.layers {
        if !layer.visible {
            continue;
        }
        let layer_id = layer.id.as_str();
        for room in &plan.rooms {
            if !(lookup.item_visible(&room.id) && lookup.layer_id_for(&room.id) == layer_id) {
                continue;
            }
            write!(svg, r#"<polygon points=""#).expect("formatting into a String cannot fail");
            for point in &room.points {
                write!(svg, "{:.3},{:.3} ", point.x, point.y)
                    .expect("formatting into a String cannot fail");
            }
            let opacity = lookup.item_opacity(&room.id);
            writeln!(
            svg,
            r##"" fill="{}" fill-opacity="{:.3}" stroke="#3c698c" stroke-opacity="{:.3}" stroke-width="1"/>"##,
            svg_color(room.fill_color),
            if room.fill_visible { 0.31 * opacity } else { 0.0 },
            0.7 * opacity,
        )
        .expect("formatting into a String cannot fail");
            let center = crate::room_detection::polygon_center(&room.points);
            let title = if room.name.trim().is_empty() {
                localizer.text("room.default_name")
            } else {
                room.name.trim().to_owned()
            };
            writeln!(
            svg,
            r##"<text x="{:.3}" y="{:.3}" text-anchor="middle" font-family="Arial, sans-serif" font-size="12" fill="#3c698c"><tspan x="{:.3}" dy="0">{}</tspan><tspan x="{:.3}" dy="14">{:.2} m²</tspan></text>"##,
            center.x,
            center.y - 7.0,
            center.x,
            escape_xml(&title),
            center.x,
            room.area_square_centimeters() / 10_000.0,
        )
        .expect("formatting into a String cannot fail");
        }

        for area in &plan.areas {
            if !(lookup.item_visible(&area.id) && lookup.layer_id_for(&area.id) == layer_id) {
                continue;
            }
            writeln!(svg, r#"<g opacity="{:.3}">"#, lookup.item_opacity(&area.id))
                .expect("formatting into a String cannot fail");
            write!(svg, r#"<polygon points=""#).expect("formatting into a String cannot fail");
            for point in &area.points {
                write!(svg, "{:.3},{:.3} ", point.x, point.y)
                    .expect("formatting into a String cannot fail");
            }
            writeln!(
                svg,
                r#"" fill="{}" fill-opacity="0.47" stroke="{}" stroke-width="1.3"/>"#,
                svg_color(area.fill_color),
                svg_color(area.stroke_color)
            )
            .expect("formatting into a String cannot fail");
            let center = area_center(area);
            writeln!(
            svg,
            r##"<text x="{:.3}" y="{:.3}" text-anchor="middle" dominant-baseline="central" font-family="Arial, sans-serif" font-size="12" fill="#465f46">{:.2} m²</text>"##,
            center.x,
            center.y,
            area.area_square_centimeters() / 10_000.0
        )
        .expect("formatting into a String cannot fail");
            svg.push_str("</g>\n");
        }

        for image in &plan.images {
            if !(lookup.item_visible(&image.id) && lookup.layer_id_for(&image.id) == layer_id) {
                continue;
            }
            let bytes: &[u8] = if image.asset_id == "builtin-emergency-exit" {
                include_bytes!("../assets/library/emergency-exit.png")
            } else if let Some(asset) = assets.iter().find(|asset| asset.id == image.asset_id) {
                &asset.bytes
            } else {
                continue;
            };
            let Ok(decoded) = image::load_from_memory(bytes) else {
                continue;
            };
            let mut png = Vec::new();
            if decoded
                .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
                .is_err()
            {
                continue;
            }
            let encoded = STANDARD.encode(png);
            writeln!(
                svg,
                r#"<g opacity="{:.3}">"#,
                lookup.item_opacity(&image.id)
            )
            .expect("formatting into a String cannot fail");
            writeln!(
            svg,
            r#"<image x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" preserveAspectRatio="none" transform="rotate({:.3} {:.3} {:.3})" href="data:image/png;base64,{}"/>"#,
            image.x - image.width / 2.0,
            image.y - image.height / 2.0,
            image.width,
            image.height,
            image.rotation,
            image.x,
            image.y,
            encoded
        )
        .expect("formatting into a String cannot fail");
            svg.push_str("</g>\n");
        }

        for object in &plan.objects {
            if !(lookup.item_visible(&object.id) && lookup.layer_id_for(&object.id) == layer_id) {
                continue;
            }
            writeln!(
                svg,
                r#"<g opacity="{:.3}">"#,
                lookup.item_opacity(&object.id)
            )
            .expect("formatting into a String cannot fail");
            let points = rotated_rectangle(
                object.x,
                object.y,
                object.width,
                object.height,
                object.rotation,
            );
            write!(svg, r#"<polygon points=""#).expect("formatting into a String cannot fail");
            for point in points {
                write!(svg, "{:.3},{:.3} ", point.x, point.y)
                    .expect("formatting into a String cannot fail");
            }
            writeln!(
                svg,
                r#"" fill="{}" fill-opacity="0.82" stroke="{}" stroke-width="1.3"/>"#,
                svg_color(object.fill_color),
                svg_color(object.stroke_color)
            )
            .expect("formatting into a String cannot fail");
            let label_key = match object.kind {
                ObjectKind::Rectangle => "object.rectangle",
                ObjectKind::Bed => "object.bed",
                ObjectKind::Table => "object.table",
                ObjectKind::Sofa => "object.sofa",
                ObjectKind::Wardrobe => "object.wardrobe",
                ObjectKind::Desk => "object.desk",
            };
            writeln!(
            svg,
            r#"<text x="{:.3}" y="{:.3}" text-anchor="middle" dominant-baseline="central" font-family="Arial, sans-serif" font-size="12" fill="{}">{}</text>"#,
            object.x,
            object.y,
            svg_color(object.stroke_color),
            escape_xml(object.display_name(&localizer.text(label_key)))
        )
        .expect("formatting into a String cannot fail");
            svg.push_str("</g>\n");
        }

        for text in &plan.texts {
            if !(lookup.item_visible(&text.id) && lookup.layer_id_for(&text.id) == layer_id) {
                continue;
            }
            writeln!(svg, r#"<g opacity="{:.3}">"#, lookup.item_opacity(&text.id))
                .expect("formatting into a String cannot fail");
            for (index, line) in text.text.lines().enumerate() {
                let line_count = text.text.lines().count().max(1) as f32;
                let offset = (index as f32 - (line_count - 1.0) / 2.0) * text.size * 1.2;
                writeln!(
                svg,
                r#"<text x="{:.3}" y="{:.3}" text-anchor="middle" dominant-baseline="central" font-family="Arial, sans-serif" font-size="{:.3}" font-weight="{}" fill="{}" transform="rotate({:.3} {:.3} {:.3})">{}</text>"#,
                text.x,
                text.y + offset,
                text.size,
                if text.bold { "bold" } else { "normal" },
                svg_color(text.color),
                text.rotation,
                text.x,
                text.y,
                escape_xml(line)
            )
            .expect("formatting into a String cannot fail");
            }
            svg.push_str("</g>\n");
        }
        for line in &plan.lines {
            if !(lookup.item_visible(&line.id) && lookup.layer_id_for(&line.id) == layer_id) {
                continue;
            }
            writeln!(svg, r#"<g opacity="{:.3}">"#, lookup.item_opacity(&line.id))
                .expect("formatting into a String cannot fail");
            write_line(
                &mut svg,
                line.start(),
                line.end(),
                1.5,
                &svg_color(line.color),
            );
            svg.push_str("</g>\n");
        }

        for wall in &plan.walls {
            if !(lookup.item_visible(&wall.id) && lookup.layer_id_for(&wall.id) == layer_id) {
                continue;
            }
            writeln!(svg, r#"<g opacity="{:.3}">"#, lookup.item_opacity(&wall.id))
                .expect("formatting into a String cannot fail");
            writeln!(
            svg,
            r#"<line x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" stroke="{}" stroke-width="{:.3}" stroke-linecap="square"/>"#,
            wall.x1,
            wall.y1,
            wall.x2,
            wall.y2,
            svg_color(wall.color),
            wall.thickness
        )
        .expect("formatting into a String cannot fail");
            svg.push_str("</g>\n");
        }

        for door in &plan.doors {
            if !(lookup.item_visible(&door.id) && lookup.layer_id_for(&door.id) == layer_id) {
                continue;
            }
            let Some(wall) = walls_by_id.get(door.wall_id.as_str()).copied() else {
                continue;
            };
            if !lookup.item_visible(&wall.id) {
                continue;
            }
            writeln!(svg, r#"<g opacity="{:.3}">"#, lookup.item_opacity(&door.id))
                .expect("formatting into a String cannot fail");
            write_door(&mut svg, door, wall);
            svg.push_str("</g>\n");
        }
        for window in &plan.windows {
            if !(lookup.item_visible(&window.id) && lookup.layer_id_for(&window.id) == layer_id) {
                continue;
            }
            let Some(wall) = walls_by_id.get(window.wall_id.as_str()).copied() else {
                continue;
            };
            if !lookup.item_visible(&wall.id) {
                continue;
            }
            let length = wall.length();
            if length <= f32::EPSILON {
                continue;
            }
            writeln!(
                svg,
                r#"<g opacity="{:.3}">"#,
                lookup.item_opacity(&window.id)
            )
            .expect("formatting into a String cannot fail");
            let ux = (wall.x2 - wall.x1) / length;
            let uy = (wall.y2 - wall.y1) / length;
            let center = wall.point_at(window.t);
            let start = Point::new(
                center.x - ux * window.width / 2.0,
                center.y - uy * window.width / 2.0,
            );
            let end = Point::new(
                center.x + ux * window.width / 2.0,
                center.y + uy * window.width / 2.0,
            );
            write_line(&mut svg, start, end, wall.thickness + 4.0, "#ffffff");
            let offsets: &[f32] = if window.kind == WindowKind::Bay {
                &[-0.35, 0.0, 0.35]
            } else {
                &[-0.25, 0.25]
            };
            for offset in offsets {
                let distance = wall.thickness * offset;
                write_line(
                    &mut svg,
                    Point::new(start.x - uy * distance, start.y + ux * distance),
                    Point::new(end.x - uy * distance, end.y + ux * distance),
                    1.6,
                    &svg_color(window.color),
                );
            }
            svg.push_str("</g>\n");
        }

        for compass in &plan.compasses {
            if !(lookup.item_visible(&compass.id) && lookup.layer_id_for(&compass.id) == layer_id) {
                continue;
            }
            writeln!(
                svg,
                r#"<g opacity="{:.3}">"#,
                lookup.item_opacity(&compass.id)
            )
            .expect("formatting into a String cannot fail");
            let radius = compass.size * 0.5;
            let color = svg_color(compass.color);
            writeln!(
            svg,
            r#"<g transform="rotate({:.3} {:.3} {:.3})"><circle cx="{:.3}" cy="{:.3}" r="{:.3}" fill="none" stroke="{}" stroke-width="1.3"/><line x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" stroke="{}" stroke-width="2"/><polygon points="{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}" fill="{}"/><text x="{:.3}" y="{:.3}" text-anchor="middle" font-family="Arial, sans-serif" font-size="14" fill="{}">N</text></g>"#,
            compass.rotation,
            compass.x,
            compass.y,
            compass.x,
            compass.y,
            radius,
            color,
            compass.x,
            compass.y + radius * 0.7,
            compass.x,
            compass.y - radius,
            color,
            compass.x,
            compass.y - radius,
            compass.x - radius * 0.16,
            compass.y - radius * 0.68,
            compass.x + radius * 0.16,
            compass.y - radius * 0.68,
            color,
            compass.x,
            compass.y - radius - 5.0,
            color
        )
        .expect("formatting into a String cannot fail");
            svg.push_str("</g>\n");
        }

        for arrow in &plan.arrows {
            if !(lookup.item_visible(&arrow.id) && lookup.layer_id_for(&arrow.id) == layer_id) {
                continue;
            }
            writeln!(
                svg,
                r#"<g opacity="{:.3}">"#,
                lookup.item_opacity(&arrow.id)
            )
            .expect("formatting into a String cannot fail");
            let start = Point::new(arrow.x, arrow.y);
            let end = arrow.end();
            let angle = arrow.angle_degrees().to_radians();
            let ux = angle.cos();
            let uy = angle.sin();
            let px = -uy;
            let py = ux;
            let head_length = (arrow.width * 1.8).min(arrow.length * 0.55).max(2.0);
            let head_half = arrow.width.max(2.0);
            let base = Point::new(end.x - ux * head_length, end.y - uy * head_length);
            let left = Point::new(base.x + px * head_half, base.y + py * head_half);
            let right = Point::new(base.x - px * head_half, base.y - py * head_half);
            let color = svg_color(arrow.color);
            match arrow.style {
                ArrowStyle::Open | ArrowStyle::OpenBold => {
                    let stroke = if arrow.style == ArrowStyle::OpenBold {
                        (arrow.width * 0.22).max(2.4)
                    } else {
                        (arrow.width * 0.10).max(1.4)
                    };
                    write_line(&mut svg, start, end, stroke, &color);
                    writeln!(
                    svg,
                    r#"<polyline points="{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}" fill="none" stroke="{}" stroke-width="{:.3}" stroke-linejoin="miter"/>"#,
                    left.x, left.y, end.x, end.y, right.x, right.y, color, stroke
                )
                .expect("formatting into a String cannot fail");
                }
                ArrowStyle::Hollow => {
                    write_line(&mut svg, start, base, (arrow.width * 0.12).max(1.5), &color);
                    writeln!(
                    svg,
                    r##"<polygon points="{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}" fill="#ffffff" stroke="{}" stroke-width="{:.3}"/>"##,
                    left.x, left.y, end.x, end.y, right.x, right.y, color,
                    (arrow.width * 0.12).max(1.5)
                )
                .expect("formatting into a String cannot fail");
                }
                ArrowStyle::Solid => {
                    let shaft_half = (arrow.width * 0.28).max(1.0);
                    let s1 = Point::new(start.x + px * shaft_half, start.y + py * shaft_half);
                    let s2 = Point::new(base.x + px * shaft_half, base.y + py * shaft_half);
                    let s3 = Point::new(base.x - px * shaft_half, base.y - py * shaft_half);
                    let s4 = Point::new(start.x - px * shaft_half, start.y - py * shaft_half);
                    writeln!(
                    svg,
                    r#"<polygon points="{:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3}" fill="{}"/>"#,
                    s1.x, s1.y, s2.x, s2.y, left.x, left.y, end.x, end.y,
                    right.x, right.y, s3.x, s3.y, s4.x, s4.y, color
                )
                .expect("formatting into a String cannot fail");
                }
            }
            svg.push_str("</g>\n");
        }

        if include_distances {
            for wall in &plan.walls {
                if lookup.item_visible(&wall.id) && lookup.layer_id_for(&wall.id) == layer_id {
                    write_distance_label(&mut svg, wall.start(), wall.end(), wall.length());
                }
            }
            for line in &plan.lines {
                if lookup.item_visible(&line.id) && lookup.layer_id_for(&line.id) == layer_id {
                    write_distance_label(&mut svg, line.start(), line.end(), line.length());
                }
            }
        }
    }
    svg.push_str("</svg>\n");
    Ok(ExportScene {
        svg,
        width,
        height,
        view_width,
        view_height,
    })
}

fn write_door(svg: &mut String, door: &Door, wall: &Wall) {
    let length = wall.length();
    if length <= f32::EPSILON {
        return;
    }
    let ux = (wall.x2 - wall.x1) / length;
    let uy = (wall.y2 - wall.y1) / length;
    let center = wall.point_at(door.t);
    let left = Point::new(
        center.x - ux * door.width / 2.0,
        center.y - uy * door.width / 2.0,
    );
    let right = Point::new(
        center.x + ux * door.width / 2.0,
        center.y + uy * door.width / 2.0,
    );
    write_line(svg, left, right, wall.thickness + 4.0, "#ffffff");
    let color = svg_color(door.color);

    if door.kind == DoorKind::Sliding {
        let offset = wall.thickness * 0.28;
        write_line(
            svg,
            Point::new(left.x - uy * offset, left.y + ux * offset),
            Point::new(center.x - uy * offset, center.y + ux * offset),
            2.5,
            &color,
        );
        write_line(
            svg,
            Point::new(center.x + uy * offset, center.y - ux * offset),
            Point::new(right.x + uy * offset, right.y - ux * offset),
            2.5,
            &color,
        );
        return;
    }

    let normal_sign = match door.swing {
        SwingSide::Inward => 1.0,
        SwingSide::Outward => -1.0,
    };
    let leaves: Vec<(Point, (f32, f32), f32)> = if door.kind == DoorKind::Double {
        vec![
            (left, (ux, uy), door.width / 2.0),
            (right, (-ux, -uy), door.width / 2.0),
        ]
    } else {
        match door.hinge {
            HingeSide::Left => vec![(left, (ux, uy), door.width)],
            HingeSide::Right => vec![(right, (-ux, -uy), door.width)],
        }
    };

    for (hinge, closed_direction, leaf_width) in leaves {
        let open_direction = (-uy * normal_sign, ux * normal_sign);
        let leaf_end = Point::new(
            hinge.x + open_direction.0 * leaf_width,
            hinge.y + open_direction.1 * leaf_width,
        );
        write_line(svg, hinge, leaf_end, 3.0, &color);
        let start_angle = closed_direction.1.atan2(closed_direction.0);
        let mut delta = open_direction.1.atan2(open_direction.0) - start_angle;
        while delta > std::f32::consts::PI {
            delta -= std::f32::consts::TAU;
        }
        while delta < -std::f32::consts::PI {
            delta += std::f32::consts::TAU;
        }
        write!(svg, r#"<polyline points=""#).expect("formatting into a String cannot fail");
        for index in 0..=18 {
            let angle = start_angle + delta * index as f32 / 18.0;
            write!(
                svg,
                "{:.3},{:.3} ",
                hinge.x + angle.cos() * leaf_width,
                hinge.y + angle.sin() * leaf_width
            )
            .expect("formatting into a String cannot fail");
        }
        writeln!(
            svg,
            r#"" fill="none" stroke="{}" stroke-width="1.3"/>"#,
            color
        )
        .expect("formatting into a String cannot fail");
        writeln!(
            svg,
            r#"<circle cx="{:.3}" cy="{:.3}" r="4" fill="{}"/>"#,
            hinge.x, hinge.y, color
        )
        .expect("formatting into a String cannot fail");
    }
}

fn write_line(svg: &mut String, start: Point, end: Point, width: f32, color: &str) {
    writeln!(
        svg,
        r#"<line x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" stroke="{}" stroke-width="{:.3}"/>"#,
        start.x, start.y, end.x, end.y, color, width
    )
    .expect("formatting into a String cannot fail");
}

fn write_distance_label(svg: &mut String, start: Point, end: Point, centimeters: f32) {
    let x = (start.x + end.x) * 0.5;
    let y = (start.y + end.y) * 0.5;
    let label = if centimeters < 100.0 {
        format!("{centimeters:.1} cm")
    } else {
        format!("{:.2} m", centimeters / 100.0)
    };
    let box_width = label.len() as f32 * 7.0 + 10.0;
    writeln!(
        svg,
        r##"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="18" fill="#ffffff" fill-opacity="0.92"/>"##,
        x - box_width / 2.0,
        y - 9.0,
        box_width
    )
    .expect("formatting into a String cannot fail");
    writeln!(
        svg,
        r##"<text x="{x:.3}" y="{y:.3}" text-anchor="middle" dominant-baseline="central" font-family="Arial, sans-serif" font-size="12" fill="#505050">{}</text>"##,
        escape_xml(&label)
    )
    .expect("formatting into a String cannot fail");
}

fn export_bounds(plan: &PlanFile) -> Option<(Point, Point)> {
    let lookup = crate::model::LayerLookup::new(plan);
    let walls_by_id: HashMap<&str, &Wall> = plan
        .walls
        .iter()
        .map(|wall| (wall.id.as_str(), wall))
        .collect();
    let mut bounds: Option<(Point, Point)> = None;
    let mut include = |point: Point| {
        bounds = Some(match bounds {
            Some((minimum, maximum)) => (
                Point::new(minimum.x.min(point.x), minimum.y.min(point.y)),
                Point::new(maximum.x.max(point.x), maximum.y.max(point.y)),
            ),
            None => (point, point),
        });
    };
    for wall in &plan.walls {
        if !lookup.item_visible(&wall.id) {
            continue;
        }
        let radius = wall.thickness * 0.5;
        for point in [wall.start(), wall.end()] {
            include(Point::new(point.x - radius, point.y - radius));
            include(Point::new(point.x + radius, point.y + radius));
        }
    }
    for line in &plan.lines {
        if !lookup.item_visible(&line.id) {
            continue;
        }
        include(line.start());
        include(line.end());
    }
    for object in &plan.objects {
        if !lookup.item_visible(&object.id) {
            continue;
        }
        for point in rotated_rectangle(
            object.x,
            object.y,
            object.width,
            object.height,
            object.rotation,
        ) {
            include(point);
        }
    }
    for image in &plan.images {
        if !lookup.item_visible(&image.id) {
            continue;
        }
        for point in rotated_rectangle(image.x, image.y, image.width, image.height, image.rotation)
        {
            include(point);
        }
    }
    for text in &plan.texts {
        if !lookup.item_visible(&text.id) {
            continue;
        }
        let (width, height) = text_dimensions(text);
        for point in rotated_rectangle(text.x, text.y, width, height, text.rotation) {
            include(point);
        }
    }
    for area in &plan.areas {
        if !lookup.item_visible(&area.id) {
            continue;
        }
        for point in &area.points {
            include(*point);
        }
    }
    for room in &plan.rooms {
        if !lookup.item_visible(&room.id) {
            continue;
        }
        for point in &room.points {
            include(*point);
        }
    }
    for compass in &plan.compasses {
        if !lookup.item_visible(&compass.id) {
            continue;
        }
        let radius = compass.size * 0.6;
        include(Point::new(compass.x - radius, compass.y - radius));
        include(Point::new(compass.x + radius, compass.y + radius));
    }
    for arrow in &plan.arrows {
        if !lookup.item_visible(&arrow.id) {
            continue;
        }
        let end = arrow.end();
        let radius = arrow.width.max(2.0);
        include(Point::new(arrow.x - radius, arrow.y - radius));
        include(Point::new(arrow.x + radius, arrow.y + radius));
        include(Point::new(end.x - radius, end.y - radius));
        include(Point::new(end.x + radius, end.y + radius));
    }
    for door in &plan.doors {
        if !lookup.item_visible(&door.id) {
            continue;
        }
        let Some(wall) = walls_by_id.get(door.wall_id.as_str()).copied() else {
            continue;
        };
        if !lookup.item_visible(&wall.id) {
            continue;
        }
        let center = wall.point_at(door.t);
        let radius = door.width;
        include(Point::new(center.x - radius, center.y - radius));
        include(Point::new(center.x + radius, center.y + radius));
    }
    bounds
}

fn rotated_rectangle(x: f32, y: f32, width: f32, height: f32, rotation: f32) -> [Point; 4] {
    let angle = rotation.to_radians();
    let cosine = angle.cos();
    let sine = angle.sin();
    [
        (-width / 2.0, -height / 2.0),
        (width / 2.0, -height / 2.0),
        (width / 2.0, height / 2.0),
        (-width / 2.0, height / 2.0),
    ]
    .map(|(local_x, local_y)| {
        Point::new(
            x + local_x * cosine - local_y * sine,
            y + local_x * sine + local_y * cosine,
        )
    })
}

fn text_dimensions(text: &crate::model::TextObject) -> (f32, f32) {
    let lines: Vec<&str> = text.text.lines().collect();
    let line_count = lines.len().max(1) as f32;
    let longest = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let width_factor = if text.bold { 0.62 } else { 0.58 };
    (
        (longest * text.size * width_factor).max(text.size),
        line_count * text.size * 1.2,
    )
}

fn area_center(area: &crate::model::AreaShape) -> Point {
    let (x, y) = area
        .points
        .iter()
        .fold((0.0, 0.0), |(x, y), point| (x + point.x, y + point.y));
    let count = area.points.len().max(1) as f32;
    Point::new(x / count, y / count)
}

fn escape_xml(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if matches!(character, '\t' | '\n' | '\r')
                || character >= '\u{20}' && character != '\u{fffe}' && character != '\u{ffff}'
            {
                character
            } else {
                '\u{fffd}'
            }
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn svg_color(color: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

#[cfg(test)]
mod tests {
    use super::{build_scene, export_pdf_a4, export_png};
    use crate::{
        localization::Localizer,
        model::{
            ImageObject, ObjectKind, PlanFile, PlanLine, PlanObject, ProjectAsset, TextObject,
            Wall, WallKind,
        },
    };
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn test_plan() -> PlanFile {
        let mut plan = PlanFile::default();
        plan.walls.push(Wall {
            id: "wall-export".to_owned(),
            x1: 0.0,
            y1: 0.0,
            x2: 400.0,
            y2: 0.0,
            thickness: 14.0,
            kind: WallKind::Exterior,
            color: [52, 56, 59],
            locked: false,
        });
        plan
    }

    #[test]
    fn svg_export_is_vector_and_bounded() {
        let scene = build_scene(&test_plan(), &[], &Localizer::load(), false).expect("SVG scene");
        assert!(scene.svg.starts_with("<svg"));
        assert!(scene.svg.contains("<line"));
        assert!(scene.width <= 4096);
        assert!(scene.height <= 4096);
    }

    #[test]
    fn png_export_writes_a_valid_png() {
        let path = std::env::temp_dir().join(format!("rpp-export-{}.png", std::process::id()));
        export_png(&path, &test_plan(), &[], &Localizer::load(), false).expect("PNG export");
        let bytes = std::fs::read(&path).expect("PNG file");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn png_export_keeps_embedded_images() {
        let mut source = RgbaImage::new(2, 2);
        source
            .pixels_mut()
            .for_each(|pixel| *pixel = Rgba([230, 20, 10, 255]));
        let mut asset_bytes = Vec::new();
        source
            .write_to(&mut Cursor::new(&mut asset_bytes), ImageFormat::Png)
            .expect("source PNG");
        let asset = ProjectAsset {
            id: "asset-red".to_owned(),
            name: "red.png".to_owned(),
            extension: "png".to_owned(),
            bytes: asset_bytes,
        };
        let mut plan = PlanFile::default();
        plan.images.push(ImageObject {
            id: "image-red".to_owned(),
            asset_id: asset.id.clone(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            rotation: 0.0,
            locked: false,
        });
        let path =
            std::env::temp_dir().join(format!("rpp-image-export-{}.png", std::process::id()));
        export_png(&path, &plan, &[asset], &Localizer::load(), false).expect("PNG image export");
        let exported = image::open(&path).expect("exported PNG").to_rgba8();
        let center = exported.get_pixel(exported.width() / 2, exported.height() / 2);
        assert!(center[0] > 200 && center[1] < 50 && center[2] < 50);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn distance_labels_can_be_included_or_hidden() {
        let localizer = Localizer::load();
        let hidden = build_scene(&test_plan(), &[], &localizer, false).expect("SVG without labels");
        let visible = build_scene(&test_plan(), &[], &localizer, true).expect("SVG with labels");
        assert!(!hidden.svg.contains("4.00 m"));
        assert!(visible.svg.contains("4.00 m"));
    }

    #[test]
    fn custom_item_colors_are_kept_in_svg() {
        let mut plan = PlanFile::default();
        plan.lines.push(PlanLine {
            id: "blue-line".to_owned(),
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 0.0,
            color: [20, 90, 180],
            locked: false,
        });
        plan.texts.push(TextObject {
            id: "red-text".to_owned(),
            text: "Label".to_owned(),
            x: 50.0,
            y: 30.0,
            size: 12.0,
            rotation: 0.0,
            color: [180, 40, 70],
            bold: true,
            locked: false,
        });
        plan.objects.push(PlanObject {
            id: "green-object".to_owned(),
            name: String::new(),
            kind: ObjectKind::Rectangle,
            x: 50.0,
            y: 70.0,
            width: 40.0,
            height: 20.0,
            rotation: 0.0,
            fill_color: [30, 160, 80],
            stroke_color: [10, 80, 40],
            locked: false,
        });
        let scene = build_scene(&plan, &[], &Localizer::load(), false).expect("colored SVG");
        assert!(scene.svg.contains("#145ab4"));
        assert!(scene.svg.contains("#b42846"));
        assert!(scene.svg.contains(r#"font-family="Arial, sans-serif""#));
        assert!(scene.svg.contains(r#"font-weight="bold""#));
        assert!(scene.svg.contains("#1ea050"));
        assert!(scene.svg.contains("#0a5028"));
    }

    #[test]
    fn pdf_export_writes_one_scaled_a4_page() {
        let path = std::env::temp_dir().join(format!("rpp-a4-export-{}.pdf", std::process::id()));
        export_pdf_a4(&path, &test_plan(), &[], &Localizer::load(), false, 50)
            .expect("A4 PDF export");
        let bytes = std::fs::read(&path).expect("PDF file");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(!bytes.windows(15).any(|window| window == b"/Subtype /Image"));
        assert!(String::from_utf8_lossy(&bytes).contains("/Count 1"));
        if std::env::var_os("RPP_KEEP_TEST_PDF").is_some() {
            println!("kept PDF at {}", path.display());
        } else {
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn pdf_export_refuses_a_scale_that_would_clip() {
        let path =
            std::env::temp_dir().join(format!("rpp-clipped-export-{}.pdf", std::process::id()));
        let result = export_pdf_a4(&path, &test_plan(), &[], &Localizer::load(), false, 1);
        assert!(result.is_err());
        assert!(!path.exists());
    }

    #[test]
    fn layer_order_controls_export_stacking_across_different_item_types() {
        let mut plan = test_plan();
        plan.layers.push(crate::model::Layer {
            id: "top".into(),
            name: "Top".into(),
            ..crate::model::Layer::default()
        });
        plan.walls[0].color = [1, 2, 3];
        plan.assign_to_layer(&plan.walls[0].id.clone(), "top");
        plan.texts.push(TextObject {
            id: "label".into(),
            text: "below-the-wall".into(),
            x: 30.0,
            y: 30.0,
            size: 12.0,
            rotation: 0.0,
            color: [0, 0, 0],
            bold: false,
            locked: false,
        });
        let scene = build_scene(&plan, &[], &Localizer::load(), false).expect("scene");
        assert!(
            scene.svg.find("below-the-wall").expect("text")
                < scene.svg.find("#010203").expect("wall")
        );
        plan.layers.swap(0, 1);
        let scene = build_scene(&plan, &[], &Localizer::load(), false).expect("scene");
        assert!(
            scene.svg.find("#010203").expect("wall")
                < scene.svg.find("below-the-wall").expect("text")
        );
    }

    #[test]
    fn hidden_parent_walls_do_not_leave_open_svg_groups_or_orphan_openings() {
        let mut plan = test_plan();
        plan.layers.push(crate::model::Layer {
            id: "hidden".into(),
            name: "Hidden".into(),
            visible: false,
            ..crate::model::Layer::default()
        });
        let wall_id = plan.walls[0].id.clone();
        plan.assign_to_layer(&wall_id, "hidden");
        plan.doors.push(crate::model::Door {
            id: "door".into(),
            wall_id,
            t: 0.5,
            width: 20.0,
            kind: crate::model::DoorKind::Single,
            hinge: crate::model::HingeSide::Left,
            swing: crate::model::SwingSide::Inward,
            color: [18, 52, 86],
            locked: false,
        });
        plan.lines.push(PlanLine {
            id: "visible-line".into(),
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 0.0,
            color: [0, 0, 0],
            locked: false,
        });
        let scene = build_scene(&plan, &[], &Localizer::load(), false).expect("scene");
        assert!(!scene.svg.contains("#123456"));
        assert!(resvg::usvg::roxmltree::Document::parse(&scene.svg).is_ok());
    }

    #[test]
    fn thick_walls_and_xml_control_characters_export_safely() {
        let mut plan = test_plan();
        plan.walls[0].thickness = 500.0;
        let (minimum, maximum) = super::export_bounds(&plan).expect("bounds");
        assert!(minimum.y <= plan.walls[0].y1 - 250.0);
        assert!(maximum.y >= plan.walls[0].y1 + 250.0);
        assert_eq!(super::escape_xml("A\0B & <"), "A\u{fffd}B &amp; &lt;");
    }
}
