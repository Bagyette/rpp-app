mod inspectors;
mod layers;
mod render;
mod rooms;

use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use eframe::egui::epaint::TextShape;
use eframe::egui::{
    Align, Align2, CentralPanel, Color32, ColorImage, CornerRadius, Event, FontDefinitions,
    FontFamily, FontId, Key, Layout, Margin, Mesh, Painter, Panel, PointerButton, Pos2, Rect,
    Response, Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureOptions, Theme, Ui, Vec2,
};
use serde::{Deserialize, Serialize};

use crate::format::{FormatError, decode_plan, encode_project};
use crate::localization::Localizer;
use crate::model::{
    AreaShape, ArrowStyle, CompassRose, DEFAULT_LAYER_ID, Door, DoorKind, HingeSide, ImageObject,
    Layer, ObjectKind, PlanArrow, PlanFile, PlanLine, PlanObject, Point, ProjectAsset, Room,
    RoomKind, RoomSource, Snapshot, SwingSide, TextObject, Wall, WallKind, Window, WindowKind,
    default_area_fill_color, default_area_stroke_color, default_arrow_color, default_compass_color,
    default_line_color, default_object_fill_color, default_object_stroke_color,
    default_opening_color, default_text_color, default_wall_color, default_wall_thickness,
    default_window_color,
};
use crate::{export, images, input, platform, storage};

const GRID: f32 = 20.0;
const MIN_WALL: f32 = 20.0;
const WALL_WIDTH: f32 = 14.0;
const MIN_ZOOM: f32 = 0.35;
const MAX_ZOOM: f32 = 3.0;
const DEFAULT_IMAGE_MAX_SIZE: f32 = 40.0;
const BUILTIN_EMERGENCY_EXIT_ID: &str = "builtin-emergency-exit";
const BUILTIN_EMERGENCY_EXIT_BYTES: &[u8] = include_bytes!("../assets/library/emergency-exit.png");
const IMAGE_LIMITS_KIB: [usize; 10] = [8, 16, 24, 32, 48, 64, 96, 128, 192, 256];
const ARIAL_REGULAR_FAMILY: &str = "RPP Arial";
const ARIAL_BOLD_FAMILY: &str = "RPP Arial Bold";
const ACCENT: Color32 = Color32::from_rgb(23, 105, 170);
const INK: Color32 = Color32::from_rgb(52, 56, 59);
const SURFACE: Color32 = Color32::from_rgb(247, 247, 245);
const CANVAS: Color32 = Color32::WHITE;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn make_id(prefix: &str) -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let counter = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{time}-{counter}")
}

fn default_image_dimensions(pixel_width: u32, pixel_height: u32) -> (f32, f32) {
    let largest = pixel_width.max(pixel_height).max(1) as f32;
    let scale = (DEFAULT_IMAGE_MAX_SIZE / largest).min(1.0);
    (pixel_width as f32 * scale, pixel_height as f32 * scale)
}

fn format_distance(centimeters: f32, unit: Unit) -> String {
    if unit == Unit::Ft {
        let total_inches = centimeters / 2.54;
        let feet = (total_inches / 12.0).floor();
        let inches = total_inches - feet * 12.0;
        return format!("{feet:.0}' {inches:.1}\"");
    }
    let value = unit.display_from_centimeters(centimeters);
    let precision = if unit == Unit::M { 3 } else { 1 };
    format!("{value:.precision$}{}", unit.suffix())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Wall,
    Line,
    Measure,
    Text,
    Door,
    Window,
    Object,
    Area,
    Room,
    Compass,
    Arrow,
}

impl Tool {
    fn instruction_key(self) -> &'static str {
        match self {
            Self::Select => "tool.select_instruction",
            Self::Wall => "tool.wall_instruction",
            Self::Line => "tool.line_instruction",
            Self::Measure => "tool.measure_instruction",
            Self::Text => "tool.text_instruction",
            Self::Door => "tool.door_instruction",
            Self::Window => "tool.window_instruction",
            Self::Object => "tool.object_instruction",
            Self::Area => "tool.area_instruction",
            Self::Room => "tool.room_instruction",
            Self::Compass => "tool.compass_instruction",
            Self::Arrow => "tool.arrow_instruction",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Selection {
    Wall(String),
    Line(String),
    Text(String),
    Door(String),
    Image(String),
    Window(String),
    Object(String),
    Area(String),
    Room(String),
    Compass(String),
    Arrow(String),
}

#[derive(Clone, Copy)]
enum Alignment {
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Unit {
    Mm,
    #[default]
    Cm,
    M,
    In,
    Ft,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AppTheme {
    #[default]
    Light,
    Dark,
    WarmDark,
}

impl Unit {
    fn display_from_centimeters(self, value: f32) -> f32 {
        match self {
            Self::Mm => value * 10.0,
            Self::Cm => value,
            Self::M => value / 100.0,
            Self::In => value / 2.54,
            Self::Ft => value / 30.48,
        }
    }

    fn to_centimeters(self, value: f32) -> f32 {
        match self {
            Self::Mm => value / 10.0,
            Self::Cm => value,
            Self::M => value * 100.0,
            Self::In => value * 2.54,
            Self::Ft => value * 30.48,
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Mm => " mm",
            Self::Cm => " cm",
            Self::M => " m",
            Self::In => " in",
            Self::Ft => " ft",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::Cm => "cm",
            Self::M => "m",
            Self::In => "in",
            Self::Ft => "ft",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct AppSettings {
    unit: Unit,
    grid_size: f32,
    grid_visible: bool,
    snap_enabled: bool,
    wall_thickness: f32,
    door_width: f32,
    window_width: f32,
    text_size: f32,
    wall_color: [u8; 3],
    area_fill_color: [u8; 3],
    pdf_paper: export::PaperFormat,
    image_limit_index: u8,
    theme: AppTheme,
    scroll_mode: input::ScrollMode,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            unit: Unit::Cm,
            grid_size: GRID,
            grid_visible: true,
            snap_enabled: true,
            wall_thickness: default_wall_thickness(),
            door_width: 90.0,
            window_width: 120.0,
            text_size: 16.0,
            wall_color: default_wall_color(),
            area_fill_color: default_area_fill_color(),
            pdf_paper: export::PaperFormat::A4,
            image_limit_index: 4,
            theme: AppTheme::Light,
            scroll_mode: input::ScrollMode::Auto,
        }
    }
}

impl AppSettings {
    fn sanitized(mut self) -> Self {
        fn bounded(value: f32, minimum: f32, maximum: f32, fallback: f32) -> f32 {
            if value.is_finite() {
                value.clamp(minimum, maximum)
            } else {
                fallback
            }
        }
        let defaults = Self::default();
        self.grid_size = bounded(self.grid_size, 0.1, 5000.0, defaults.grid_size);
        self.wall_thickness = bounded(self.wall_thickness, 0.1, 1000.0, defaults.wall_thickness);
        self.door_width = bounded(self.door_width, 20.0, 1000.0, defaults.door_width);
        self.window_width = bounded(self.window_width, 20.0, 1000.0, defaults.window_width);
        self.text_size = bounded(self.text_size, 1.0, 500.0, defaults.text_size);
        self.image_limit_index = self.image_limit_index.min(10);
        self
    }
}

fn cycle_hit_selection(hits: &[Selection], current: Option<&Selection>) -> Option<Selection> {
    if hits.is_empty() {
        return None;
    }
    let next_index = current
        .and_then(|selection| hits.iter().position(|hit| hit == selection))
        .map_or(0, |index| (index + 1) % hits.len());
    Some(hits[next_index].clone())
}

fn toggle_additive_selection(
    primary: &mut Option<Selection>,
    additional: &mut Vec<Selection>,
    clicked: Selection,
) {
    if primary.as_ref() == Some(&clicked) {
        *primary = additional.pop();
    } else if let Some(index) = additional
        .iter()
        .position(|selection| selection == &clicked)
    {
        additional.remove(index);
    } else if primary.is_none() {
        *primary = Some(clicked);
    } else {
        additional.push(clicked);
    }
}

fn selectable_selections(plan: &PlanFile) -> Vec<Selection> {
    let mut selections = Vec::new();
    selections.extend(
        plan.walls
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Wall(item.id.clone())),
    );
    selections.extend(
        plan.lines
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Line(item.id.clone())),
    );
    selections.extend(
        plan.texts
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Text(item.id.clone())),
    );
    selections.extend(
        plan.doors
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Door(item.id.clone())),
    );
    selections.extend(
        plan.images
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Image(item.id.clone())),
    );
    selections.extend(
        plan.windows
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Window(item.id.clone())),
    );
    selections.extend(
        plan.objects
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Object(item.id.clone())),
    );
    selections.extend(
        plan.areas
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Area(item.id.clone())),
    );
    selections.extend(
        plan.compasses
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Compass(item.id.clone())),
    );
    selections.extend(
        plan.arrows
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Arrow(item.id.clone())),
    );
    selections.extend(
        plan.rooms
            .iter()
            .filter(|item| !item.locked && plan.item_editable(&item.id))
            .map(|item| Selection::Room(item.id.clone())),
    );
    selections
}

#[derive(Clone)]
enum PendingAction {
    New,
    Open,
    OpenPath(PathBuf),
    Close,
}

#[derive(Clone)]
struct DraftWall {
    start: Point,
    end: Point,
}

#[derive(Clone)]
enum WallDragKind {
    Move,
    Start,
    End,
}

#[derive(Clone)]
struct WallDrag {
    wall_id: String,
    kind: WallDragKind,
    pointer_start: Point,
    original: Wall,
    before: Snapshot,
}

#[derive(Clone)]
struct LineDrag {
    line_id: String,
    pointer_start: Point,
    original: PlanLine,
    before: Snapshot,
}

#[derive(Clone)]
struct TextDrag {
    text_id: String,
    pointer_start: Point,
    original: TextObject,
    before: Snapshot,
}

#[derive(Clone)]
struct DoorDrag {
    door_id: String,
    wall_id: String,
    before: Snapshot,
}

#[derive(Clone)]
struct WindowDrag {
    window_id: String,
    wall_id: String,
    before: Snapshot,
}

#[derive(Clone)]
struct ImageDrag {
    image_id: String,
    pointer_start: Point,
    original: ImageObject,
    before: Snapshot,
}

#[derive(Clone)]
struct ObjectDrag {
    object_id: String,
    pointer_start: Point,
    original: PlanObject,
    resize_handle: Option<usize>,
    before: Snapshot,
}

#[derive(Clone)]
struct ArrowDrag {
    arrow_id: String,
    pointer_start: Point,
    original: PlanArrow,
    before: Snapshot,
}

#[derive(Clone)]
struct AreaDrag {
    area_id: String,
    pointer_start: Point,
    original: AreaShape,
    before: Snapshot,
}

#[derive(Clone)]
struct RoomDrag {
    room_id: String,
    pointer_start: Point,
    original: Room,
    before: Snapshot,
}

#[derive(Clone)]
struct CompassDrag {
    compass_id: String,
    pointer_start: Point,
    original: CompassRose,
    before: Snapshot,
}

#[derive(Clone)]
struct GroupDrag {
    selections: Vec<Selection>,
    last_pointer: Point,
    before: Snapshot,
}

pub struct RppApp {
    context: egui::Context,
    plan: PlanFile,
    assets: Arc<Vec<ProjectAsset>>,
    textures: HashMap<String, TextureHandle>,
    undo_icon: TextureHandle,
    redo_icon: TextureHandle,
    tool: Tool,
    selection: Option<Selection>,
    additional_selections: Vec<Selection>,
    draft: Option<DraftWall>,
    line_draft: Option<DraftWall>,
    measurement: Option<DraftWall>,
    arrow_draft: Option<DraftWall>,
    selection_box: Option<DraftWall>,
    area_draft: Vec<Point>,
    room_draft: Vec<Point>,
    wall_press_origin: Option<Point>,
    wall_press_created_draft: bool,
    wall_drag: Option<WallDrag>,
    line_drag: Option<LineDrag>,
    text_drag: Option<TextDrag>,
    door_drag: Option<DoorDrag>,
    window_drag: Option<WindowDrag>,
    image_drag: Option<ImageDrag>,
    object_drag: Option<ObjectDrag>,
    arrow_drag: Option<ArrowDrag>,
    area_drag: Option<AreaDrag>,
    room_drag: Option<RoomDrag>,
    compass_drag: Option<CompassDrag>,
    group_drag: Option<GroupDrag>,
    batching_edits: bool,
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
    zoom: f32,
    pan: Vec2,
    panning_canvas: bool,
    grid_size: f32,
    grid_visible: bool,
    snap_enabled: bool,
    cursor_world: Option<Point>,
    localizer: Localizer,
    status: String,
    current_file: Option<PathBuf>,
    recovery_path: Option<PathBuf>,
    recovery_error: Option<String>,
    room_detection_limited: bool,
    dirty: bool,
    pending_action: Option<PendingAction>,
    export_distances: bool,
    pdf_scale: u32,
    pdf_paper: export::PaperFormat,
    unit: Unit,
    default_wall_thickness: f32,
    default_door_width: f32,
    default_window_width: f32,
    default_text_size: f32,
    default_wall_color: [u8; 3],
    default_area_fill_color: [u8; 3],
    image_limit_index: u8,
    settings_open: bool,
    layers_open: bool,
    active_layer_id: String,
    theme: AppTheme,
    scroll_mode: input::ScrollMode,
}

impl RppApp {
    fn panel_fill(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => SURFACE,
            AppTheme::Dark => Color32::from_rgb(31, 32, 33),
            AppTheme::WarmDark => Color32::from_rgb(39, 38, 35),
        }
    }

    fn secondary_panel_fill(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(244, 244, 242),
            AppTheme::Dark => Color32::from_rgb(38, 39, 40),
            AppTheme::WarmDark => Color32::from_rgb(48, 46, 41),
        }
    }

    fn canvas_fill(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => CANVAS,
            AppTheme::Dark => Color32::from_rgb(25, 26, 27),
            AppTheme::WarmDark => Color32::from_rgb(33, 32, 29),
        }
    }

    fn grid_color(&self, major: bool) -> Color32 {
        match (self.theme, major) {
            (AppTheme::Light, true) => Color32::from_rgb(211, 215, 218),
            (AppTheme::Light, false) => Color32::from_rgb(236, 238, 237),
            (AppTheme::Dark, true) => Color32::from_rgb(58, 61, 63),
            (AppTheme::Dark, false) => Color32::from_rgb(42, 44, 46),
            (AppTheme::WarmDark, true) => Color32::from_rgb(65, 62, 55),
            (AppTheme::WarmDark, false) => Color32::from_rgb(48, 46, 41),
        }
    }

    fn muted_text_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_gray(105),
            AppTheme::Dark => Color32::from_gray(175),
            AppTheme::WarmDark => Color32::from_rgb(190, 184, 169),
        }
    }

    fn subtle_text_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_gray(135),
            AppTheme::Dark => Color32::from_gray(145),
            AppTheme::WarmDark => Color32::from_rgb(155, 150, 138),
        }
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::from_context(cc.egui_ctx.clone());
        if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) {
            app.request_open_path(path);
        }
        app
    }

    fn from_context(context: egui::Context) -> Self {
        let settings = load_settings().sanitized();
        configure_style(&context, settings.theme);
        let mut localizer = Localizer::load();
        if let Some(path) = language_path()
            && let Ok(language) = fs::read_to_string(path)
        {
            localizer.set_language(language.trim());
        }
        let initial_status = localizer.text("app.ready");
        let undo_icon = load_embedded_icon(
            &context,
            "undo-icon",
            include_bytes!("../assets/icons/undo.png"),
        );
        let redo_icon = load_embedded_icon(
            &context,
            "redo-icon",
            include_bytes!("../assets/icons/redo.png"),
        );

        let mut app = Self {
            context: context.clone(),
            plan: PlanFile::default(),
            assets: Arc::new(Vec::new()),
            textures: HashMap::new(),
            undo_icon,
            redo_icon,
            tool: Tool::Wall,
            selection: None,
            additional_selections: Vec::new(),
            draft: None,
            line_draft: None,
            measurement: None,
            arrow_draft: None,
            selection_box: None,
            area_draft: Vec::new(),
            room_draft: Vec::new(),
            wall_press_origin: None,
            wall_press_created_draft: false,
            wall_drag: None,
            line_drag: None,
            text_drag: None,
            door_drag: None,
            window_drag: None,
            image_drag: None,
            object_drag: None,
            arrow_drag: None,
            area_drag: None,
            room_drag: None,
            compass_drag: None,
            group_drag: None,
            batching_edits: false,
            past: Vec::new(),
            future: Vec::new(),
            zoom: 1.0,
            pan: Vec2::new(80.0, 70.0),
            panning_canvas: false,
            grid_size: settings.grid_size,
            grid_visible: settings.grid_visible,
            snap_enabled: settings.snap_enabled,
            cursor_world: None,
            localizer,
            status: initial_status,
            current_file: None,
            recovery_path: autosave_path(),
            recovery_error: None,
            room_detection_limited: false,
            dirty: false,
            pending_action: None,
            export_distances: true,
            pdf_scale: 50,
            pdf_paper: settings.pdf_paper,
            unit: settings.unit,
            default_wall_thickness: settings.wall_thickness,
            default_door_width: settings.door_width,
            default_window_width: settings.window_width,
            default_text_size: settings.text_size,
            default_wall_color: settings.wall_color,
            default_area_fill_color: settings.area_fill_color,
            image_limit_index: settings.image_limit_index.min(10),
            settings_open: false,
            layers_open: false,
            active_layer_id: DEFAULT_LAYER_ID.to_owned(),
            theme: settings.theme,
            scroll_mode: settings.scroll_mode,
        };

        app.rebuild_textures(&context);
        if let Some(path) = &app.recovery_path
            && let Ok(bytes) = storage::read_limited(path, crate::format::MAX_PROJECT_BYTES)
            && let Ok(decoded) = decode_plan(&bytes)
        {
            app.plan = decoded.plan;
            app.assets = Arc::new(decoded.assets);
            app.refresh_automatic_rooms();
            app.rebuild_textures(&context);
            app.status = app.localizer.text("app.restored");
            app.dirty = true;
        }

        app
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot::from_project(&self.plan, &self.assets)
    }

    fn selected_items(&self) -> Vec<Selection> {
        let mut items = Vec::new();
        if let Some(primary) = self.selection.clone() {
            items.push(primary);
        }
        items.extend(self.additional_selections.clone());
        items
    }

    fn selection_locked(&self, selection: &Selection) -> bool {
        match selection {
            Selection::Wall(id) => self
                .plan
                .walls
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Line(id) => self
                .plan
                .lines
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Text(id) => self
                .plan
                .texts
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Door(id) => self
                .plan
                .doors
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Window(id) => self
                .plan
                .windows
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Image(id) => self
                .plan
                .images
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Object(id) => self
                .plan
                .objects
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Area(id) => self
                .plan
                .areas
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Room(id) => self
                .plan
                .rooms
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Compass(id) => self
                .plan
                .compasses
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
            Selection::Arrow(id) => self
                .plan
                .arrows
                .iter()
                .find(|item| &item.id == id)
                .is_none_or(|item| item.locked),
        }
    }

    fn selection_editable(&self, selection: &Selection) -> bool {
        self.plan.item_editable(layers::selection_id(selection))
            && !self.selection_locked(selection)
    }

    fn has_active_drag(&self) -> bool {
        self.wall_drag.is_some()
            || self.line_drag.is_some()
            || self.text_drag.is_some()
            || self.door_drag.is_some()
            || self.window_drag.is_some()
            || self.image_drag.is_some()
            || self.object_drag.is_some()
            || self.arrow_drag.is_some()
            || self.area_drag.is_some()
            || self.room_drag.is_some()
            || self.compass_drag.is_some()
            || self.group_drag.is_some()
    }

    fn cancel_interaction(&mut self) {
        let before = self
            .wall_drag
            .as_ref()
            .map(|drag| drag.before.clone())
            .or_else(|| self.line_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.text_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.door_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.window_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.image_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.object_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.arrow_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.area_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.room_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.compass_drag.as_ref().map(|drag| drag.before.clone()))
            .or_else(|| self.group_drag.as_ref().map(|drag| drag.before.clone()));
        if let Some(before) = before {
            before.restore(&mut self.plan, &mut self.assets);
        }
        self.clear_interaction();
    }

    fn clear_interaction(&mut self) {
        self.draft = None;
        self.line_draft = None;
        self.arrow_draft = None;
        self.measurement = None;
        self.selection_box = None;
        self.area_draft.clear();
        self.room_draft.clear();
        self.wall_press_origin = None;
        self.wall_press_created_draft = false;
        self.wall_drag = None;
        self.line_drag = None;
        self.text_drag = None;
        self.door_drag = None;
        self.window_drag = None;
        self.image_drag = None;
        self.object_drag = None;
        self.arrow_drag = None;
        self.area_drag = None;
        self.room_drag = None;
        self.compass_drag = None;
        self.group_drag = None;
        self.panning_canvas = false;
    }

    fn set_selected_locked(&mut self, locked: bool) {
        let before = self.snapshot();
        for selection in self.selected_items() {
            if !self.plan.item_editable(layers::selection_id(&selection)) {
                continue;
            }
            match selection {
                Selection::Wall(id) => {
                    if let Some(item) = self.plan.walls.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Line(id) => {
                    if let Some(item) = self.plan.lines.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Text(id) => {
                    if let Some(item) = self.plan.texts.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Door(id) => {
                    if let Some(item) = self.plan.doors.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Image(id) => {
                    if let Some(item) = self.plan.images.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Window(id) => {
                    if let Some(item) = self.plan.windows.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Object(id) => {
                    if let Some(item) = self.plan.objects.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Area(id) => {
                    if let Some(item) = self.plan.areas.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Room(id) => {
                    if let Some(item) = self.plan.rooms.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Compass(id) => {
                    if let Some(item) = self.plan.compasses.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
                Selection::Arrow(id) => {
                    if let Some(item) = self.plan.arrows.iter_mut().find(|item| item.id == id) {
                        item.locked = locked;
                    }
                }
            }
        }
        self.commit(before);
    }

    fn set_selected_color(&mut self, color: [u8; 3]) {
        let before = self.snapshot();
        for selection in self.selected_items() {
            if !self.selection_editable(&selection) {
                continue;
            }
            match selection {
                Selection::Wall(id) => self
                    .plan
                    .walls
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.color = color),
                Selection::Line(id) => self
                    .plan
                    .lines
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.color = color),
                Selection::Text(id) => self
                    .plan
                    .texts
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.color = color),
                Selection::Door(id) => self
                    .plan
                    .doors
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.color = color),
                Selection::Window(id) => self
                    .plan
                    .windows
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.color = color),
                Selection::Object(id) => self
                    .plan
                    .objects
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.stroke_color = color),
                Selection::Area(id) => self
                    .plan
                    .areas
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.stroke_color = color),
                Selection::Room(id) => self
                    .plan
                    .rooms
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.fill_color = color),
                Selection::Compass(id) => self
                    .plan
                    .compasses
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.color = color),
                Selection::Arrow(id) => self
                    .plan
                    .arrows
                    .iter_mut()
                    .find(|item| item.id == id)
                    .map(|item| item.color = color),
                Selection::Image(_) => None,
            };
        }
        self.commit(before);
    }

    fn align_selected(&mut self, alignment: Alignment) {
        let selections = self.selected_items();
        if selections.len() < 2 {
            return;
        }
        let bounds = selections
            .iter()
            .filter_map(|selection| selection_bounds(&self.plan, selection))
            .collect::<Vec<_>>();
        if bounds.len() < 2 {
            return;
        }
        let group_minimum = Point::new(
            bounds
                .iter()
                .map(|(minimum, _)| minimum.x)
                .fold(f32::INFINITY, f32::min),
            bounds
                .iter()
                .map(|(minimum, _)| minimum.y)
                .fold(f32::INFINITY, f32::min),
        );
        let group_maximum = Point::new(
            bounds
                .iter()
                .map(|(_, maximum)| maximum.x)
                .fold(f32::NEG_INFINITY, f32::max),
            bounds
                .iter()
                .map(|(_, maximum)| maximum.y)
                .fold(f32::NEG_INFINITY, f32::max),
        );
        let before = self.snapshot();
        let movable = selections
            .iter()
            .filter(|selection| !opening_follows_selected_wall(&self.plan, selection, &selections))
            .cloned()
            .collect::<Vec<_>>();
        for selection in &movable {
            let Some((minimum, maximum)) = selection_bounds(&self.plan, selection) else {
                continue;
            };
            let delta = match alignment {
                Alignment::Left => Point::new(group_minimum.x - minimum.x, 0.0),
                Alignment::HorizontalCenter => Point::new(
                    (group_minimum.x + group_maximum.x - minimum.x - maximum.x) * 0.5,
                    0.0,
                ),
                Alignment::Right => Point::new(group_maximum.x - maximum.x, 0.0),
                Alignment::Top => Point::new(0.0, group_minimum.y - minimum.y),
                Alignment::VerticalCenter => Point::new(
                    0.0,
                    (group_minimum.y + group_maximum.y - minimum.y - maximum.y) * 0.5,
                ),
                Alignment::Bottom => Point::new(0.0, group_maximum.y - maximum.y),
            };
            self.translate_selection(selection, delta);
        }
        self.commit(before);
        self.status = self.localizer.text("status.items_aligned");
    }

    fn persist_settings(&self) {
        save_settings(&AppSettings {
            unit: self.unit,
            grid_size: self.grid_size,
            grid_visible: self.grid_visible,
            snap_enabled: self.snap_enabled,
            wall_thickness: self.default_wall_thickness,
            door_width: self.default_door_width,
            window_width: self.default_window_width,
            text_size: self.default_text_size,
            wall_color: self.default_wall_color,
            area_fill_color: self.default_area_fill_color,
            pdf_paper: self.pdf_paper,
            image_limit_index: self.image_limit_index,
            theme: self.theme,
            scroll_mode: self.scroll_mode,
        });
    }

    fn rebuild_textures(&mut self, context: &egui::Context) {
        self.textures.clear();
        if let Ok((image, _, _)) = decode_image(BUILTIN_EMERGENCY_EXIT_BYTES) {
            self.textures.insert(
                BUILTIN_EMERGENCY_EXIT_ID.to_owned(),
                context.load_texture("builtin-emergency-exit", image, TextureOptions::LINEAR),
            );
        }
        for asset in self.assets.iter() {
            if let Ok((image, _, _)) = decode_image(&asset.bytes) {
                self.textures.insert(
                    asset.id.clone(),
                    context.load_texture(
                        format!("asset-{}", asset.id),
                        image,
                        TextureOptions::LINEAR,
                    ),
                );
            }
        }
    }

    fn format_error_text(&self, error: &FormatError) -> String {
        match error {
            FormatError::UnknownFormat => self.localizer.text("format.unknown"),
            FormatError::UnsupportedVersion(version) => self.localizer.format(
                "format.unsupported_version",
                &[("version", version.to_string())],
            ),
            FormatError::InvalidHeader => self.localizer.text("format.invalid_header"),
            FormatError::TooLarge => self.localizer.text("format.too_large"),
            FormatError::Corrupted => self.localizer.text("format.corrupted"),
            FormatError::InvalidPlan => self.localizer.text("format.invalid_plan"),
            FormatError::Syntax { line, .. } => self
                .localizer
                .format("format.syntax", &[("line", line.to_string())]),
        }
    }

    fn snap_point(&self, point: Point) -> Point {
        if !self.snap_enabled {
            return point;
        }
        let grid = Point::new(
            (point.x / self.grid_size).round() * self.grid_size,
            (point.y / self.grid_size).round() * self.grid_size,
        );
        let threshold = 12.0 / self.zoom;
        self.plan
            .walls
            .iter()
            .flat_map(|wall| [wall.start(), wall.point_at(0.5), wall.end()])
            .filter_map(|candidate| {
                let distance = point.distance(candidate);
                (distance <= threshold).then_some((candidate, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(candidate, _)| candidate)
            .unwrap_or(grid)
    }

    fn magnet_wall_endpoint(&self, point: Point) -> Point {
        magnet_to_wall(&self.plan.walls, point, 12.0 / self.zoom)
    }

    fn wall_snap_point(&self, point: Point) -> Point {
        self.magnet_wall_endpoint(self.snap_point(point))
    }

    fn snap_with_axis(&self, point: Point, start: Point, axis_lock: bool) -> Point {
        let mut point = self.snap_point(point);
        if axis_lock {
            if (point.x - start.x).abs() > (point.y - start.y).abs() {
                point.y = start.y;
            } else {
                point.x = start.x;
            }
        }
        point
    }

    fn fit_plan(&mut self, rect: Rect) {
        let Some((minimum, maximum)) = plan_bounds(&self.plan) else {
            self.zoom = 1.0;
            self.pan = Vec2::new(80.0, 70.0);
            self.status = self.localizer.text("status.view_reset");
            return;
        };

        let margin = 56.0;
        let available = (rect.size() - Vec2::splat(margin * 2.0)).max(Vec2::splat(1.0));
        let width = (maximum.x - minimum.x).max(1.0);
        let height = (maximum.y - minimum.y).max(1.0);
        self.zoom = (available.x / width)
            .min(available.y / height)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        let center = Point::new((minimum.x + maximum.x) * 0.5, (minimum.y + maximum.y) * 0.5);
        self.pan = rect.center() - rect.min - Vec2::new(center.x, center.y) * self.zoom;
        self.status = self.localizer.text("status.view_fitted");
    }

    fn nudge_selected(&mut self, delta: Point) {
        if !self.additional_selections.is_empty() {
            let before = self.snapshot();
            let selections = self.selected_items();
            let movable = selections
                .iter()
                .filter(|selection| {
                    !opening_follows_selected_wall(&self.plan, selection, &selections)
                })
                .cloned()
                .collect::<Vec<_>>();
            for selection in movable {
                self.translate_selection(&selection, delta);
            }
            self.commit(before);
            self.status = self.localizer.text("status.items_moved");
            return;
        }
        let Some(selection) = self.selection.clone() else {
            return;
        };
        if !self.selection_editable(&selection) {
            return;
        }
        let before = self.snapshot();
        let mut moved = false;

        match selection {
            Selection::Wall(id) => {
                if let Some(wall) = self.plan.walls.iter_mut().find(|wall| wall.id == id) {
                    if wall.locked {
                        self.status = self.localizer.text("status.wall_locked");
                        return;
                    }
                    wall.x1 += delta.x;
                    wall.y1 += delta.y;
                    wall.x2 += delta.x;
                    wall.y2 += delta.y;
                    moved = true;
                }
            }
            Selection::Line(id) => {
                if let Some(line) = self.plan.lines.iter_mut().find(|line| line.id == id) {
                    if line.locked {
                        self.status = self.localizer.text("status.line_locked");
                        return;
                    }
                    line.x1 += delta.x;
                    line.y1 += delta.y;
                    line.x2 += delta.x;
                    line.y2 += delta.y;
                    moved = true;
                }
            }
            Selection::Text(id) => {
                if let Some(text) = self.plan.texts.iter_mut().find(|text| text.id == id) {
                    if text.locked {
                        self.status = self.localizer.text("status.text_locked");
                        return;
                    }
                    text.x += delta.x;
                    text.y += delta.y;
                    moved = true;
                }
            }
            Selection::Image(id) => {
                if let Some(image) = self.plan.images.iter_mut().find(|image| image.id == id) {
                    if image.locked {
                        self.status = self.localizer.text("status.image_locked");
                        return;
                    }
                    image.x += delta.x;
                    image.y += delta.y;
                    moved = true;
                }
            }
            Selection::Object(id) => {
                if let Some(object) = self.plan.objects.iter_mut().find(|object| object.id == id) {
                    if object.locked {
                        self.status = self.localizer.text("status.object_locked");
                        return;
                    }
                    object.x += delta.x;
                    object.y += delta.y;
                    moved = true;
                }
            }
            Selection::Area(id) => {
                if let Some(area) = self.plan.areas.iter_mut().find(|area| area.id == id) {
                    if area.locked {
                        self.status = self.localizer.text("status.area_locked");
                        return;
                    }
                    for point in &mut area.points {
                        point.x += delta.x;
                        point.y += delta.y;
                    }
                    moved = true;
                }
            }
            Selection::Room(id) => {
                if let Some(room) = self.plan.rooms.iter_mut().find(|room| room.id == id) {
                    if room.locked || room.source == RoomSource::Automatic {
                        self.status = self.localizer.text("status.room_locked");
                        return;
                    }
                    for point in &mut room.points {
                        point.x += delta.x;
                        point.y += delta.y;
                    }
                    moved = true;
                }
            }
            Selection::Compass(id) => {
                if let Some(compass) = self
                    .plan
                    .compasses
                    .iter_mut()
                    .find(|compass| compass.id == id)
                {
                    if compass.locked {
                        self.status = self.localizer.text("status.compass_locked");
                        return;
                    }
                    compass.x += delta.x;
                    compass.y += delta.y;
                    moved = true;
                }
            }
            Selection::Arrow(id) => {
                if let Some(arrow) = self.plan.arrows.iter_mut().find(|arrow| arrow.id == id) {
                    if arrow.locked {
                        self.status = self.localizer.text("status.arrow_locked");
                        return;
                    }
                    arrow.x += delta.x;
                    arrow.y += delta.y;
                    moved = true;
                }
            }
            Selection::Door(id) => {
                let Some(door) = self.plan.doors.iter().find(|door| door.id == id).cloned() else {
                    return;
                };
                if door.locked {
                    self.status = self.localizer.text("status.door_locked");
                    return;
                }
                if delta.x == 0.0 {
                    return;
                }
                let Some(length) = self
                    .plan
                    .walls
                    .iter()
                    .find(|wall| wall.id == door.wall_id)
                    .map(Wall::length)
                else {
                    return;
                };
                let margin = door.width / (2.0 * length);
                let next_t = (door.t + delta.x / length).clamp(margin, 1.0 - margin);
                if !self.opening_available(
                    &door.wall_id,
                    next_t,
                    door.width,
                    Some(door.id.as_str()),
                ) {
                    self.status = self.localizer.text("status.opening_overlap");
                    return;
                }
                if let Some(target) = self.plan.doors.iter_mut().find(|item| item.id == id) {
                    target.t = next_t;
                    moved = true;
                }
            }
            Selection::Window(id) => {
                let Some(window) = self
                    .plan
                    .windows
                    .iter()
                    .find(|window| window.id == id)
                    .cloned()
                else {
                    return;
                };
                if window.locked {
                    self.status = self.localizer.text("status.window_locked");
                    return;
                }
                if delta.x == 0.0 {
                    return;
                }
                let Some(length) = self
                    .plan
                    .walls
                    .iter()
                    .find(|wall| wall.id == window.wall_id)
                    .map(Wall::length)
                else {
                    return;
                };
                let margin = window.width / (2.0 * length);
                let next_t = (window.t + delta.x / length).clamp(margin, 1.0 - margin);
                if !self.opening_available(
                    &window.wall_id,
                    next_t,
                    window.width,
                    Some(window.id.as_str()),
                ) {
                    self.status = self.localizer.text("status.opening_overlap");
                    return;
                }
                if let Some(target) = self.plan.windows.iter_mut().find(|item| item.id == id) {
                    target.t = next_t;
                    moved = true;
                }
            }
        }

        if moved {
            self.commit(before);
            self.status = self.localizer.format(
                "status.item_moved",
                &[
                    (
                        "distance",
                        format!(
                            "{:.2}",
                            self.unit
                                .display_from_centimeters(delta.x.abs().max(delta.y.abs()))
                        ),
                    ),
                    ("unit", self.unit.as_str().to_owned()),
                ],
            );
        }
    }

    fn opening_available(
        &self,
        wall_id: &str,
        position: f32,
        width: f32,
        excluded_id: Option<&str>,
    ) -> bool {
        let Some(length) = self
            .plan
            .walls
            .iter()
            .find(|wall| wall.id == wall_id)
            .map(Wall::length)
        else {
            return false;
        };
        if !width.is_finite()
            || !position.is_finite()
            || !length.is_finite()
            || length <= 0.0
            || width <= 0.0
            || width > length
        {
            return false;
        }
        let half_span = width / (2.0 * length);
        let start = position - half_span;
        let end = position + half_span;
        if start < 0.0 || end > 1.0 {
            return false;
        }
        self.plan
            .doors
            .iter()
            .filter(|door| door.wall_id == wall_id && excluded_id != Some(door.id.as_str()))
            .map(|door| {
                let half = door.width / (2.0 * length);
                (door.t - half, door.t + half)
            })
            .chain(
                self.plan
                    .windows
                    .iter()
                    .filter(|window| {
                        window.wall_id == wall_id && excluded_id != Some(window.id.as_str())
                    })
                    .map(|window| {
                        let half = window.width / (2.0 * length);
                        (window.t - half, window.t + half)
                    }),
            )
            .all(|(other_start, other_end)| end <= other_start || start >= other_end)
    }

    fn find_opening_position(&self, wall_id: &str, width: f32, preferred: f32) -> Option<f32> {
        let length = self
            .plan
            .walls
            .iter()
            .find(|wall| wall.id == wall_id)
            .map(Wall::length)?;
        if !width.is_finite() || width <= 0.0 || width > length || !preferred.is_finite() {
            return None;
        }
        let margin = width / (2.0 * length);
        (0..=20)
            .flat_map(|step| {
                let offset = step as f32 * 0.05;
                [preferred + offset, preferred - offset]
            })
            .map(|candidate| candidate.clamp(margin, 1.0 - margin))
            .find(|candidate| self.opening_available(wall_id, *candidate, width, None))
    }

    fn commit(&mut self, before: Snapshot) {
        if self.batching_edits {
            return;
        }
        if before.walls.len() != self.plan.walls.len()
            || before
                .walls
                .iter()
                .zip(&self.plan.walls)
                .any(|(before, after)| {
                    before.start() != after.start() || before.end() != after.end()
                })
        {
            self.refresh_automatic_rooms();
        }
        self.plan.prune_layer_assignments();
        if let Err(error) = crate::format::validate_plan(&self.plan) {
            before.restore(&mut self.plan, &mut self.assets);
            self.rebuild_textures(&self.context.clone());
            self.status = self.format_error_text(&error);
            return;
        }
        let after = self.snapshot();
        if before != after {
            self.past.push(before);
            if self.past.len() > 100 {
                self.past.remove(0);
            }
            self.future.clear();
            self.dirty = true;
            self.autosave();
        }
    }

    fn repair_openings_for_wall(&mut self, wall_id: &str) {
        let Some(length) = self
            .plan
            .walls
            .iter()
            .find(|wall| wall.id == wall_id)
            .map(Wall::length)
        else {
            return;
        };
        let before_count = self.plan.doors.len();
        let before_window_count = self.plan.windows.len();
        self.plan
            .doors
            .retain(|door| door.wall_id != wall_id || door.width <= length);
        for door in &mut self.plan.doors {
            if door.wall_id == wall_id && length > 0.0 {
                let margin = door.width / (2.0 * length);
                door.t = door.t.clamp(margin, 1.0 - margin);
            }
        }
        self.plan
            .windows
            .retain(|window| window.wall_id != wall_id || window.width <= length);
        for window in &mut self.plan.windows {
            if window.wall_id == wall_id && length > 0.0 {
                let margin = window.width / (2.0 * length);
                window.t = window.t.clamp(margin, 1.0 - margin);
            }
        }
        if self.plan.doors.len() != before_count {
            self.status = self.localizer.text("status.door_removed_short_wall");
        } else if self.plan.windows.len() != before_window_count {
            self.status = self.localizer.text("status.window_removed_short_wall");
        }
    }

    fn autosave(&mut self) {
        let Some(path) = &self.recovery_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let result = encode_project(&self.plan, self.assets.as_slice())
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                storage::write_atomic(path, &bytes).map_err(|error| error.to_string())
            });
        self.recovery_error = result.err().map(|error| {
            self.localizer
                .format("status.autosave_error", &[("error", error)])
        });
    }

    fn undo(&mut self) {
        self.cancel_interaction();
        let Some(previous) = self.past.pop() else {
            return;
        };
        self.future.push(self.snapshot());
        let assets_changed = !Arc::ptr_eq(&self.assets, &previous.assets);
        previous.restore(&mut self.plan, &mut self.assets);
        if assets_changed {
            self.rebuild_textures(&self.context.clone());
        }
        self.clear_interaction();
        self.selection = None;
        self.additional_selections.clear();
        self.dirty = true;
        self.status = self.localizer.text("status.undo");
        self.autosave();
    }

    fn redo(&mut self) {
        self.cancel_interaction();
        let Some(next) = self.future.pop() else {
            return;
        };
        self.past.push(self.snapshot());
        let assets_changed = !Arc::ptr_eq(&self.assets, &next.assets);
        next.restore(&mut self.plan, &mut self.assets);
        if assets_changed {
            self.rebuild_textures(&self.context.clone());
        }
        self.clear_interaction();
        self.selection = None;
        self.additional_selections.clear();
        self.dirty = true;
        self.status = self.localizer.text("status.redo");
        self.autosave();
    }

    fn new_plan(&mut self) {
        self.plan = PlanFile::default();
        self.active_layer_id = DEFAULT_LAYER_ID.to_owned();
        self.room_detection_limited = false;
        self.assets = Arc::new(Vec::new());
        self.textures.clear();
        self.selection = None;
        self.additional_selections.clear();
        self.clear_interaction();
        self.rebuild_textures(&self.context.clone());
        self.current_file = None;
        self.past.clear();
        self.future.clear();
        self.dirty = false;
        self.status = self.localizer.text("status.new_plan");
        self.autosave();
    }

    fn request_new_plan(&mut self) {
        if self.dirty {
            self.pending_action = Some(PendingAction::New);
        } else {
            self.new_plan();
        }
    }

    fn request_open_plan(&mut self) {
        if self.dirty {
            self.pending_action = Some(PendingAction::Open);
        } else {
            self.open_plan();
        }
    }

    fn request_open_path(&mut self, path: PathBuf) {
        if self.dirty {
            self.pending_action = Some(PendingAction::OpenPath(path));
        } else {
            self.open_path(path);
        }
    }

    fn run_pending_action(&mut self, ctx: &egui::Context, action: PendingAction) {
        self.pending_action = None;
        match action {
            PendingAction::New => self.new_plan(),
            PendingAction::Open => self.open_plan(),
            PendingAction::OpenPath(path) => self.open_path(path),
            PendingAction::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn confirm_discard_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_action.clone() else {
            return;
        };
        egui::Modal::new(egui::Id::new("unsaved-changes")).show(ctx, |ui| {
            ui.heading(self.localizer.text("dialog.unsaved_title"));
            ui.label(self.localizer.text("dialog.unsaved_body"));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(self.localizer.text("action.save")).clicked() {
                    self.save_plan();
                    if !self.dirty {
                        self.run_pending_action(ctx, action.clone());
                    }
                }
                if ui.button(self.localizer.text("action.discard")).clicked() {
                    self.dirty = false;
                    self.run_pending_action(ctx, action.clone());
                }
                if ui.button(self.localizer.text("action.cancel")).clicked() {
                    self.pending_action = None;
                }
            });
        });
    }

    fn open_plan(&mut self) {
        let title = self.localizer.text("dialog.open_title");
        let filter = self.localizer.text("file.filter");
        let Some(path) = rfd::FileDialog::new()
            .set_title(title)
            .add_filter(filter, &["rpp"])
            .pick_file()
        else {
            return;
        };

        self.open_path(path);
    }

    fn open_path(&mut self, path: PathBuf) {
        match storage::read_limited(&path, crate::format::MAX_PROJECT_BYTES) {
            Ok(bytes) => match decode_plan(&bytes) {
                Ok(decoded) => {
                    let needs_conversion = decoded.needs_conversion;
                    self.plan = decoded.plan;
                    self.refresh_automatic_rooms();
                    self.active_layer_id = DEFAULT_LAYER_ID.to_owned();
                    self.assets = Arc::new(decoded.assets);
                    let context = self.context.clone();
                    self.rebuild_textures(&context);
                    self.selection = None;
                    self.additional_selections.clear();
                    self.clear_interaction();
                    self.past.clear();
                    self.future.clear();
                    self.current_file = Some(path.clone());
                    self.dirty = needs_conversion;
                    self.status = if needs_conversion {
                        self.localizer.text("status.legacy_opened")
                    } else {
                        self.localizer
                            .format("status.opened", &[("path", path.display().to_string())])
                    };
                    self.autosave();
                }
                Err(error) => {
                    let error = self.format_error_text(&error);
                    self.status = self
                        .localizer
                        .format("status.invalid_file", &[("error", error)])
                }
            },
            Err(error) => {
                self.status = self
                    .localizer
                    .format("status.read_error", &[("error", error.to_string())])
            }
        }
    }

    fn save_plan(&mut self) {
        let title = self.localizer.text("dialog.save_title");
        let filename = self.localizer.text("dialog.default_filename");
        let filter = self.localizer.text("file.filter");
        let path = self.current_file.clone().or_else(|| {
            rfd::FileDialog::new()
                .set_title(title)
                .set_file_name(filename)
                .add_filter(filter, &["rpp"])
                .save_file()
        });
        let Some(path) = path else {
            return;
        };
        self.save_to(path);
    }

    fn save_plan_as(&mut self) {
        let title = self.localizer.text("dialog.save_as_title");
        let filename = self.localizer.text("dialog.default_filename");
        let filter = self.localizer.text("file.filter");
        let Some(path) = rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(filename)
            .add_filter(filter, &["rpp"])
            .save_file()
        else {
            return;
        };
        self.save_to(path);
    }

    fn export_svg(&mut self) {
        self.export_image("svg");
    }

    fn export_png(&mut self) {
        self.export_image("png");
    }

    fn export_pdf(&mut self) {
        if self.plan.walls.is_empty()
            && self.plan.lines.is_empty()
            && self.plan.texts.is_empty()
            && self.plan.objects.is_empty()
            && self.plan.images.is_empty()
            && self.plan.areas.is_empty()
            && self.plan.compasses.is_empty()
            && self.plan.arrows.is_empty()
            && self.plan.rooms.is_empty()
        {
            self.status = self.localizer.text("status.nothing_to_export");
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.localizer.text("dialog.export_pdf_title"))
            .set_file_name(self.localizer.text("dialog.default_pdf_filename"))
            .add_filter(self.localizer.text("file.pdf_filter"), &["pdf"])
            .save_file()
        else {
            return;
        };
        self.status = match export::export_pdf(
            &path,
            &self.plan,
            self.assets.as_slice(),
            &self.localizer,
            self.export_distances,
            self.pdf_scale,
            self.pdf_paper,
        ) {
            Ok(()) => self
                .localizer
                .format("status.exported", &[("path", path.display().to_string())]),
            Err(error) => self.localizer.format(
                "status.export_error",
                &[("format", "PDF".to_owned()), ("error", error)],
            ),
        };
    }

    fn export_image(&mut self, format: &str) {
        if self.plan.walls.is_empty()
            && self.plan.lines.is_empty()
            && self.plan.texts.is_empty()
            && self.plan.objects.is_empty()
            && self.plan.images.is_empty()
            && self.plan.areas.is_empty()
            && self.plan.compasses.is_empty()
            && self.plan.arrows.is_empty()
            && self.plan.rooms.is_empty()
        {
            self.status = self.localizer.text("status.nothing_to_export");
            return;
        }
        let (title_key, filename_key, filter_key) = match format {
            "svg" => (
                "dialog.export_svg_title",
                "dialog.default_svg_filename",
                "file.svg_filter",
            ),
            _ => (
                "dialog.export_png_title",
                "dialog.default_png_filename",
                "file.png_filter",
            ),
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.localizer.text(title_key))
            .set_file_name(self.localizer.text(filename_key))
            .add_filter(self.localizer.text(filter_key), &[format])
            .save_file()
        else {
            return;
        };
        let result = match format {
            "svg" => export::export_svg(
                &path,
                &self.plan,
                self.assets.as_slice(),
                &self.localizer,
                self.export_distances,
            ),
            _ => export::export_png(
                &path,
                &self.plan,
                self.assets.as_slice(),
                &self.localizer,
                self.export_distances,
            ),
        };
        self.status = match result {
            Ok(()) => self
                .localizer
                .format("status.exported", &[("path", path.display().to_string())]),
            Err(error) => self.localizer.format(
                "status.export_error",
                &[("format", format.to_uppercase()), ("error", error)],
            ),
        };
    }

    fn save_to(&mut self, path: PathBuf) {
        let result = encode_project(&self.plan, self.assets.as_slice())
            .map_err(|error| self.format_error_text(&error))
            .and_then(|bytes| write_plan_safely(&path, &bytes).map_err(|error| error.to_string()));
        match result {
            Ok(()) => {
                self.current_file = Some(path.clone());
                self.dirty = false;
                self.status = self
                    .localizer
                    .format("status.saved", &[("path", path.display().to_string())]);
            }
            Err(error) => {
                self.status = self
                    .localizer
                    .format("status.save_error", &[("error", error)])
            }
        }
    }

    fn import_image(&mut self) {
        let title = self.localizer.text("dialog.import_image_title");
        let filter = self.localizer.text("file.image_filter");
        let Some(path) = rfd::FileDialog::new()
            .set_title(title)
            .add_filter(filter, &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
            .pick_file()
        else {
            return;
        };
        let Ok(bytes) = storage::read_limited(&path, images::MAX_ENCODED_BYTES) else {
            self.status = self.localizer.text("status.image_read_error");
            return;
        };
        if bytes.len() > 32 * 1024 * 1024 {
            self.status = self.localizer.text("status.image_too_large");
            return;
        }
        let limit = if self.image_limit_index as usize >= IMAGE_LIMITS_KIB.len() {
            None
        } else {
            Some(IMAGE_LIMITS_KIB[self.image_limit_index as usize] * 1024)
        };
        let Ok((bytes, extension, color_image, pixel_width, pixel_height)) =
            process_imported_image(&bytes, limit)
        else {
            self.status = self.localizer.text("status.image_invalid");
            return;
        };

        if let Some(asset_id) = self
            .assets
            .iter()
            .find(|asset| asset.bytes == bytes)
            .map(|asset| asset.id.clone())
        {
            self.place_image_instance(&asset_id, pixel_width, pixel_height);
            self.status = self.localizer.text("status.image_reused");
            return;
        }

        let decoded_total: u64 = self
            .assets
            .iter()
            .filter_map(|asset| images::decoded_byte_len(&asset.bytes))
            .sum();
        if self.assets.len() >= 256
            || decoded_total + u64::from(pixel_width) * u64::from(pixel_height) * 4
                > images::MAX_DECODED_BYTES
        {
            self.status = self.localizer.text("status.image_too_large");
            return;
        }

        let before = self.snapshot();
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(sanitize_asset_name)
            .unwrap_or_else(|| "image".to_owned());
        let asset_id = make_id("asset");
        self.textures.insert(
            asset_id.clone(),
            self.context.load_texture(
                format!("asset-{asset_id}"),
                color_image,
                TextureOptions::LINEAR,
            ),
        );
        Arc::make_mut(&mut self.assets).push(ProjectAsset {
            id: asset_id.clone(),
            name,
            extension,
            bytes,
        });
        self.add_image_instance(&asset_id, pixel_width, pixel_height);
        self.commit(before);
        self.status = self.localizer.text("status.image_imported");
    }

    fn reuse_asset(&mut self, asset_id: &str) {
        if asset_id == BUILTIN_EMERGENCY_EXIT_ID {
            self.place_image_instance(asset_id, 96, 96);
            self.status = self.localizer.text("status.image_reused");
            return;
        }
        let dimensions = self
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .and_then(|asset| decode_image(&asset.bytes).ok())
            .map(|(_, width, height)| (width, height));
        if let Some((width, height)) = dimensions {
            self.place_image_instance(asset_id, width, height);
            self.status = self.localizer.text("status.image_reused");
        }
    }

    fn place_image_instance(&mut self, asset_id: &str, pixel_width: u32, pixel_height: u32) {
        let before = self.snapshot();
        self.add_image_instance(asset_id, pixel_width, pixel_height);
        self.commit(before);
    }

    fn add_image_instance(&mut self, asset_id: &str, pixel_width: u32, pixel_height: u32) {
        let (width, height) = default_image_dimensions(pixel_width, pixel_height);
        let image = ImageObject {
            id: make_id("image"),
            asset_id: asset_id.to_owned(),
            x: 260.0,
            y: 220.0,
            width,
            height,
            rotation: 0.0,
            locked: false,
        };
        let id = image.id.clone();
        self.plan.images.push(image);
        self.assign_to_active_layer(&id);
        self.selection = Some(Selection::Image(id));
    }

    fn remove_asset(&mut self, asset_id: &str) {
        let before = self.snapshot();
        let removed_instances = self
            .plan
            .images
            .iter()
            .filter(|image| image.asset_id == asset_id)
            .count();
        self.plan.images.retain(|image| image.asset_id != asset_id);
        Arc::make_mut(&mut self.assets).retain(|asset| asset.id != asset_id);
        self.textures.remove(asset_id);
        if matches!(
            &self.selection,
            Some(Selection::Image(id))
                if !self.plan.images.iter().any(|image| &image.id == id)
        ) {
            self.selection = None;
        }
        self.status = self.localizer.format(
            "status.asset_deleted",
            &[("count", removed_instances.to_string())],
        );
        self.commit(before);
    }

    fn remove_selected(&mut self) {
        let Some(primary) = self.selection.clone() else {
            return;
        };
        let mut selections = vec![primary];
        selections.extend(self.additional_selections.clone());
        selections.retain(|selection| self.selection_editable(selection));
        let removed_count = selections.len();
        if selections.is_empty() {
            return;
        }
        let before = self.snapshot();
        for selection in selections {
            match selection {
                Selection::Wall(id) => {
                    self.plan.walls.retain(|wall| wall.id != id);
                    self.plan.doors.retain(|door| door.wall_id != id);
                    self.plan.windows.retain(|window| window.wall_id != id);
                    self.status = self.localizer.text("status.wall_deleted");
                }
                Selection::Line(id) => {
                    self.plan.lines.retain(|line| line.id != id);
                    self.status = self.localizer.text("status.line_deleted");
                }
                Selection::Text(id) => {
                    self.plan.texts.retain(|text| text.id != id);
                    self.status = self.localizer.text("status.text_deleted");
                }
                Selection::Door(id) => {
                    self.plan.doors.retain(|door| door.id != id);
                    self.status = self.localizer.text("status.door_deleted");
                }
                Selection::Image(id) => {
                    self.plan.images.retain(|image| image.id != id);
                    self.status = self.localizer.text("status.image_deleted");
                }
                Selection::Window(id) => {
                    self.plan.windows.retain(|window| window.id != id);
                    self.status = self.localizer.text("status.window_deleted");
                }
                Selection::Object(id) => {
                    self.plan.objects.retain(|object| object.id != id);
                    self.status = self.localizer.text("status.object_deleted");
                }
                Selection::Area(id) => {
                    self.plan.areas.retain(|area| area.id != id);
                    self.status = self.localizer.text("status.area_deleted");
                }
                Selection::Room(id) => {
                    if self
                        .plan
                        .rooms
                        .iter()
                        .any(|room| room.id == id && room.source == RoomSource::Manual)
                    {
                        self.plan.rooms.retain(|room| room.id != id);
                        self.status = self.localizer.text("status.room_deleted");
                    }
                }
                Selection::Compass(id) => {
                    self.plan.compasses.retain(|compass| compass.id != id);
                    self.status = self.localizer.text("status.compass_deleted");
                }
                Selection::Arrow(id) => {
                    self.plan.arrows.retain(|arrow| arrow.id != id);
                    self.status = self.localizer.text("status.arrow_deleted");
                }
            }
        }
        self.refresh_automatic_rooms();
        let existing_ids = self
            .plan
            .walls
            .iter()
            .map(|item| item.id.as_str())
            .chain(self.plan.lines.iter().map(|item| item.id.as_str()))
            .chain(self.plan.doors.iter().map(|item| item.id.as_str()))
            .chain(self.plan.windows.iter().map(|item| item.id.as_str()))
            .chain(self.plan.objects.iter().map(|item| item.id.as_str()))
            .chain(self.plan.images.iter().map(|item| item.id.as_str()))
            .chain(self.plan.texts.iter().map(|item| item.id.as_str()))
            .chain(self.plan.areas.iter().map(|item| item.id.as_str()))
            .chain(self.plan.rooms.iter().map(|item| item.id.as_str()))
            .chain(self.plan.compasses.iter().map(|item| item.id.as_str()))
            .chain(self.plan.arrows.iter().map(|item| item.id.as_str()))
            .map(str::to_owned)
            .collect::<std::collections::HashSet<_>>();
        self.plan
            .layer_assignments
            .retain(|assignment| existing_ids.contains(&assignment.item_id));
        if removed_count > 1 {
            self.status = self.localizer.format(
                "status.items_deleted",
                &[("count", removed_count.to_string())],
            );
        }
        self.selection = None;
        self.commit(before);
    }

    fn duplicate_selected(&mut self) {
        if !self.additional_selections.is_empty() {
            let before = self.snapshot();
            self.batching_edits = true;
            let mut selections = self.additional_selections.clone();
            if let Some(primary) = self.selection.clone() {
                selections.insert(0, primary);
            }
            self.additional_selections.clear();
            let mut created = Vec::new();
            for selection in selections {
                self.selection = Some(selection.clone());
                self.duplicate_selected();
                if let Some(new_selection) = self.selection.clone()
                    && new_selection != selection
                {
                    created.push(new_selection);
                }
            }
            self.batching_edits = false;
            self.selection = created.first().cloned();
            self.additional_selections = created.into_iter().skip(1).collect();
            self.commit(before);
            self.status = self.localizer.text("status.items_duplicated");
            return;
        }
        let Some(selection) = self.selection.clone() else {
            return;
        };
        let before = self.snapshot();
        match selection {
            Selection::Wall(id) => {
                let Some(mut wall) = self.plan.walls.iter().find(|wall| wall.id == id).cloned()
                else {
                    return;
                };
                wall.id = make_id("wall");
                wall.x1 += GRID;
                wall.y1 += GRID;
                wall.x2 += GRID;
                wall.y2 += GRID;
                let new_id = wall.id.clone();
                self.plan.walls.push(wall);
                self.assign_to_active_layer(&new_id);
                self.refresh_automatic_rooms();
                self.selection = Some(Selection::Wall(new_id));
                self.status = self.localizer.text("status.wall_duplicated");
            }
            Selection::Line(id) => {
                let Some(mut line) = self.plan.lines.iter().find(|line| line.id == id).cloned()
                else {
                    return;
                };
                line.id = make_id("line");
                line.x1 += self.grid_size;
                line.y1 += self.grid_size;
                line.x2 += self.grid_size;
                line.y2 += self.grid_size;
                let new_id = line.id.clone();
                self.plan.lines.push(line);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Line(new_id));
                self.status = self.localizer.text("status.line_duplicated");
            }
            Selection::Text(id) => {
                let Some(mut text) = self.plan.texts.iter().find(|text| text.id == id).cloned()
                else {
                    return;
                };
                text.id = make_id("text");
                text.x += self.grid_size;
                text.y += self.grid_size;
                let new_id = text.id.clone();
                self.plan.texts.push(text);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Text(new_id));
                self.status = self.localizer.text("status.text_duplicated");
            }
            Selection::Door(id) => {
                let Some(mut door) = self.plan.doors.iter().find(|door| door.id == id).cloned()
                else {
                    return;
                };
                door.id = make_id("door");
                let Some(position) =
                    self.find_opening_position(&door.wall_id, door.width, door.t + 0.15)
                else {
                    self.status = self.localizer.text("status.no_opening_space");
                    return;
                };
                door.t = position;
                let new_id = door.id.clone();
                self.plan.doors.push(door);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Door(new_id));
                self.status = self.localizer.text("status.door_duplicated");
            }
            Selection::Image(id) => {
                let Some(mut image) = self
                    .plan
                    .images
                    .iter()
                    .find(|image| image.id == id)
                    .cloned()
                else {
                    return;
                };
                image.id = make_id("image");
                image.x += GRID;
                image.y += GRID;
                let new_id = image.id.clone();
                self.plan.images.push(image);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Image(new_id));
                self.status = self.localizer.text("status.image_duplicated");
            }
            Selection::Window(id) => {
                let Some(mut window) = self
                    .plan
                    .windows
                    .iter()
                    .find(|window| window.id == id)
                    .cloned()
                else {
                    return;
                };
                window.id = make_id("window");
                let Some(position) =
                    self.find_opening_position(&window.wall_id, window.width, window.t + 0.15)
                else {
                    self.status = self.localizer.text("status.no_opening_space");
                    return;
                };
                window.t = position;
                let new_id = window.id.clone();
                self.plan.windows.push(window);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Window(new_id));
                self.status = self.localizer.text("status.window_duplicated");
            }
            Selection::Object(id) => {
                let Some(mut object) = self
                    .plan
                    .objects
                    .iter()
                    .find(|object| object.id == id)
                    .cloned()
                else {
                    return;
                };
                object.id = make_id("object");
                object.x += GRID;
                object.y += GRID;
                let new_id = object.id.clone();
                self.plan.objects.push(object);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Object(new_id));
                self.status = self.localizer.text("status.object_duplicated");
            }
            Selection::Area(id) => {
                let Some(mut area) = self.plan.areas.iter().find(|area| area.id == id).cloned()
                else {
                    return;
                };
                area.id = make_id("area");
                for point in &mut area.points {
                    point.x += self.grid_size;
                    point.y += self.grid_size;
                }
                let new_id = area.id.clone();
                self.plan.areas.push(area);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Area(new_id));
                self.status = self.localizer.text("status.area_duplicated");
            }
            Selection::Room(id) => {
                let Some(mut room) = self.plan.rooms.iter().find(|room| room.id == id).cloned()
                else {
                    return;
                };
                room.id = make_id("room");
                room.source = RoomSource::Manual;
                for point in &mut room.points {
                    point.x += self.grid_size;
                    point.y += self.grid_size;
                }
                let new_id = room.id.clone();
                self.plan.rooms.push(room);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Room(new_id));
                self.status = self.localizer.text("status.room_duplicated");
            }
            Selection::Compass(id) => {
                let Some(mut compass) = self
                    .plan
                    .compasses
                    .iter()
                    .find(|compass| compass.id == id)
                    .cloned()
                else {
                    return;
                };
                compass.id = make_id("compass");
                compass.x += self.grid_size;
                compass.y += self.grid_size;
                let new_id = compass.id.clone();
                self.plan.compasses.push(compass);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Compass(new_id));
                self.status = self.localizer.text("status.compass_duplicated");
            }
            Selection::Arrow(id) => {
                let Some(mut arrow) = self
                    .plan
                    .arrows
                    .iter()
                    .find(|arrow| arrow.id == id)
                    .cloned()
                else {
                    return;
                };
                arrow.id = make_id("arrow");
                arrow.x += self.grid_size;
                arrow.y += self.grid_size;
                let new_id = arrow.id.clone();
                self.plan.arrows.push(arrow);
                self.assign_to_active_layer(&new_id);
                self.selection = Some(Selection::Arrow(new_id));
                self.status = self.localizer.text("status.arrow_duplicated");
            }
        }
        self.commit(before);
    }

    fn split_selected_wall(&mut self) {
        let Some(Selection::Wall(id)) = self.selection.clone() else {
            return;
        };
        let Some(index) = self.plan.walls.iter().position(|wall| wall.id == id) else {
            return;
        };
        if !self.selection_editable(&Selection::Wall(id.clone())) {
            return;
        }
        let original = self.plan.walls[index].clone();
        if original.length() / 2.0 < MIN_WALL {
            self.status = self.localizer.text("status.wall_too_short");
            return;
        }
        let opening_crosses_cut = self
            .plan
            .doors
            .iter()
            .filter(|door| door.wall_id == original.id)
            .map(|door| (door.t, door.width))
            .chain(
                self.plan
                    .windows
                    .iter()
                    .filter(|window| window.wall_id == original.id)
                    .map(|window| (window.t, window.width)),
            )
            .any(|(position, width)| {
                let half_span = width / (2.0 * original.length());
                position - half_span < 0.5 && position + half_span > 0.5
            });
        if opening_crosses_cut {
            self.status = self.localizer.text("status.split_blocked");
            return;
        }
        let before = self.snapshot();
        let original_layer_id = self.plan.layer_id_for(&original.id).to_owned();
        let middle = original.point_at(0.5);
        let mut second = original.clone();
        second.id = make_id("wall");
        second.x1 = middle.x;
        second.y1 = middle.y;
        self.plan.walls[index].x2 = middle.x;
        self.plan.walls[index].y2 = middle.y;

        for door in &mut self.plan.doors {
            if door.wall_id != original.id {
                continue;
            }
            if door.t < 0.5 {
                door.t *= 2.0;
            } else {
                door.wall_id = second.id.clone();
                door.t = (door.t - 0.5) * 2.0;
            }
        }
        for window in &mut self.plan.windows {
            if window.wall_id != original.id {
                continue;
            }
            if window.t < 0.5 {
                window.t *= 2.0;
            } else {
                window.wall_id = second.id.clone();
                window.t = (window.t - 0.5) * 2.0;
            }
        }

        let second_id = second.id.clone();
        self.plan.walls.push(second);
        self.plan.assign_to_layer(&second_id, &original_layer_id);
        self.selection = Some(Selection::Wall(second_id));
        self.status = self.localizer.text("status.wall_split");
        self.commit(before);
    }

    fn select_all(&mut self) {
        let selections = selectable_selections(&self.plan);

        self.selection = selections.first().cloned();
        self.additional_selections = selections.into_iter().skip(1).collect();
        self.tool = Tool::Select;
        self.status = self.localizer.format(
            "status.items_selected",
            &[(
                "count",
                (self.additional_selections.len() + usize::from(self.selection.is_some()))
                    .to_string(),
            )],
        );
    }

    fn keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let previous_tool = self.tool;
        let editing_text = ctx.egui_wants_keyboard_input();
        if self.settings_open
            || self.layers_open
            || ctx.any_popup_open()
            || self.pending_action.is_some()
        {
            return;
        }
        let actions = ctx.input_mut(|input| input::KeyboardActions::read(input, editing_text));
        if actions.new {
            self.request_new_plan();
        }
        if actions.open {
            self.request_open_plan();
        }
        if actions.save_as {
            self.save_plan_as();
        }
        if actions.save {
            self.save_plan();
        }
        if actions.undo {
            self.undo();
        }
        if actions.redo {
            self.redo();
        }
        if actions.select_all {
            self.select_all();
        }
        if actions.duplicate {
            self.duplicate_selected();
        }
        if actions.delete {
            self.remove_selected();
        }
        if let Some(key) = actions.tool_key {
            self.tool = match key {
                Key::V => Tool::Select,
                Key::M => Tool::Wall,
                Key::L => Tool::Line,
                Key::R => Tool::Measure,
                Key::T => Tool::Text,
                Key::P => Tool::Door,
                Key::F => Tool::Window,
                Key::O => Tool::Object,
                Key::A => Tool::Area,
                Key::B => Tool::Room,
                Key::N => Tool::Compass,
                Key::Y => Tool::Arrow,
                _ => self.tool,
            };
        }
        if actions.nudge != Vec2::ZERO {
            self.nudge_selected(Point::new(actions.nudge.x, actions.nudge.y));
        }
        if self.tool != previous_tool && !actions.select_all {
            self.additional_selections.clear();
        }
        if self.tool != previous_tool || actions.escape {
            self.cancel_interaction();
        }
        if actions.escape {
            self.selection = None;
            self.additional_selections.clear();
        }
    }

    fn top_bar(&mut self, root: &mut Ui) {
        Panel::top("top_bar")
            .min_size(48.0)
            .frame(
                egui::Frame::new()
                    .fill(self.panel_fill())
                    .inner_margin(Margin::symmetric(14, 8)),
            )
            .show(root, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("RPP").strong().size(19.0).color(ACCENT));
                    ui.label(
                        egui::RichText::new("Ran Plan Plan")
                            .size(12.0)
                            .color(self.muted_text_color()),
                    );
                    ui.label(
                        egui::RichText::new(env!("CARGO_PKG_VERSION"))
                            .size(10.0)
                            .color(self.subtle_text_color()),
                    );
                    ui.add_space(24.0);

                    if flat_button(ui, &self.localizer.text("action.new")).clicked() {
                        self.request_new_plan();
                    }
                    if flat_button(ui, &self.localizer.text("action.open")).clicked() {
                        self.request_open_plan();
                    }
                    let save_label = if self.dirty {
                        self.localizer.text("action.save_dirty")
                    } else {
                        self.localizer.text("action.save")
                    };
                    if flat_button(ui, &save_label).clicked() {
                        self.save_plan();
                    }
                    ui.menu_button(self.localizer.text("action.save_as"), |ui| {
                        if ui
                            .button(self.localizer.text("action.save_as_plan"))
                            .clicked()
                        {
                            self.save_plan_as();
                            ui.close();
                        }
                        ui.separator();
                        ui.checkbox(
                            &mut self.export_distances,
                            self.localizer.text("export.include_distances"),
                        );
                        if ui
                            .button(self.localizer.text("action.export_svg"))
                            .clicked()
                        {
                            self.export_svg();
                            ui.close();
                        }
                        if ui
                            .button(self.localizer.text("action.export_png"))
                            .clicked()
                        {
                            self.export_png();
                            ui.close();
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(self.localizer.text("export.pdf_scale"));
                            ui.add(
                                egui::DragValue::new(&mut self.pdf_scale)
                                    .range(1..=1000)
                                    .prefix("1:"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(self.localizer.text("export.paper"));
                            egui::ComboBox::from_id_salt("pdf-paper")
                                .selected_text(self.pdf_paper.label())
                                .show_ui(ui, |ui| {
                                    for paper in [
                                        export::PaperFormat::A4,
                                        export::PaperFormat::Letter,
                                        export::PaperFormat::Legal,
                                        export::PaperFormat::Tabloid,
                                    ] {
                                        ui.selectable_value(
                                            &mut self.pdf_paper,
                                            paper,
                                            paper.label(),
                                        );
                                    }
                                });
                        });
                        if ui
                            .button(self.localizer.text("action.export_pdf"))
                            .clicked()
                        {
                            self.export_pdf();
                            ui.close();
                        }
                    });
                    if flat_button(ui, &self.localizer.text("action.import_image")).clicked() {
                        self.import_image();
                    }
                    if flat_button(ui, &self.localizer.text("action.settings")).clicked() {
                        self.settings_open = true;
                    }
                    if flat_button(ui, &self.localizer.text("action.layers")).clicked() {
                        self.layers_open = true;
                    }

                    ui.horizontal(|ui| {
                        let languages = self.localizer.languages();
                        let mut selected_language = self.localizer.current_language().to_owned();
                        egui::ComboBox::from_id_salt("language")
                            .selected_text(
                                languages
                                    .iter()
                                    .find(|(code, _)| code == &selected_language)
                                    .map(|(_, name)| name.as_str())
                                    .unwrap_or("English (US)"),
                            )
                            .show_ui(ui, |ui| {
                                for (code, name) in &languages {
                                    ui.selectable_value(&mut selected_language, code.clone(), name);
                                }
                            });
                        if selected_language != self.localizer.current_language() {
                            self.localizer.set_language(&selected_language);
                            save_language(&selected_language);
                            self.status = self.localizer.text("app.ready");
                        }
                        ui.label(self.localizer.text("language.label"));
                        let redo_image = egui::Image::from_texture(&self.redo_icon)
                            .fit_to_exact_size(Vec2::splat(28.0));
                        if ui
                            .add_enabled(
                                !self.future.is_empty(),
                                egui::Button::new(redo_image).frame(false),
                            )
                            .on_hover_text(self.localizer.text("hint.redo"))
                            .clicked()
                        {
                            self.redo();
                        }
                        let undo_image = egui::Image::from_texture(&self.undo_icon)
                            .fit_to_exact_size(Vec2::splat(28.0));
                        if ui
                            .add_enabled(
                                !self.past.is_empty(),
                                egui::Button::new(undo_image).frame(false),
                            )
                            .on_hover_text(self.localizer.text("hint.undo"))
                            .clicked()
                        {
                            self.undo();
                        }
                    });
                });
            });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = self.settings_open;
        let mut changed = false;
        egui::Window::new(self.localizer.text("settings.title"))
            .open(&mut open)
            .resizable(false)
            .vscroll(true)
            .default_width(390.0)
            .show(ctx, |ui| {
                ui.heading(self.localizer.text("settings.general"));
                ui.separator();

                let languages = self.localizer.languages();
                let mut language = self.localizer.current_language().to_owned();
                property_row(ui, &self.localizer.text("property.language"), |ui| {
                    egui::ComboBox::from_id_salt("settings-language")
                        .selected_text(
                            languages
                                .iter()
                                .find(|(code, _)| code == &language)
                                .map(|(_, name)| name.as_str())
                                .unwrap_or("English (US)"),
                        )
                        .show_ui(ui, |ui| {
                            for (code, name) in &languages {
                                ui.selectable_value(&mut language, code.clone(), name);
                            }
                        })
                        .response
                });
                if language != self.localizer.current_language() {
                    self.localizer.set_language(&language);
                    save_language(&language);
                }

                let original_theme = self.theme;
                property_row(ui, &self.localizer.text("property.theme"), |ui| {
                    egui::ComboBox::from_id_salt("settings-theme")
                        .selected_text(match self.theme {
                            AppTheme::Light => self.localizer.text("theme.light"),
                            AppTheme::Dark => self.localizer.text("theme.dark"),
                            AppTheme::WarmDark => self.localizer.text("theme.warm_dark"),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.theme,
                                AppTheme::Light,
                                self.localizer.text("theme.light"),
                            );
                            ui.selectable_value(
                                &mut self.theme,
                                AppTheme::Dark,
                                self.localizer.text("theme.dark"),
                            );
                            ui.selectable_value(
                                &mut self.theme,
                                AppTheme::WarmDark,
                                self.localizer.text("theme.warm_dark"),
                            );
                        })
                        .response
                });
                if self.theme != original_theme {
                    configure_style(ctx, self.theme);
                    changed = true;
                }

                let original_unit = self.unit;
                property_row(ui, &self.localizer.text("property.unit"), |ui| {
                    egui::ComboBox::from_id_salt("settings-unit")
                        .selected_text(self.unit.as_str())
                        .show_ui(ui, |ui| {
                            for unit in [Unit::Mm, Unit::Cm, Unit::M, Unit::In, Unit::Ft] {
                                ui.selectable_value(&mut self.unit, unit, unit.as_str());
                            }
                        })
                        .response
                });
                changed |= self.unit != original_unit;
                changed |= ui
                    .checkbox(&mut self.grid_visible, self.localizer.text("property.grid"))
                    .changed();
                changed |= ui
                    .checkbox(&mut self.snap_enabled, self.localizer.text("property.snap"))
                    .changed();
                let mut spacing = self.unit.display_from_centimeters(self.grid_size);
                if property_row(ui, &self.localizer.text("property.spacing"), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut spacing)
                            .range(0.01..=50_000.0)
                            .suffix(self.unit.suffix()),
                    )
                })
                .changed()
                {
                    self.grid_size = self.unit.to_centimeters(spacing).clamp(0.1, 5000.0);
                    changed = true;
                }

                let original_scroll_mode = self.scroll_mode;
                property_row(ui, &self.localizer.text("navigation.scroll"), |ui| {
                    egui::ComboBox::from_id_salt("scroll-mode")
                        .selected_text(self.localizer.text(self.scroll_mode.translation_key()))
                        .show_ui(ui, |ui| {
                            for mode in [
                                input::ScrollMode::Auto,
                                input::ScrollMode::Pan,
                                input::ScrollMode::Zoom,
                            ] {
                                ui.selectable_value(
                                    &mut self.scroll_mode,
                                    mode,
                                    self.localizer.text(mode.translation_key()),
                                );
                            }
                        })
                        .response
                });
                changed |= self.scroll_mode != original_scroll_mode;

                ui.add_space(12.0);
                ui.heading(self.localizer.text("settings.images"));
                ui.separator();
                let mut index = self.image_limit_index as i32;
                let label = if index as usize >= IMAGE_LIMITS_KIB.len() {
                    self.localizer.text("image_limit.source")
                } else {
                    format!("{} KiB", IMAGE_LIMITS_KIB[index as usize])
                };
                ui.label(
                    self.localizer
                        .format("image_limit.current", &[("limit", label)]),
                );
                if ui
                    .add(egui::Slider::new(&mut index, 0..=10).show_value(false))
                    .changed()
                {
                    self.image_limit_index = index.clamp(0, 10) as u8;
                    changed = true;
                }
                ui.small(self.localizer.text("image_limit.help"));
            });
        self.settings_open = open;
        if changed {
            self.persist_settings();
        }
    }

    fn tool_panel(&mut self, root: &mut Ui) {
        Panel::left("tools")
            .exact_size(112.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(self.secondary_panel_fill())
                    .inner_margin(Margin::same(10)),
            )
            .show(root, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("tools-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.tool_panel_contents(ui));
            });
    }

    fn tool_panel_contents(&mut self, ui: &mut Ui) {
        let previous_tool = self.tool;
        tool_button(
            ui,
            &mut self.tool,
            Tool::Select,
            &self.localizer.text("tool.select"),
            "V",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Wall,
            &self.localizer.text("tool.wall"),
            "M",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Line,
            &self.localizer.text("tool.line"),
            "L",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Measure,
            &self.localizer.text("tool.measure"),
            "R",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Text,
            &self.localizer.text("tool.text"),
            "T",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Door,
            &self.localizer.text("tool.door"),
            "P",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Window,
            &self.localizer.text("tool.window"),
            "F",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Object,
            &self.localizer.text("tool.object"),
            "O",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Area,
            &self.localizer.text("tool.area"),
            "A",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Room,
            &self.localizer.text("tool.room"),
            "B",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Compass,
            &self.localizer.text("tool.compass"),
            "N",
        );
        tool_button(
            ui,
            &mut self.tool,
            Tool::Arrow,
            &self.localizer.text("tool.arrow"),
            "Y",
        );
        if self.tool != previous_tool {
            self.cancel_interaction();
            self.additional_selections.clear();
        }
    }

    fn status_bar(&mut self, root: &mut Ui) {
        Panel::bottom("status")
            .exact_size(28.0)
            .frame(
                egui::Frame::new()
                    .fill(self.secondary_panel_fill())
                    .inner_margin(Margin::symmetric(10, 6)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    let status = self.recovery_error.as_deref().unwrap_or(&self.status);
                    ui.allocate_ui(
                        Vec2::new(ui.available_width() * 0.42, ui.available_height()),
                        |ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(status).size(11.0).color(
                                    if self.recovery_error.is_some() {
                                        Color32::from_rgb(180, 45, 45)
                                    } else {
                                        self.muted_text_color()
                                    },
                                ))
                                .truncate(),
                            )
                            .on_hover_text(format!(
                                "{}\n{}",
                                status,
                                self.localizer.text(self.tool.instruction_key())
                            ));
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let counts = self.localizer.format(
                            "status.counts",
                            &[
                                ("walls", self.plan.walls.len().to_string()),
                                (
                                    "wall_suffix",
                                    if self.plan.walls.len() == 1 { "" } else { "s" }.to_owned(),
                                ),
                                ("rooms", self.plan.rooms.len().to_string()),
                                (
                                    "room_suffix",
                                    if self.plan.rooms.len() == 1 { "" } else { "s" }.to_owned(),
                                ),
                                ("lines", self.plan.lines.len().to_string()),
                                (
                                    "line_suffix",
                                    if self.plan.lines.len() == 1 { "" } else { "s" }.to_owned(),
                                ),
                                ("texts", self.plan.texts.len().to_string()),
                                (
                                    "text_suffix",
                                    if self.plan.texts.len() == 1 { "" } else { "s" }.to_owned(),
                                ),
                                ("doors", self.plan.doors.len().to_string()),
                                (
                                    "door_suffix",
                                    if self.plan.doors.len() == 1 { "" } else { "s" }.to_owned(),
                                ),
                                ("images", self.plan.images.len().to_string()),
                                (
                                    "image_suffix",
                                    if self.plan.images.len() == 1 { "" } else { "s" }.to_owned(),
                                ),
                                ("windows", self.plan.windows.len().to_string()),
                                (
                                    "window_suffix",
                                    if self.plan.windows.len() == 1 {
                                        ""
                                    } else {
                                        "s"
                                    }
                                    .to_owned(),
                                ),
                                ("objects", self.plan.objects.len().to_string()),
                                (
                                    "object_suffix",
                                    if self.plan.objects.len() == 1 {
                                        ""
                                    } else {
                                        "s"
                                    }
                                    .to_owned(),
                                ),
                                ("areas", self.plan.areas.len().to_string()),
                                (
                                    "area_suffix",
                                    if self.plan.areas.len() == 1 { "" } else { "s" }.to_owned(),
                                ),
                                ("compasses", self.plan.compasses.len().to_string()),
                            ],
                        );
                        ui.label(
                            egui::RichText::new(counts)
                                .size(11.0)
                                .color(self.muted_text_color()),
                        );
                        if let Some(cursor) = self.cursor_world {
                            ui.label(
                                egui::RichText::new(self.localizer.format(
                                    "status.cursor",
                                    &[
                                        (
                                            "x",
                                            format!(
                                                "{:.2}",
                                                self.unit.display_from_centimeters(cursor.x)
                                            ),
                                        ),
                                        (
                                            "y",
                                            format!(
                                                "{:.2}",
                                                self.unit.display_from_centimeters(cursor.y)
                                            ),
                                        ),
                                        ("unit", self.unit.as_str().to_owned()),
                                    ],
                                ))
                                .size(11.0)
                                .color(self.muted_text_color()),
                            );
                        }
                        ui.label(
                            egui::RichText::new(format!("{:.0} %", self.zoom * 100.0))
                                .size(11.0)
                                .color(self.muted_text_color()),
                        );
                    });
                });
            });
    }

    fn canvas(&mut self, root: &mut Ui, input_blocked: bool) {
        let ctx = root.ctx().clone();
        CentralPanel::default()
            .frame(egui::Frame::new().fill(self.canvas_fill()))
            .show(root, |ui| {
                let rect = ui.available_rect_before_wrap();
                let response = ui.allocate_rect(rect, Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                if !ctx.egui_wants_keyboard_input() && !input_blocked {
                    let actions = ctx.input_mut(input::view_actions);
                    if actions.fit {
                        self.fit_plan(rect);
                    } else if actions.zoom != 1.0 {
                        let anchor = rect.center();
                        let before = self.screen_to_world(rect, anchor);
                        self.zoom = (self.zoom * actions.zoom).clamp(MIN_ZOOM, MAX_ZOOM);
                        self.pan = anchor - rect.min - Vec2::new(before.x, before.y) * self.zoom;
                    }
                }
                let zoom_rect = Rect::from_min_size(
                    Pos2::new(rect.right() - 188.0, rect.bottom() - 44.0),
                    Vec2::new(176.0, 32.0),
                );
                let over_zoom_controls = ctx.input(|input| {
                    input
                        .pointer
                        .hover_pos()
                        .is_some_and(|position| zoom_rect.contains(position))
                });
                if !input_blocked && (!over_zoom_controls || self.has_active_drag()) {
                    self.handle_canvas_input(&ctx, &response, rect);
                }
                self.paint_canvas(&painter, rect);
                ui.scope_builder(egui::UiBuilder::new().max_rect(zoom_rect), |ui| {
                    if input_blocked {
                        ui.disable();
                    }
                    egui::Frame::new()
                        .fill(self.panel_fill())
                        .stroke(Stroke::new(
                            1.0,
                            if self.theme == AppTheme::Light {
                                Color32::from_gray(205)
                            } else {
                                Color32::from_gray(75)
                            },
                        ))
                        .show(ui, |ui| {
                            ui.horizontal_centered(|ui| {
                                if ui.small_button("−").clicked() {
                                    self.zoom = (self.zoom - 0.1).max(MIN_ZOOM);
                                }
                                ui.label(format!("{:.0} %", self.zoom * 100.0));
                                if ui.small_button("+").clicked() {
                                    self.zoom = (self.zoom + 0.1).min(MAX_ZOOM);
                                }
                                if ui
                                    .small_button(self.localizer.text("action.fit_plan"))
                                    .on_hover_text(self.localizer.text("hint.fit_plan"))
                                    .clicked()
                                {
                                    self.fit_plan(rect);
                                }
                            });
                        });
                });
            });
    }

    fn handle_canvas_input(&mut self, ctx: &egui::Context, response: &Response, rect: Rect) {
        self.cursor_world = response
            .hover_pos()
            .map(|position| self.screen_to_world(rect, position));
        if response.hovered() {
            let navigation = ctx.input(|input| input::canvas_navigation(input, self.scroll_mode));
            if let Some(pointer) = response.hover_pos() {
                let before = self.screen_to_world(rect, pointer);
                self.zoom = (self.zoom * navigation.zoom).clamp(MIN_ZOOM, MAX_ZOOM);
                self.pan =
                    pointer - rect.min - Vec2::new(before.x, before.y) * self.zoom + navigation.pan;
            }
        }

        let navigation_allowed = !self.has_active_drag();
        let space_down = navigation_allowed
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|input| input.key_down(Key::Space));
        if space_down
            && response.is_pointer_button_down_on()
            && ctx.input(|input| input.pointer.primary_down())
        {
            self.panning_canvas = true;
        }
        let panning = self.panning_canvas;
        if panning {
            self.pan += response.drag_delta();
            if !ctx.input(|input| input.pointer.primary_down()) {
                self.panning_canvas = false;
            }
        }
        if navigation_allowed && !panning && response.dragged_by(PointerButton::Middle) {
            self.pan += response.drag_delta();
        }
        if space_down
            || panning
            || navigation_allowed
                && ctx.input(|input| input.pointer.button_down(PointerButton::Middle))
        {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            return;
        }
        self.cursor_world = response
            .hover_pos()
            .map(|position| self.screen_to_world(rect, position));

        if self.tool == Tool::Wall {
            self.handle_wall_events(ctx, rect);
            return;
        }
        if self.tool == Tool::Line {
            self.handle_line_events(ctx, rect);
            return;
        }
        if self.tool == Tool::Arrow {
            self.handle_arrow_events(ctx, rect);
            return;
        }
        if self.tool == Tool::Measure {
            self.handle_measure_events(ctx, rect);
            return;
        }
        if self.tool == Tool::Area {
            self.handle_area_events(ctx, response, rect);
            return;
        }
        if self.tool == Tool::Room {
            self.handle_room_events(ctx, response, rect);
            return;
        }

        let pointer_pos = response
            .interact_pointer_pos()
            .or_else(|| ctx.input(|input| input.pointer.latest_pos()));
        let Some(pointer_pos) = pointer_pos else {
            return;
        };
        let world = self.screen_to_world(rect, pointer_pos);

        if self.tool == Tool::Compass && response.clicked_by(PointerButton::Primary) {
            let before = self.snapshot();
            let point = self.snap_point(world);
            let compass = CompassRose {
                id: make_id("compass"),
                x: point.x,
                y: point.y,
                size: 80.0,
                rotation: 0.0,
                color: default_compass_color(),
                locked: false,
            };
            let id = compass.id.clone();
            self.plan.compasses.push(compass);
            self.assign_to_active_layer(&id);
            self.selection = Some(Selection::Compass(id));
            self.status = self.localizer.text("status.compass_added");
            self.commit(before);
            self.tool = Tool::Select;
            return;
        }

        if self.tool == Tool::Text && response.clicked_by(PointerButton::Primary) {
            let before = self.snapshot();
            let point = self.snap_point(world);
            let text = TextObject {
                id: make_id("text"),
                text: self.localizer.text("text.default"),
                x: point.x,
                y: point.y,
                size: self.default_text_size,
                rotation: 0.0,
                color: default_text_color(),
                bold: false,
                locked: false,
            };
            let id = text.id.clone();
            self.plan.texts.push(text);
            self.assign_to_active_layer(&id);
            self.selection = Some(Selection::Text(id));
            self.status = self.localizer.text("status.text_added");
            self.commit(before);
            self.tool = Tool::Select;
            return;
        }

        if self.tool == Tool::Door && response.clicked_by(PointerButton::Primary) {
            if let Some((index, t, distance)) = self.nearest_wall(world)
                && distance <= 18.0 / self.zoom
            {
                let wall_id = self.plan.walls[index].id.clone();
                let length = self.plan.walls[index].length();
                if length >= 45.0 {
                    let width = self.default_door_width.min((length - 10.0).max(40.0));
                    let half = width / 2.0 / length;
                    let position = t.clamp(half, 1.0 - half);
                    if !self.opening_available(&wall_id, position, width, None) {
                        self.status = self.localizer.text("status.opening_overlap");
                        return;
                    }
                    let before = self.snapshot();
                    let door = Door {
                        id: make_id("door"),
                        wall_id,
                        t: position,
                        width,
                        kind: DoorKind::Single,
                        hinge: HingeSide::Left,
                        swing: SwingSide::Inward,
                        color: default_opening_color(),
                        locked: false,
                    };
                    let id = door.id.clone();
                    self.plan.doors.push(door);
                    self.assign_to_active_layer(&id);
                    self.selection = Some(Selection::Door(id));
                    self.status = self.localizer.text("status.door_added");
                    self.commit(before);
                }
            }
            return;
        }

        if self.tool == Tool::Window && response.clicked_by(PointerButton::Primary) {
            if let Some((index, t, distance)) = self.nearest_wall(world)
                && distance <= 18.0 / self.zoom
            {
                let wall_id = self.plan.walls[index].id.clone();
                let length = self.plan.walls[index].length();
                if length >= 60.0 {
                    let width = self.default_window_width.min((length - 10.0).max(50.0));
                    let half = width / (2.0 * length);
                    let position = t.clamp(half, 1.0 - half);
                    if !self.opening_available(&wall_id, position, width, None) {
                        self.status = self.localizer.text("status.opening_overlap");
                        return;
                    }
                    let before = self.snapshot();
                    let window = Window {
                        id: make_id("window"),
                        wall_id,
                        t: position,
                        width,
                        kind: WindowKind::Standard,
                        color: default_window_color(),
                        locked: false,
                    };
                    let id = window.id.clone();
                    self.plan.windows.push(window);
                    self.assign_to_active_layer(&id);
                    self.selection = Some(Selection::Window(id));
                    self.status = self.localizer.text("status.window_added");
                    self.commit(before);
                }
            }
            return;
        }

        if self.tool == Tool::Object && response.clicked_by(PointerButton::Primary) {
            let before = self.snapshot();
            let point = self.snap_point(world);
            let object = PlanObject {
                id: make_id("object"),
                kind: ObjectKind::Rectangle,
                name: String::new(),
                x: point.x,
                y: point.y,
                width: 100.0,
                height: 60.0,
                rotation: 0.0,
                fill_color: default_object_fill_color(),
                stroke_color: default_object_stroke_color(),
                locked: false,
            };
            let id = object.id.clone();
            self.plan.objects.push(object);
            self.assign_to_active_layer(&id);
            self.selection = Some(Selection::Object(id));
            self.status = self.localizer.text("status.object_added");
            self.commit(before);
            self.tool = Tool::Select;
            return;
        }

        if self.tool == Tool::Select {
            let additive_selection = ctx.input(|input| input.modifiers.shift);
            if response.drag_started_by(PointerButton::Primary) {
                if !additive_selection {
                    let selected_started =
                        self.begin_group_drag(world) || self.begin_selected_drag(world);
                    let item_started = selected_started
                        || self.begin_compass_drag(world)
                        || self.begin_text_drag(world)
                        || self.begin_window_drag(world)
                        || self.begin_door_drag(world)
                        || self.begin_wall_drag(world)
                        || self.begin_line_drag(world)
                        || self.begin_object_drag(world)
                        || self.begin_area_drag(world)
                        || self.begin_room_drag(world)
                        || self.begin_image_drag(world);
                    if !item_started {
                        self.selection = None;
                        self.additional_selections.clear();
                        self.selection_box = Some(DraftWall {
                            start: world,
                            end: world,
                        });
                    }
                } else if self.hit_selections(world).is_empty() {
                    self.selection_box = Some(DraftWall {
                        start: world,
                        end: world,
                    });
                }
            }
            if response.dragged_by(PointerButton::Primary) {
                if self.group_drag.is_some() {
                    self.update_group_drag(world);
                } else if let Some(selection_box) = &mut self.selection_box {
                    selection_box.end = world;
                } else if self.door_drag.is_some() {
                    self.update_door_drag(world);
                } else if self.window_drag.is_some() {
                    self.update_window_drag(world);
                } else if self.text_drag.is_some() {
                    self.update_text_drag(world);
                } else if self.compass_drag.is_some() {
                    self.update_compass_drag(world);
                } else if self.area_drag.is_some() {
                    self.update_area_drag(world);
                } else if self.room_drag.is_some() {
                    self.update_room_drag(world);
                } else if self.object_drag.is_some() {
                    self.update_object_drag(world);
                } else if self.arrow_drag.is_some() {
                    self.update_arrow_drag(world);
                } else if self.line_drag.is_some() {
                    self.update_line_drag(world);
                } else if self.image_drag.is_some() {
                    self.update_image_drag(world);
                } else {
                    self.update_wall_drag(world);
                }
            }
            if response.drag_stopped_by(PointerButton::Primary) {
                if let Some(drag) = self.group_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.door_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.window_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.text_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.compass_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.area_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.room_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.object_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.arrow_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.line_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.image_drag.take() {
                    self.commit(drag.before);
                } else if let Some(drag) = self.wall_drag.take() {
                    self.refresh_automatic_rooms();
                    self.commit(drag.before);
                } else if let Some(selection_box) = self.selection_box.take() {
                    self.select_box_contents(&selection_box, additive_selection);
                }
            }
            if response.clicked_by(PointerButton::Primary) {
                self.selection_box = None;
                self.select_at(world, additive_selection);
            }
        }
    }

    fn handle_line_events(&mut self, ctx: &egui::Context, rect: Rect) {
        let (pressed, down, released, pointer, shift) = ctx.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.pointer.button_released(PointerButton::Primary),
                input.pointer.interact_pos(),
                input.modifiers.shift,
            )
        });
        if pressed && let Some(position) = pointer.filter(|position| rect.contains(*position)) {
            let point = self.snap_point(self.screen_to_world(rect, position));
            self.line_draft = Some(DraftWall {
                start: point,
                end: point,
            });
        }
        if down
            && let Some(position) = pointer
            && let Some(start) = self.line_draft.as_ref().map(|draft| draft.start)
        {
            let end = self.snap_with_axis(self.screen_to_world(rect, position), start, shift);
            if let Some(draft) = &mut self.line_draft {
                draft.end = end;
            }
        }
        if released
            && let Some(draft) = self.line_draft.take()
            && draft.start.distance(draft.end) >= 1.0
        {
            let before = self.snapshot();
            let line = PlanLine {
                id: make_id("line"),
                x1: draft.start.x,
                y1: draft.start.y,
                x2: draft.end.x,
                y2: draft.end.y,
                color: default_line_color(),
                locked: false,
            };
            let id = line.id.clone();
            self.plan.lines.push(line);
            self.assign_to_active_layer(&id);
            self.selection = Some(Selection::Line(id));
            self.status = self.localizer.text("status.line_added");
            self.commit(before);
        }
    }

    fn handle_measure_events(&mut self, ctx: &egui::Context, rect: Rect) {
        let (pressed, down, released, pointer, shift) = ctx.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.pointer.button_released(PointerButton::Primary),
                input.pointer.interact_pos(),
                input.modifiers.shift,
            )
        });
        if pressed && let Some(position) = pointer.filter(|position| rect.contains(*position)) {
            let raw = self.screen_to_world(rect, position);
            let point = if shift { self.snap_point(raw) } else { raw };
            self.measurement = Some(DraftWall {
                start: point,
                end: point,
            });
        }
        if down
            && let Some(position) = pointer
            && self.measurement.is_some()
        {
            let raw = self.screen_to_world(rect, position);
            let end = if shift { self.snap_point(raw) } else { raw };
            if let Some(measurement) = &mut self.measurement {
                measurement.end = end;
            }
        }
        if released {
            self.measurement = None;
        }
    }

    fn handle_arrow_events(&mut self, ctx: &egui::Context, rect: Rect) {
        let (pressed, down, released, pointer) = ctx.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.pointer.button_released(PointerButton::Primary),
                input.pointer.interact_pos(),
            )
        });
        if pressed && let Some(position) = pointer.filter(|position| rect.contains(*position)) {
            let point = self.snap_point(self.screen_to_world(rect, position));
            self.arrow_draft = Some(DraftWall {
                start: point,
                end: point,
            });
        }
        if down
            && let Some(position) = pointer
            && let Some(start) = self.arrow_draft.as_ref().map(|draft| draft.start)
        {
            let raw = self.screen_to_world(rect, position);
            let length = start.distance(raw);
            let orientation = arrow_orientation_step(start, raw);
            let angle = orientation as f32 * PlanArrow::DEGREES_PER_STEP;
            let end = Point::new(
                start.x + length * angle.to_radians().cos(),
                start.y + length * angle.to_radians().sin(),
            );
            if let Some(draft) = &mut self.arrow_draft {
                draft.end = end;
            }
        }
        if released
            && let Some(draft) = self.arrow_draft.take()
            && draft.start.distance(draft.end) >= 5.0
        {
            let before = self.snapshot();
            let arrow = PlanArrow {
                id: make_id("arrow"),
                x: draft.start.x,
                y: draft.start.y,
                length: draft.start.distance(draft.end),
                width: 20.0,
                style: ArrowStyle::Open,
                orientation: arrow_orientation_step(draft.start, draft.end),
                color: default_arrow_color(),
                locked: false,
            };
            let id = arrow.id.clone();
            self.plan.arrows.push(arrow);
            self.assign_to_active_layer(&id);
            self.selection = Some(Selection::Arrow(id));
            self.status = self.localizer.text("status.arrow_added");
            self.commit(before);
            self.tool = Tool::Select;
        }
    }

    fn handle_area_events(&mut self, ctx: &egui::Context, response: &Response, rect: Rect) {
        let finish = ctx.input(|input| input.key_pressed(Key::Enter))
            || response.double_clicked_by(PointerButton::Primary)
            || response.clicked_by(PointerButton::Secondary);
        if finish {
            self.finish_area();
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
                .area_draft
                .last()
                .is_none_or(|previous| previous.distance(point) >= 1.0)
            {
                self.area_draft.push(point);
            }
        }
    }

    fn finish_area(&mut self) {
        let points = normalized_polygon_points(&std::mem::take(&mut self.area_draft));
        if points.len() < 3 {
            return;
        }
        let before = self.snapshot();
        let area = AreaShape {
            id: make_id("area"),
            points,
            fill_color: self.default_area_fill_color,
            stroke_color: default_area_stroke_color(),
            locked: false,
        };
        let id = area.id.clone();
        self.plan.areas.push(area);
        self.assign_to_active_layer(&id);
        self.selection = Some(Selection::Area(id));
        self.status = self.localizer.text("status.area_added");
        self.commit(before);
        self.tool = Tool::Select;
    }

    fn handle_wall_events(&mut self, ctx: &egui::Context, rect: Rect) {
        let events = ctx.input(|input| input.events.clone());

        for event in events {
            match event {
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers,
                } => {
                    if pressed && rect.contains(pos) {
                        let start = self.wall_snap_point(self.screen_to_world(rect, pos));
                        self.wall_press_origin = Some(start);
                        self.wall_press_created_draft = self.draft.is_none();
                        if self.wall_press_created_draft {
                            self.draft = Some(DraftWall { start, end: start });
                        }
                    } else if !pressed
                        && let Some(press_origin) = self.wall_press_origin.take()
                        && let Some(mut draft) = self.draft.take()
                    {
                        let end = self.magnet_wall_endpoint(self.snap_with_axis(
                            self.screen_to_world(rect, pos),
                            draft.start,
                            modifiers.shift,
                        ));
                        let was_drag = press_origin.distance(end) >= MIN_WALL;
                        if !self.wall_press_created_draft || was_drag {
                            draft.end = end;
                            self.add_wall_segment(draft);
                            self.draft = Some(DraftWall { start: end, end });
                        } else {
                            self.draft = Some(DraftWall {
                                start: press_origin,
                                end: press_origin,
                            });
                        }
                    }
                }
                Event::PointerMoved(pos) if self.draft.is_some() => {
                    let shift = ctx.input(|input| input.modifiers.shift);
                    let world = self.screen_to_world(rect, pos);
                    let start = self.draft.as_ref().map(|draft| draft.start);
                    let snapped = start.map(|start| {
                        self.magnet_wall_endpoint(self.snap_with_axis(world, start, shift))
                    });
                    if let Some(draft) = &mut self.draft {
                        draft.end = snapped.unwrap_or(world);
                    }
                }
                Event::PointerButton {
                    button: PointerButton::Secondary,
                    pressed: false,
                    ..
                } => {
                    self.draft = None;
                    self.wall_press_origin = None;
                }
                _ => {}
            }
        }
    }

    fn add_wall_segment(&mut self, draft: DraftWall) {
        if draft.start.distance(draft.end) < MIN_WALL {
            return;
        }
        let before = self.snapshot();
        let wall = Wall {
            id: make_id("wall"),
            x1: draft.start.x,
            y1: draft.start.y,
            x2: draft.end.x,
            y2: draft.end.y,
            thickness: self.default_wall_thickness,
            kind: WallKind::Interior,
            color: self.default_wall_color,
            locked: false,
        };
        let id = wall.id.clone();
        self.plan.walls.push(wall);
        self.assign_to_active_layer(&id);
        self.refresh_automatic_rooms();
        self.selection = Some(Selection::Wall(id));
        self.status = self.localizer.text("status.wall_added");
        self.commit(before);
    }

    fn begin_selected_drag(&mut self, pointer: Point) -> bool {
        if self
            .selection
            .as_ref()
            .is_some_and(|selection| !self.selection_editable(selection))
        {
            return true;
        }
        let threshold = 14.0 / self.zoom;
        let selection = self.selection.clone();
        let selected_is_hit = match &selection {
            Some(Selection::Door(id)) => self.plan.doors.iter().any(|door| {
                door.id == *id
                    && self
                        .plan
                        .walls
                        .iter()
                        .find(|wall| wall.id == door.wall_id)
                        .is_some_and(|wall| pointer.distance(wall.point_at(door.t)) <= threshold)
            }),
            Some(Selection::Window(id)) => self.plan.windows.iter().any(|window| {
                window.id == *id
                    && self
                        .plan
                        .walls
                        .iter()
                        .find(|wall| wall.id == window.wall_id)
                        .is_some_and(|wall| pointer.distance(wall.point_at(window.t)) <= threshold)
            }),
            Some(Selection::Wall(id)) => self
                .plan
                .walls
                .iter()
                .find(|wall| wall.id == *id)
                .is_some_and(|wall| projection(wall, pointer).2 <= threshold),
            Some(Selection::Line(id)) => self
                .plan
                .lines
                .iter()
                .find(|line| line.id == *id)
                .is_some_and(|line| line_distance(line, pointer) <= threshold),
            Some(Selection::Text(id)) => self
                .plan
                .texts
                .iter()
                .find(|text| text.id == *id)
                .is_some_and(|text| point_in_text(pointer, text)),
            Some(Selection::Object(id)) => self
                .plan
                .objects
                .iter()
                .find(|object| object.id == *id)
                .is_some_and(|object| {
                    point_in_object(pointer, object)
                        || object_resize_handles(object)
                            .iter()
                            .any(|handle| handle.distance(pointer) <= threshold)
                }),
            Some(Selection::Image(id)) => self
                .plan
                .images
                .iter()
                .find(|image| image.id == *id)
                .is_some_and(|image| point_in_image(pointer, image)),
            Some(Selection::Area(id)) => self
                .plan
                .areas
                .iter()
                .find(|area| area.id == *id)
                .is_some_and(|area| point_in_area(pointer, area)),
            Some(Selection::Room(id)) => self
                .plan
                .rooms
                .iter()
                .find(|room| room.id == *id)
                .is_some_and(|room| crate::room_detection::point_in_polygon(pointer, &room.points)),
            Some(Selection::Compass(id)) => self
                .plan
                .compasses
                .iter()
                .find(|compass| compass.id == *id)
                .is_some_and(|compass| point_in_compass(pointer, compass)),
            Some(Selection::Arrow(id)) => self
                .plan
                .arrows
                .iter()
                .find(|arrow| arrow.id == *id)
                .is_some_and(|arrow| {
                    let end = arrow.end();
                    line_distance(
                        &PlanLine {
                            id: String::new(),
                            x1: arrow.x,
                            y1: arrow.y,
                            x2: end.x,
                            y2: end.y,
                            color: arrow.color,
                            locked: arrow.locked,
                        },
                        pointer,
                    ) <= threshold.max(arrow.width * 0.5)
                }),
            None => false,
        };
        if !selected_is_hit {
            return false;
        }
        match selection {
            Some(Selection::Door(_)) => self.begin_door_drag(pointer),
            Some(Selection::Window(_)) => self.begin_window_drag(pointer),
            Some(Selection::Wall(_)) => self.begin_wall_drag(pointer),
            Some(Selection::Line(_)) => self.begin_line_drag(pointer),
            Some(Selection::Text(_)) => self.begin_text_drag(pointer),
            Some(Selection::Object(_)) => self.begin_object_drag(pointer),
            Some(Selection::Image(_)) => self.begin_image_drag(pointer),
            Some(Selection::Area(_)) => self.begin_area_drag(pointer),
            Some(Selection::Room(_)) => self.begin_room_drag(pointer),
            Some(Selection::Compass(_)) => self.begin_compass_drag(pointer),
            Some(Selection::Arrow(_)) => self.begin_arrow_drag(pointer),
            None => false,
        }
    }

    fn begin_group_drag(&mut self, pointer: Point) -> bool {
        if self.additional_selections.is_empty() {
            return false;
        }
        let hits = self.hit_selections(pointer);
        if !hits.iter().any(|hit| self.is_selected(hit)) {
            return false;
        }
        let mut selections = self.additional_selections.clone();
        if let Some(primary) = self.selection.clone() {
            selections.insert(0, primary);
        }
        self.group_drag = Some(GroupDrag {
            selections,
            last_pointer: pointer,
            before: self.snapshot(),
        });
        true
    }

    fn update_group_drag(&mut self, pointer: Point) {
        let Some(drag) = self.group_drag.as_mut() else {
            return;
        };
        let delta = Point::new(
            pointer.x - drag.last_pointer.x,
            pointer.y - drag.last_pointer.y,
        );
        drag.last_pointer = pointer;
        let selections = drag.selections.clone();
        let movable = selections
            .iter()
            .filter(|selection| !opening_follows_selected_wall(&self.plan, selection, &selections))
            .cloned()
            .collect::<Vec<_>>();
        for selection in &movable {
            self.translate_selection(selection, delta);
        }
    }

    fn translate_selection(&mut self, selection: &Selection, delta: Point) {
        if !self.selection_editable(selection) {
            return;
        }
        match selection {
            Selection::Wall(id) => {
                if let Some(item) = self.plan.walls.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                {
                    item.x1 += delta.x;
                    item.y1 += delta.y;
                    item.x2 += delta.x;
                    item.y2 += delta.y;
                }
            }
            Selection::Line(id) => {
                if let Some(item) = self.plan.lines.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                {
                    item.x1 += delta.x;
                    item.y1 += delta.y;
                    item.x2 += delta.x;
                    item.y2 += delta.y;
                }
            }
            Selection::Text(id) => {
                if let Some(item) = self.plan.texts.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                {
                    item.x += delta.x;
                    item.y += delta.y;
                }
            }
            Selection::Image(id) => {
                if let Some(item) = self.plan.images.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                {
                    item.x += delta.x;
                    item.y += delta.y;
                }
            }
            Selection::Object(id) => {
                if let Some(item) = self.plan.objects.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                {
                    item.x += delta.x;
                    item.y += delta.y;
                }
            }
            Selection::Area(id) => {
                if let Some(item) = self.plan.areas.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                {
                    for point in &mut item.points {
                        point.x += delta.x;
                        point.y += delta.y;
                    }
                }
            }
            Selection::Room(id) => {
                if let Some(item) = self.plan.rooms.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                    && item.source == RoomSource::Manual
                {
                    for point in &mut item.points {
                        point.x += delta.x;
                        point.y += delta.y;
                    }
                }
            }
            Selection::Compass(id) => {
                if let Some(item) = self.plan.compasses.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                {
                    item.x += delta.x;
                    item.y += delta.y;
                }
            }
            Selection::Arrow(id) => {
                if let Some(item) = self.plan.arrows.iter_mut().find(|item| &item.id == id)
                    && !item.locked
                {
                    item.x += delta.x;
                    item.y += delta.y;
                }
            }
            Selection::Door(id) => {
                let Some(item) = self.plan.doors.iter_mut().find(|item| &item.id == id) else {
                    return;
                };
                let Some(wall) = self.plan.walls.iter().find(|wall| wall.id == item.wall_id) else {
                    return;
                };
                let length = wall.length();
                if item.locked || length <= f32::EPSILON {
                    return;
                }
                let projected =
                    (delta.x * (wall.x2 - wall.x1) + delta.y * (wall.y2 - wall.y1)) / length;
                let margin = item.width / (2.0 * length);
                item.t = (item.t + projected / length).clamp(margin, 1.0 - margin);
            }
            Selection::Window(id) => {
                let Some(item) = self.plan.windows.iter_mut().find(|item| &item.id == id) else {
                    return;
                };
                let Some(wall) = self.plan.walls.iter().find(|wall| wall.id == item.wall_id) else {
                    return;
                };
                let length = wall.length();
                if item.locked || length <= f32::EPSILON {
                    return;
                }
                let projected =
                    (delta.x * (wall.x2 - wall.x1) + delta.y * (wall.y2 - wall.y1)) / length;
                let margin = item.width / (2.0 * length);
                item.t = (item.t + projected / length).clamp(margin, 1.0 - margin);
            }
        }
    }

    fn begin_area_drag(&mut self, pointer: Point) -> bool {
        let area = self
            .plan
            .areas
            .iter()
            .rev()
            .find(|area| point_in_area(pointer, area))
            .cloned();
        let Some(area) = area else {
            return false;
        };
        self.selection = Some(Selection::Area(area.id.clone()));
        if area.locked {
            self.status = self.localizer.text("status.area_locked");
            return true;
        }
        self.area_drag = Some(AreaDrag {
            area_id: area.id.clone(),
            pointer_start: pointer,
            original: area,
            before: self.snapshot(),
        });
        true
    }

    fn update_area_drag(&mut self, pointer: Point) {
        let Some(drag) = self.area_drag.clone() else {
            return;
        };
        let dx = pointer.x - drag.pointer_start.x;
        let dy = pointer.y - drag.pointer_start.y;
        if let Some(area) = self
            .plan
            .areas
            .iter_mut()
            .find(|area| area.id == drag.area_id)
        {
            for (point, original) in area.points.iter_mut().zip(&drag.original.points) {
                point.x = original.x + dx;
                point.y = original.y + dy;
            }
        }
    }

    fn begin_compass_drag(&mut self, pointer: Point) -> bool {
        let compass = self
            .plan
            .compasses
            .iter()
            .rev()
            .find(|compass| point_in_compass(pointer, compass))
            .cloned();
        let Some(compass) = compass else {
            return false;
        };
        self.selection = Some(Selection::Compass(compass.id.clone()));
        if compass.locked {
            self.status = self.localizer.text("status.compass_locked");
            return true;
        }
        self.compass_drag = Some(CompassDrag {
            compass_id: compass.id.clone(),
            pointer_start: pointer,
            original: compass,
            before: self.snapshot(),
        });
        true
    }

    fn update_compass_drag(&mut self, pointer: Point) {
        let Some(drag) = self.compass_drag.clone() else {
            return;
        };
        if let Some(compass) = self
            .plan
            .compasses
            .iter_mut()
            .find(|compass| compass.id == drag.compass_id)
        {
            compass.x = drag.original.x + pointer.x - drag.pointer_start.x;
            compass.y = drag.original.y + pointer.y - drag.pointer_start.y;
        }
    }

    fn begin_text_drag(&mut self, pointer: Point) -> bool {
        let selected_id = match &self.selection {
            Some(Selection::Text(id)) => Some(id.as_str()),
            _ => None,
        };
        let text = selected_id
            .and_then(|id| {
                self.plan
                    .texts
                    .iter()
                    .find(|text| text.id == id && point_in_text(pointer, text))
            })
            .or_else(|| {
                self.plan
                    .texts
                    .iter()
                    .rev()
                    .find(|text| point_in_text(pointer, text))
            })
            .cloned();
        let Some(text) = text else {
            return false;
        };
        self.selection = Some(Selection::Text(text.id.clone()));
        if text.locked {
            self.status = self.localizer.text("status.text_locked");
            return true;
        }
        self.text_drag = Some(TextDrag {
            text_id: text.id.clone(),
            pointer_start: pointer,
            original: text,
            before: self.snapshot(),
        });
        true
    }

    fn update_text_drag(&mut self, pointer: Point) {
        let Some(drag) = self.text_drag.clone() else {
            return;
        };
        let dx = pointer.x - drag.pointer_start.x;
        let dy = pointer.y - drag.pointer_start.y;
        let (dx, dy) = if self.snap_enabled {
            (
                (dx / self.grid_size).round() * self.grid_size,
                (dy / self.grid_size).round() * self.grid_size,
            )
        } else {
            (dx, dy)
        };
        if let Some(text) = self
            .plan
            .texts
            .iter_mut()
            .find(|text| text.id == drag.text_id)
        {
            text.x = drag.original.x + dx;
            text.y = drag.original.y + dy;
        }
    }

    fn begin_line_drag(&mut self, pointer: Point) -> bool {
        let threshold = 11.0 / self.zoom;
        let selected_id = match &self.selection {
            Some(Selection::Line(id)) => Some(id.as_str()),
            _ => None,
        };
        let line = selected_id
            .and_then(|id| {
                self.plan
                    .lines
                    .iter()
                    .find(|line| line.id == id && line_distance(line, pointer) <= threshold)
            })
            .or_else(|| {
                self.plan
                    .lines
                    .iter()
                    .filter(|line| line_distance(line, pointer) <= threshold)
                    .min_by(|a, b| line_distance(a, pointer).total_cmp(&line_distance(b, pointer)))
            })
            .cloned();
        let Some(line) = line else {
            return false;
        };
        self.selection = Some(Selection::Line(line.id.clone()));
        if line.locked {
            self.status = self.localizer.text("status.line_locked");
            return true;
        }
        self.line_drag = Some(LineDrag {
            line_id: line.id.clone(),
            pointer_start: pointer,
            original: line,
            before: self.snapshot(),
        });
        true
    }

    fn update_line_drag(&mut self, pointer: Point) {
        let Some(drag) = self.line_drag.clone() else {
            return;
        };
        let dx = pointer.x - drag.pointer_start.x;
        let dy = pointer.y - drag.pointer_start.y;
        let (dx, dy) = if self.snap_enabled {
            (
                (dx / self.grid_size).round() * self.grid_size,
                (dy / self.grid_size).round() * self.grid_size,
            )
        } else {
            (dx, dy)
        };
        if let Some(line) = self
            .plan
            .lines
            .iter_mut()
            .find(|line| line.id == drag.line_id)
        {
            line.x1 = drag.original.x1 + dx;
            line.y1 = drag.original.y1 + dy;
            line.x2 = drag.original.x2 + dx;
            line.y2 = drag.original.y2 + dy;
        }
    }

    fn begin_door_drag(&mut self, pointer: Point) -> bool {
        let threshold = 14.0 / self.zoom;
        let selected_id = match &self.selection {
            Some(Selection::Door(id)) => Some(id.as_str()),
            _ => None,
        };
        let preferred = selected_id.and_then(|id| {
            self.plan.doors.iter().find(|door| {
                door.id == id
                    && self
                        .plan
                        .walls
                        .iter()
                        .find(|wall| wall.id == door.wall_id)
                        .is_some_and(|wall| pointer.distance(wall.point_at(door.t)) <= threshold)
            })
        });
        let closest = preferred.cloned().or_else(|| {
            self.plan
                .doors
                .iter()
                .filter_map(|door| {
                    let wall = self
                        .plan
                        .walls
                        .iter()
                        .find(|wall| wall.id == door.wall_id)?;
                    Some((door, pointer.distance(wall.point_at(door.t))))
                })
                .filter(|(_, distance)| *distance <= threshold)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(door, _)| door.clone())
        });

        let Some(door) = closest else {
            return false;
        };
        self.selection = Some(Selection::Door(door.id.clone()));
        if door.locked {
            self.status = self.localizer.text("status.door_locked");
            return true;
        }
        self.door_drag = Some(DoorDrag {
            door_id: door.id,
            wall_id: door.wall_id,
            before: self.snapshot(),
        });
        true
    }

    fn update_door_drag(&mut self, pointer: Point) {
        let Some(drag) = self.door_drag.clone() else {
            return;
        };
        let Some(wall) = self.plan.walls.iter().find(|wall| wall.id == drag.wall_id) else {
            return;
        };
        let (t, _, _) = projection(wall, pointer);
        let length = wall.length();
        let Some(door) = self
            .plan
            .doors
            .iter_mut()
            .find(|door| door.id == drag.door_id)
        else {
            return;
        };
        if length > 0.0 {
            let margin = (door.width / (2.0 * length)).min(0.5);
            let candidate = t.clamp(margin, 1.0 - margin);
            let door_id = door.id.clone();
            let wall_id = door.wall_id.clone();
            let width = door.width;
            let _ = door;
            if self.opening_available(&wall_id, candidate, width, Some(&door_id))
                && let Some(door) = self.plan.doors.iter_mut().find(|door| door.id == door_id)
            {
                door.t = candidate;
            }
        }
    }

    fn begin_window_drag(&mut self, pointer: Point) -> bool {
        let threshold = 14.0 / self.zoom;
        let selected_id = match &self.selection {
            Some(Selection::Window(id)) => Some(id.as_str()),
            _ => None,
        };
        let preferred = selected_id.and_then(|id| {
            self.plan.windows.iter().find(|window| {
                window.id == id
                    && self
                        .plan
                        .walls
                        .iter()
                        .find(|wall| wall.id == window.wall_id)
                        .is_some_and(|wall| pointer.distance(wall.point_at(window.t)) <= threshold)
            })
        });
        let closest = preferred.cloned().or_else(|| {
            self.plan
                .windows
                .iter()
                .filter_map(|window| {
                    let wall = self
                        .plan
                        .walls
                        .iter()
                        .find(|wall| wall.id == window.wall_id)?;
                    Some((window, pointer.distance(wall.point_at(window.t))))
                })
                .filter(|(_, distance)| *distance <= threshold)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(window, _)| window.clone())
        });
        let Some(window) = closest else {
            return false;
        };
        self.selection = Some(Selection::Window(window.id.clone()));
        if window.locked {
            self.status = self.localizer.text("status.window_locked");
            return true;
        }
        self.window_drag = Some(WindowDrag {
            window_id: window.id,
            wall_id: window.wall_id,
            before: self.snapshot(),
        });
        true
    }

    fn update_window_drag(&mut self, pointer: Point) {
        let Some(drag) = self.window_drag.clone() else {
            return;
        };
        let Some(wall) = self.plan.walls.iter().find(|wall| wall.id == drag.wall_id) else {
            return;
        };
        let (position, _, _) = projection(wall, pointer);
        let length = wall.length();
        let Some(window) = self
            .plan
            .windows
            .iter()
            .find(|window| window.id == drag.window_id)
        else {
            return;
        };
        if length <= 0.0 {
            return;
        }
        let margin = (window.width / (2.0 * length)).min(0.5);
        let candidate = position.clamp(margin, 1.0 - margin);
        let width = window.width;
        if self.opening_available(&drag.wall_id, candidate, width, Some(&drag.window_id))
            && let Some(window) = self
                .plan
                .windows
                .iter_mut()
                .find(|window| window.id == drag.window_id)
        {
            window.t = candidate;
        }
    }

    fn begin_image_drag(&mut self, pointer: Point) -> bool {
        let selected_id = match &self.selection {
            Some(Selection::Image(id)) => Some(id.as_str()),
            _ => None,
        };
        let image = selected_id
            .and_then(|id| {
                self.plan
                    .images
                    .iter()
                    .find(|image| image.id == id && point_in_image(pointer, image))
            })
            .or_else(|| {
                self.plan
                    .images
                    .iter()
                    .rev()
                    .find(|image| point_in_image(pointer, image))
            })
            .cloned();
        let Some(image) = image else {
            return false;
        };
        self.selection = Some(Selection::Image(image.id.clone()));
        if image.locked {
            self.status = self.localizer.text("status.image_locked");
            return true;
        }
        self.image_drag = Some(ImageDrag {
            image_id: image.id.clone(),
            pointer_start: pointer,
            original: image,
            before: self.snapshot(),
        });
        true
    }

    fn update_image_drag(&mut self, pointer: Point) {
        let Some(drag) = self.image_drag.clone() else {
            return;
        };
        let dx = pointer.x - drag.pointer_start.x;
        let dy = pointer.y - drag.pointer_start.y;
        let (dx, dy) = if self.snap_enabled {
            (
                (dx / self.grid_size).round() * self.grid_size,
                (dy / self.grid_size).round() * self.grid_size,
            )
        } else {
            (dx, dy)
        };
        if let Some(image) = self
            .plan
            .images
            .iter_mut()
            .find(|image| image.id == drag.image_id)
        {
            image.x = drag.original.x + dx;
            image.y = drag.original.y + dy;
        }
    }

    fn begin_object_drag(&mut self, pointer: Point) -> bool {
        let selected_id = match &self.selection {
            Some(Selection::Object(id)) => Some(id.as_str()),
            _ => None,
        };
        let selected_object = selected_id.and_then(|id| {
            self.plan
                .objects
                .iter()
                .find(|object| object.id == id)
                .cloned()
        });
        let resize_handle = selected_object.as_ref().and_then(|object| {
            let threshold = 12.0 / self.zoom;
            object_resize_handles(object)
                .iter()
                .position(|handle| handle.distance(pointer) <= threshold)
        });
        let object = selected_object
            .filter(|object| resize_handle.is_some() || point_in_object(pointer, object))
            .or_else(|| {
                selected_id.and_then(|id| {
                    self.plan
                        .objects
                        .iter()
                        .find(|object| object.id == id && point_in_object(pointer, object))
                        .cloned()
                })
            })
            .or_else(|| {
                self.plan
                    .objects
                    .iter()
                    .rev()
                    .find(|object| point_in_object(pointer, object))
                    .cloned()
            });
        let Some(object) = object else {
            return false;
        };
        self.selection = Some(Selection::Object(object.id.clone()));
        if object.locked {
            self.status = self.localizer.text("status.object_locked");
            return true;
        }
        self.object_drag = Some(ObjectDrag {
            object_id: object.id.clone(),
            pointer_start: pointer,
            original: object,
            resize_handle,
            before: self.snapshot(),
        });
        true
    }

    fn update_object_drag(&mut self, pointer: Point) {
        let Some(drag) = self.object_drag.clone() else {
            return;
        };
        if let Some(handle) = drag.resize_handle {
            let angle = drag.original.rotation.to_radians();
            let cos = angle.cos();
            let sin = angle.sin();
            let world_dx = pointer.x - drag.original.x;
            let world_dy = pointer.y - drag.original.y;
            let local_pointer = Point::new(
                world_dx * cos + world_dy * sin,
                -world_dx * sin + world_dy * cos,
            );
            let signs = [
                (-1.0, -1.0),
                (0.0, -1.0),
                (1.0, -1.0),
                (1.0, 0.0),
                (1.0, 1.0),
                (0.0, 1.0),
                (-1.0, 1.0),
                (-1.0, 0.0),
            ];
            let (sx, sy) = signs[handle];
            let opposite_x = -sx * drag.original.width * 0.5;
            let opposite_y = -sy * drag.original.height * 0.5;
            let mut width = if sx == 0.0 {
                drag.original.width
            } else {
                (local_pointer.x - opposite_x).abs().max(1.0)
            };
            let mut height = if sy == 0.0 {
                drag.original.height
            } else {
                (local_pointer.y - opposite_y).abs().max(1.0)
            };
            if sx != 0.0 && sy != 0.0 && self.context.input(|input| input.modifiers.shift) {
                let ratio = (drag.original.width / drag.original.height).max(0.001);
                if width / height > ratio {
                    height = width / ratio;
                } else {
                    width = height * ratio;
                }
            }
            let center_local_x = if sx == 0.0 {
                0.0
            } else {
                (local_pointer.x + opposite_x) * 0.5
            };
            let center_local_y = if sy == 0.0 {
                0.0
            } else {
                (local_pointer.y + opposite_y) * 0.5
            };
            if let Some(object) = self
                .plan
                .objects
                .iter_mut()
                .find(|object| object.id == drag.object_id)
            {
                object.x = drag.original.x + center_local_x * cos - center_local_y * sin;
                object.y = drag.original.y + center_local_x * sin + center_local_y * cos;
                object.width = width;
                object.height = height;
            }
            return;
        }
        let dx = pointer.x - drag.pointer_start.x;
        let dy = pointer.y - drag.pointer_start.y;
        let (dx, dy) = if self.snap_enabled {
            (
                (dx / self.grid_size).round() * self.grid_size,
                (dy / self.grid_size).round() * self.grid_size,
            )
        } else {
            (dx, dy)
        };
        if let Some(object) = self
            .plan
            .objects
            .iter_mut()
            .find(|object| object.id == drag.object_id)
        {
            object.x = drag.original.x + dx;
            object.y = drag.original.y + dy;
        }
    }

    fn begin_arrow_drag(&mut self, pointer: Point) -> bool {
        let Some(Selection::Arrow(id)) = self.selection.clone() else {
            return false;
        };
        let Some(arrow) = self
            .plan
            .arrows
            .iter()
            .find(|arrow| arrow.id == id)
            .cloned()
        else {
            return false;
        };
        if arrow.locked {
            return true;
        }
        self.arrow_drag = Some(ArrowDrag {
            arrow_id: id,
            pointer_start: pointer,
            original: arrow,
            before: self.snapshot(),
        });
        true
    }

    fn update_arrow_drag(&mut self, pointer: Point) {
        let Some(drag) = self.arrow_drag.clone() else {
            return;
        };
        let mut dx = pointer.x - drag.pointer_start.x;
        let mut dy = pointer.y - drag.pointer_start.y;
        if self.snap_enabled {
            dx = (dx / self.grid_size).round() * self.grid_size;
            dy = (dy / self.grid_size).round() * self.grid_size;
        }
        if let Some(arrow) = self
            .plan
            .arrows
            .iter_mut()
            .find(|arrow| arrow.id == drag.arrow_id)
        {
            arrow.x = drag.original.x + dx;
            arrow.y = drag.original.y + dy;
        }
    }

    fn begin_wall_drag(&mut self, pointer: Point) -> bool {
        let hit_radius = 11.0 / self.zoom;
        let selected_wall_id = match &self.selection {
            Some(Selection::Wall(id)) => Some(id.clone()),
            _ => None,
        };

        if let Some(id) = selected_wall_id
            && let Some(wall) = self.plan.walls.iter().find(|wall| wall.id == id).cloned()
        {
            if wall.locked {
                return true;
            }
            let kind = if pointer.distance(wall.start()) <= hit_radius {
                Some(WallDragKind::Start)
            } else if pointer.distance(wall.end()) <= hit_radius {
                Some(WallDragKind::End)
            } else {
                let (_, _, distance) = projection(&wall, pointer);
                (distance <= hit_radius).then_some(WallDragKind::Move)
            };
            if let Some(kind) = kind {
                self.wall_drag = Some(WallDrag {
                    wall_id: wall.id.clone(),
                    kind,
                    pointer_start: pointer,
                    original: wall,
                    before: self.snapshot(),
                });
                return true;
            }
        }

        if let Some((index, _, distance)) = self.nearest_wall(pointer)
            && distance <= hit_radius
        {
            let wall = self.plan.walls[index].clone();
            self.selection = Some(Selection::Wall(wall.id.clone()));
            if wall.locked {
                self.status = self.localizer.text("status.wall_locked");
                return true;
            }
            self.wall_drag = Some(WallDrag {
                wall_id: wall.id.clone(),
                kind: WallDragKind::Move,
                pointer_start: pointer,
                original: wall,
                before: self.snapshot(),
            });
            return true;
        }
        false
    }

    fn update_wall_drag(&mut self, pointer: Point) {
        let Some(drag) = self.wall_drag.clone() else {
            return;
        };
        let snapped_pointer = self.snap_point(pointer);
        let snap_enabled = self.snap_enabled;
        let grid_size = self.grid_size;
        let Some(wall) = self
            .plan
            .walls
            .iter_mut()
            .find(|wall| wall.id == drag.wall_id)
        else {
            return;
        };
        match drag.kind {
            WallDragKind::Move => {
                let dx = if snap_enabled {
                    (pointer.x - drag.pointer_start.x) / grid_size
                } else {
                    pointer.x - drag.pointer_start.x
                };
                let dy = if snap_enabled {
                    (pointer.y - drag.pointer_start.y) / grid_size
                } else {
                    pointer.y - drag.pointer_start.y
                };
                let dx = if snap_enabled {
                    dx.round() * grid_size
                } else {
                    dx
                };
                let dy = if snap_enabled {
                    dy.round() * grid_size
                } else {
                    dy
                };
                wall.x1 = drag.original.x1 + dx;
                wall.y1 = drag.original.y1 + dy;
                wall.x2 = drag.original.x2 + dx;
                wall.y2 = drag.original.y2 + dy;
            }
            WallDragKind::Start => {
                wall.x1 = snapped_pointer.x;
                wall.y1 = snapped_pointer.y;
            }
            WallDragKind::End => {
                wall.x2 = snapped_pointer.x;
                wall.y2 = snapped_pointer.y;
            }
        }
        if !matches!(drag.kind, WallDragKind::Move) {
            self.repair_openings_for_wall(&drag.wall_id);
        }
    }

    fn select_at(&mut self, pointer: Point, additive: bool) {
        let hits = self.hit_selections(pointer);
        if !additive {
            self.selection = cycle_hit_selection(&hits, self.selection.as_ref());
            self.additional_selections.clear();
            return;
        }

        let Some(clicked) = hits.first().cloned() else {
            return;
        };
        toggle_additive_selection(
            &mut self.selection,
            &mut self.additional_selections,
            clicked,
        );
    }

    fn select_box_contents(&mut self, selection_box: &DraftWall, additive: bool) {
        let matches = selections_in_box(&self.plan, selection_box);
        if !additive {
            self.selection = None;
            self.additional_selections.clear();
        }
        for item in matches {
            if !self.is_selected(&item) {
                if self.selection.is_none() {
                    self.selection = Some(item);
                } else {
                    self.additional_selections.push(item);
                }
            }
        }
        let count = self.selected_items().len();
        self.status = self
            .localizer
            .format("status.items_selected", &[("count", count.to_string())]);
    }

    fn is_selected(&self, selection: &Selection) -> bool {
        self.selection.as_ref() == Some(selection) || self.additional_selections.contains(selection)
    }

    fn hit_selections(&self, pointer: Point) -> Vec<Selection> {
        let threshold = 14.0 / self.zoom;
        let mut hits = Vec::new();

        for compass in self.plan.compasses.iter().rev() {
            if point_in_compass(pointer, compass) {
                hits.push(Selection::Compass(compass.id.clone()));
            }
        }
        for text in self.plan.texts.iter().rev() {
            if point_in_text(pointer, text) {
                hits.push(Selection::Text(text.id.clone()));
            }
        }

        let mut windows = Vec::new();
        for window in &self.plan.windows {
            let Some(wall) = self
                .plan
                .walls
                .iter()
                .find(|wall| wall.id == window.wall_id)
            else {
                continue;
            };
            let distance = pointer.distance(wall.point_at(window.t));
            if distance <= threshold {
                windows.push((distance, Selection::Window(window.id.clone())));
            }
        }
        windows.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits.extend(windows.into_iter().map(|(_, selection)| selection));

        let mut doors = Vec::new();
        for door in &self.plan.doors {
            let Some(wall) = self.plan.walls.iter().find(|wall| wall.id == door.wall_id) else {
                continue;
            };
            let distance = pointer.distance(wall.point_at(door.t));
            if distance <= threshold {
                doors.push((distance, Selection::Door(door.id.clone())));
            }
        }
        doors.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits.extend(doors.into_iter().map(|(_, selection)| selection));

        let mut walls: Vec<(f32, Selection)> = self
            .plan
            .walls
            .iter()
            .filter_map(|wall| {
                let distance = projection(wall, pointer).2;
                (distance <= threshold).then(|| (distance, Selection::Wall(wall.id.clone())))
            })
            .collect();
        walls.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits.extend(walls.into_iter().map(|(_, selection)| selection));

        let mut lines: Vec<(f32, Selection)> = self
            .plan
            .lines
            .iter()
            .filter_map(|line| {
                let distance = line_distance(line, pointer);
                (distance <= threshold).then(|| (distance, Selection::Line(line.id.clone())))
            })
            .collect();
        lines.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits.extend(lines.into_iter().map(|(_, selection)| selection));

        for object in self.plan.objects.iter().rev() {
            if point_in_object(pointer, object) {
                hits.push(Selection::Object(object.id.clone()));
            }
        }
        for image in self.plan.images.iter().rev() {
            if point_in_image(pointer, image) {
                hits.push(Selection::Image(image.id.clone()));
            }
        }
        for area in self.plan.areas.iter().rev() {
            if point_in_area(pointer, area) {
                hits.push(Selection::Area(area.id.clone()));
            }
        }
        for room in self.plan.rooms.iter().rev() {
            if crate::room_detection::point_in_polygon(pointer, &room.points) {
                hits.push(Selection::Room(room.id.clone()));
            }
        }
        for arrow in self.plan.arrows.iter().rev() {
            let line = PlanLine {
                id: String::new(),
                x1: arrow.x,
                y1: arrow.y,
                x2: arrow.end().x,
                y2: arrow.end().y,
                color: arrow.color,
                locked: arrow.locked,
            };
            if line_distance(&line, pointer) <= threshold + arrow.width * 0.5 {
                hits.push(Selection::Arrow(arrow.id.clone()));
            }
        }
        hits.retain(|selection| self.plan.item_editable(layers::selection_id(selection)));
        hits
    }

    fn nearest_wall(&self, point: Point) -> Option<(usize, f32, f32)> {
        self.plan
            .walls
            .iter()
            .enumerate()
            .map(|(index, wall)| {
                let (t, _, distance) = projection(wall, point);
                (index, t, distance)
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
    }
}

impl eframe::App for RppApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cancel_interaction();
        if self.dirty {
            self.autosave();
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !ctx.input(|input| input.focused) {
            self.cancel_interaction();
        }
        if ctx.input(|input| input.viewport().close_requested()) && self.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_action = Some(PendingAction::Close);
        }
        if self.pending_action.is_none()
            && let Some(path) = ctx.input_mut(|input| {
                std::mem::take(&mut input.raw.dropped_files)
                    .into_iter()
                    .find_map(|file| file.path)
            })
        {
            self.request_open_path(path);
        }
        self.keyboard_shortcuts(&ctx);
        // Remember the popup state before drawing the top bar too: selecting a
        // menu item may close the popup during this pass, but that same click
        // must still not continue through to the canvas underneath.
        let popup_was_open = ctx.any_popup_open();
        self.top_bar(ui);
        self.status_bar(ui);
        self.tool_panel(ui);
        self.inspector(ui);
        self.canvas(
            ui,
            popup_was_open
                || ctx.any_popup_open()
                || self.settings_open
                || self.layers_open
                || self.pending_action.is_some(),
        );
        self.confirm_discard_dialog(&ctx);
        self.settings_window(&ctx);
        self.layers_window(&ctx);
    }
}

fn configure_style(ctx: &egui::Context, theme: AppTheme) {
    configure_arial_fonts(ctx);
    let mut visuals = match theme {
        AppTheme::Light => egui::Visuals::light(),
        AppTheme::Dark | AppTheme::WarmDark => egui::Visuals::dark(),
    };
    visuals.panel_fill = match theme {
        AppTheme::Light => SURFACE,
        AppTheme::Dark => Color32::from_rgb(31, 32, 33),
        AppTheme::WarmDark => Color32::from_rgb(39, 38, 35),
    };
    visuals.window_fill = visuals.panel_fill;
    visuals.selection.bg_fill = Color32::from_rgb(220, 234, 245);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.inactive.corner_radius = CornerRadius::ZERO;
    visuals.widgets.hovered.corner_radius = CornerRadius::ZERO;
    visuals.widgets.active.corner_radius = CornerRadius::ZERO;
    visuals.widgets.open.corner_radius = CornerRadius::ZERO;
    ctx.set_visuals(visuals);

    let egui_theme = if theme == AppTheme::Light {
        Theme::Light
    } else {
        Theme::Dark
    };
    let mut style = (*ctx.style_of(egui_theme)).clone();
    style.spacing.item_spacing = Vec2::new(6.0, 6.0);
    style.spacing.button_padding = Vec2::new(9.0, 5.0);
    ctx.set_style_of(egui_theme, style);
}

fn arial_font_family(bold: bool) -> FontFamily {
    FontFamily::Name(
        if bold {
            ARIAL_BOLD_FAMILY
        } else {
            ARIAL_REGULAR_FAMILY
        }
        .into(),
    )
}

fn configure_arial_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut regular = fallback.clone();
    if let Some(font) = platform::editor_font(resvg::usvg::fontdb::Weight::NORMAL) {
        let name = "rpp-arial-regular".to_owned();
        fonts.font_data.insert(name.clone(), Arc::new(font));
        regular.insert(0, name);
    }
    let mut bold = regular.clone();
    if let Some(font) = platform::editor_font(resvg::usvg::fontdb::Weight::BOLD) {
        let name = "rpp-arial-bold".to_owned();
        fonts.font_data.insert(name.clone(), Arc::new(font));
        bold.insert(0, name);
    }
    fonts
        .families
        .insert(FontFamily::Name(ARIAL_REGULAR_FAMILY.into()), regular);
    fonts
        .families
        .insert(FontFamily::Name(ARIAL_BOLD_FAMILY.into()), bold);
    ctx.set_fonts(fonts);
}

fn flat_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(text).frame(false))
}

fn tool_button(ui: &mut Ui, current: &mut Tool, tool: Tool, label: &str, shortcut: &str) {
    let selected = *current == tool;
    let text = egui::RichText::new(format!("{label:<11} {shortcut}"))
        .size(13.0)
        .color(if selected {
            Color32::from_rgb(13, 82, 138)
        } else {
            ui.visuals().text_color()
        });
    let button = egui::Button::new(text)
        .selected(selected)
        .min_size(Vec2::new(ui.available_width(), 36.0))
        .corner_radius(CornerRadius::ZERO);
    if ui.add(button).clicked() {
        *current = tool;
    }
}

fn property_row<R>(ui: &mut Ui, label: &str, content: impl FnOnce(&mut Ui) -> R) -> R {
    ui.separator();
    ui.horizontal(|ui| {
        ui.set_min_height(36.0);
        ui.label(
            egui::RichText::new(label)
                .size(12.0)
                .color(Color32::from_gray(85)),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), content)
            .inner
    })
    .inner
}

fn distance_drag(
    ui: &mut Ui,
    centimeters: &mut f32,
    unit: Unit,
    minimum_cm: f32,
    maximum_cm: f32,
) -> Response {
    let mut display = unit.display_from_centimeters(*centimeters);
    let response = ui.add(
        egui::DragValue::new(&mut display)
            .range(
                unit.display_from_centimeters(minimum_cm)
                    ..=unit.display_from_centimeters(maximum_cm),
            )
            .speed(if unit == Unit::M { 0.01 } else { 1.0 })
            .suffix(unit.suffix()),
    );
    if response.changed() {
        *centimeters = unit.to_centimeters(display);
    }
    response
}

fn projection(wall: &Wall, point: Point) -> (f32, Point, f32) {
    let dx = wall.x2 - wall.x1;
    let dy = wall.y2 - wall.y1;
    let squared = dx * dx + dy * dy;
    if squared <= f32::EPSILON {
        return (0.5, wall.start(), point.distance(wall.start()));
    }
    let t = (((point.x - wall.x1) * dx + (point.y - wall.y1) * dy) / squared).clamp(0.0, 1.0);
    let projected = wall.point_at(t);
    (t, projected, point.distance(projected))
}

fn magnet_to_wall(walls: &[Wall], point: Point, threshold: f32) -> Point {
    let endpoint = walls
        .iter()
        .flat_map(|wall| [wall.start(), wall.end()])
        .map(|candidate| (candidate, point.distance(candidate)))
        .filter(|(_, distance)| *distance <= threshold)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((candidate, _)) = endpoint {
        return candidate;
    }
    let mut best = None::<(Point, f32)>;
    for wall in walls {
        let (_, projected, projected_distance) = projection(wall, point);
        if projected_distance <= threshold
            && best.is_none_or(|(_, best_distance)| projected_distance < best_distance)
        {
            best = Some((projected, projected_distance));
        }
    }
    best.map_or(point, |(candidate, _)| candidate)
}

fn line_distance(line: &PlanLine, point: Point) -> f32 {
    let dx = line.x2 - line.x1;
    let dy = line.y2 - line.y1;
    let squared = dx * dx + dy * dy;
    if squared <= f32::EPSILON {
        return point.distance(line.start());
    }
    let t = (((point.x - line.x1) * dx + (point.y - line.y1) * dy) / squared).clamp(0.0, 1.0);
    point.distance(Point::new(line.x1 + dx * t, line.y1 + dy * t))
}

fn arrow_orientation_step(start: Point, end: Point) -> u8 {
    let degrees = (end.y - start.y)
        .atan2(end.x - start.x)
        .to_degrees()
        .rem_euclid(360.0);
    ((degrees / PlanArrow::DEGREES_PER_STEP).round() as u8) % PlanArrow::ORIENTATION_COUNT
}

fn point_in_image(point: Point, image: &ImageObject) -> bool {
    let dx = point.x - image.x;
    let dy = point.y - image.y;
    let angle = -image.rotation.to_radians();
    let local_x = dx * angle.cos() - dy * angle.sin();
    let local_y = dx * angle.sin() + dy * angle.cos();
    local_x.abs() <= image.width / 2.0 && local_y.abs() <= image.height / 2.0
}

fn point_in_object(point: Point, object: &PlanObject) -> bool {
    let dx = point.x - object.x;
    let dy = point.y - object.y;
    let angle = -object.rotation.to_radians();
    let local_x = dx * angle.cos() - dy * angle.sin();
    let local_y = dx * angle.sin() + dy * angle.cos();
    local_x.abs() <= object.width / 2.0 && local_y.abs() <= object.height / 2.0
}

fn point_in_area(point: Point, area: &AreaShape) -> bool {
    let mut inside = false;
    for (a, b) in area
        .points
        .iter()
        .zip(area.points.iter().cycle().skip(1))
        .take(area.points.len())
    {
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
    }
    inside
}

fn point_in_compass(point: Point, compass: &CompassRose) -> bool {
    point.distance(Point::new(compass.x, compass.y)) <= compass.size * 0.6
}

fn triangulate_polygon(points: &[Pos2]) -> Vec<[Pos2; 3]> {
    if points.len() < 3 {
        return Vec::new();
    }
    let signed_area = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum::<f32>();
    let mut indices: Vec<usize> = if signed_area >= 0.0 {
        (0..points.len()).collect()
    } else {
        (0..points.len()).rev().collect()
    };
    let mut triangles = Vec::with_capacity(points.len() - 2);
    let mut guard = 0;
    while indices.len() > 3 && guard < points.len() * points.len() {
        guard += 1;
        let mut ear = None;
        for index in 0..indices.len() {
            let previous = indices[(index + indices.len() - 1) % indices.len()];
            let current = indices[index];
            let next = indices[(index + 1) % indices.len()];
            let a = points[previous];
            let b = points[current];
            let c = points[next];
            if cross(a, b, c) <= 0.0 {
                continue;
            }
            if indices.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_triangle(points[candidate], a, b, c)
            }) {
                continue;
            }
            ear = Some((index, [a, b, c]));
            break;
        }
        let Some((index, triangle)) = ear else {
            break;
        };
        triangles.push(triangle);
        indices.remove(index);
    }
    if indices.len() == 3 {
        triangles.push([points[indices[0]], points[indices[1]], points[indices[2]]]);
    }
    triangles
}

fn cross(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn point_in_triangle(point: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let ab = cross(a, b, point);
    let bc = cross(b, c, point);
    let ca = cross(c, a, point);
    ab >= 0.0 && bc >= 0.0 && ca >= 0.0
}

fn text_dimensions(text: &TextObject) -> (f32, f32) {
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

fn area_center(area: &AreaShape) -> Point {
    let points = normalized_polygon_points(&area.points);
    if points.is_empty() {
        return Point::default();
    }
    let (x, y) = points
        .iter()
        .fold((0.0, 0.0), |(x, y), point| (x + point.x, y + point.y));
    Point::new(x / points.len() as f32, y / points.len() as f32)
}

fn normalized_polygon_points(points: &[Point]) -> Vec<Point> {
    let mut normalized = Vec::with_capacity(points.len());
    for point in points {
        if normalized
            .last()
            .is_none_or(|previous: &Point| previous.distance(*point) > 0.001)
        {
            normalized.push(*point);
        }
    }
    if normalized.len() > 1
        && normalized
            .first()
            .zip(normalized.last())
            .is_some_and(|(first, last)| first.distance(*last) <= 0.001)
    {
        normalized.pop();
    }
    normalized
}

fn format_area(square_centimeters: f32, unit: Unit) -> String {
    match unit {
        Unit::Mm => format!("{:.0} mm²", square_centimeters * 100.0),
        Unit::Cm => format!("{square_centimeters:.1} cm²"),
        Unit::M => format!("{:.2} m²", square_centimeters / 10_000.0),
        Unit::In => format!("{:.1} in²", square_centimeters / 6.4516),
        Unit::Ft => format!("{:.2} ft²", square_centimeters / 929.0304),
    }
}

fn point_in_text(point: Point, text: &TextObject) -> bool {
    let (width, height) = text_dimensions(text);
    let dx = point.x - text.x;
    let dy = point.y - text.y;
    let angle = -text.rotation.to_radians();
    let local_x = dx * angle.cos() - dy * angle.sin();
    let local_y = dx * angle.sin() + dy * angle.cos();
    local_x.abs() <= width / 2.0 && local_y.abs() <= height / 2.0
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

fn object_resize_handles(object: &PlanObject) -> [Point; 8] {
    let corners = rotated_rectangle(
        object.x,
        object.y,
        object.width,
        object.height,
        object.rotation,
    );
    [
        corners[0],
        corners[0].lerp(corners[1], 0.5),
        corners[1],
        corners[1].lerp(corners[2], 0.5),
        corners[2],
        corners[2].lerp(corners[3], 0.5),
        corners[3],
        corners[3].lerp(corners[0], 0.5),
    ]
}

fn bounds_from_points(points: impl IntoIterator<Item = Point>) -> Option<(Point, Point)> {
    let mut bounds: Option<(Point, Point)> = None;
    for point in points {
        bounds = Some(match bounds {
            Some((minimum, maximum)) => (
                Point::new(minimum.x.min(point.x), minimum.y.min(point.y)),
                Point::new(maximum.x.max(point.x), maximum.y.max(point.y)),
            ),
            None => (point, point),
        });
    }
    bounds
}

fn selection_bounds(plan: &PlanFile, selection: &Selection) -> Option<(Point, Point)> {
    match selection {
        Selection::Wall(id) => {
            let wall = plan.walls.iter().find(|item| &item.id == id)?;
            let padding = wall.thickness * 0.5;
            let (minimum, maximum) = bounds_from_points([wall.start(), wall.end()])?;
            Some((
                Point::new(minimum.x - padding, minimum.y - padding),
                Point::new(maximum.x + padding, maximum.y + padding),
            ))
        }
        Selection::Line(id) => {
            let line = plan.lines.iter().find(|item| &item.id == id)?;
            bounds_from_points([line.start(), line.end()])
        }
        Selection::Text(id) => {
            let text = plan.texts.iter().find(|item| &item.id == id)?;
            let (width, height) = text_dimensions(text);
            bounds_from_points(rotated_rectangle(
                text.x,
                text.y,
                width,
                height,
                text.rotation,
            ))
        }
        Selection::Door(id) => {
            let door = plan.doors.iter().find(|item| &item.id == id)?;
            let wall = plan.walls.iter().find(|wall| wall.id == door.wall_id)?;
            let half = door.width / (2.0 * wall.length().max(f32::EPSILON));
            bounds_from_points([
                wall.point_at((door.t - half).clamp(0.0, 1.0)),
                wall.point_at((door.t + half).clamp(0.0, 1.0)),
            ])
        }
        Selection::Window(id) => {
            let window = plan.windows.iter().find(|item| &item.id == id)?;
            let wall = plan.walls.iter().find(|wall| wall.id == window.wall_id)?;
            let half = window.width / (2.0 * wall.length().max(f32::EPSILON));
            bounds_from_points([
                wall.point_at((window.t - half).clamp(0.0, 1.0)),
                wall.point_at((window.t + half).clamp(0.0, 1.0)),
            ])
        }
        Selection::Image(id) => {
            let image = plan.images.iter().find(|item| &item.id == id)?;
            bounds_from_points(rotated_rectangle(
                image.x,
                image.y,
                image.width,
                image.height,
                image.rotation,
            ))
        }
        Selection::Object(id) => {
            let object = plan.objects.iter().find(|item| &item.id == id)?;
            bounds_from_points(rotated_rectangle(
                object.x,
                object.y,
                object.width,
                object.height,
                object.rotation,
            ))
        }
        Selection::Area(id) => {
            let area = plan.areas.iter().find(|item| &item.id == id)?;
            bounds_from_points(area.points.iter().copied())
        }
        Selection::Room(id) => {
            let room = plan.rooms.iter().find(|item| &item.id == id)?;
            bounds_from_points(room.points.iter().copied())
        }
        Selection::Compass(id) => {
            let compass = plan.compasses.iter().find(|item| &item.id == id)?;
            let radius = compass.size * 0.6;
            Some((
                Point::new(compass.x - radius, compass.y - radius),
                Point::new(compass.x + radius, compass.y + radius),
            ))
        }
        Selection::Arrow(id) => {
            let arrow = plan.arrows.iter().find(|item| &item.id == id)?;
            let padding = arrow.width * 0.5;
            let (minimum, maximum) =
                bounds_from_points([Point::new(arrow.x, arrow.y), arrow.end()])?;
            Some((
                Point::new(minimum.x - padding, minimum.y - padding),
                Point::new(maximum.x + padding, maximum.y + padding),
            ))
        }
    }
}

fn selections_in_box(plan: &PlanFile, selection_box: &DraftWall) -> Vec<Selection> {
    let left_to_right = selection_box.end.x >= selection_box.start.x;
    let minimum = Point::new(
        selection_box.start.x.min(selection_box.end.x),
        selection_box.start.y.min(selection_box.end.y),
    );
    let maximum = Point::new(
        selection_box.start.x.max(selection_box.end.x),
        selection_box.start.y.max(selection_box.end.y),
    );

    selectable_selections(plan)
        .into_iter()
        .filter(|selection| {
            let Some((item_minimum, item_maximum)) = selection_bounds(plan, selection) else {
                return false;
            };
            if left_to_right {
                item_minimum.x >= minimum.x
                    && item_minimum.y >= minimum.y
                    && item_maximum.x <= maximum.x
                    && item_maximum.y <= maximum.y
            } else {
                item_maximum.x >= minimum.x
                    && item_maximum.y >= minimum.y
                    && item_minimum.x <= maximum.x
                    && item_minimum.y <= maximum.y
            }
        })
        .collect()
}

fn opening_follows_selected_wall(
    plan: &PlanFile,
    selection: &Selection,
    selections: &[Selection],
) -> bool {
    let parent_wall = match selection {
        Selection::Door(id) => plan
            .doors
            .iter()
            .find(|item| &item.id == id)
            .map(|item| item.wall_id.as_str()),
        Selection::Window(id) => plan
            .windows
            .iter()
            .find(|item| &item.id == id)
            .map(|item| item.wall_id.as_str()),
        _ => None,
    };
    parent_wall.is_some_and(|wall_id| {
        selections
            .iter()
            .any(|item| matches!(item, Selection::Wall(id) if id == wall_id))
    })
}

fn plan_bounds(plan: &PlanFile) -> Option<(Point, Point)> {
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
        if !plan.item_visible(&wall.id) {
            continue;
        }
        include(wall.start());
        include(wall.end());
    }
    for line in &plan.lines {
        if !plan.item_visible(&line.id) {
            continue;
        }
        include(line.start());
        include(line.end());
    }
    for image in &plan.images {
        if !plan.item_visible(&image.id) {
            continue;
        }
        for point in rotated_rectangle(image.x, image.y, image.width, image.height, image.rotation)
        {
            include(point);
        }
    }
    for object in &plan.objects {
        if !plan.item_visible(&object.id) {
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
    for text in &plan.texts {
        if !plan.item_visible(&text.id) {
            continue;
        }
        let (width, height) = text_dimensions(text);
        for point in rotated_rectangle(text.x, text.y, width, height, text.rotation) {
            include(point);
        }
    }
    for area in &plan.areas {
        if !plan.item_visible(&area.id) {
            continue;
        }
        for point in &area.points {
            include(*point);
        }
    }
    for room in &plan.rooms {
        if !plan.item_visible(&room.id) {
            continue;
        }
        for point in &room.points {
            include(*point);
        }
    }
    for compass in &plan.compasses {
        if !plan.item_visible(&compass.id) {
            continue;
        }
        let radius = compass.size * 0.6;
        include(Point::new(compass.x - radius, compass.y - radius));
        include(Point::new(compass.x + radius, compass.y + radius));
    }
    for arrow in &plan.arrows {
        if !plan.item_visible(&arrow.id) {
            continue;
        }
        let end = arrow.end();
        let radius = arrow.width.max(2.0);
        include(Point::new(arrow.x - radius, arrow.y - radius));
        include(Point::new(arrow.x + radius, arrow.y + radius));
        include(Point::new(end.x - radius, end.y - radius));
        include(Point::new(end.x + radius, end.y + radius));
    }

    bounds
}

fn decode_image(bytes: &[u8]) -> Result<(ColorImage, u32, u32), image::ImageError> {
    let rgba = images::decode(bytes)?.to_rgba8();
    let (width, height) = rgba.dimensions();
    let color_image =
        ColorImage::from_rgba_unmultiplied([width as usize, height as usize], rgba.as_raw());
    Ok((color_image, width, height))
}

fn process_imported_image(
    source: &[u8],
    limit: Option<usize>,
) -> Result<(Vec<u8>, String, ColorImage, u32, u32), image::ImageError> {
    use image::{DynamicImage, GenericImageView, ImageEncoder, imageops::FilterType};

    let decoded = images::decode(source)?;
    if limit.is_none() {
        let (color, width, height) = decode_image(source)?;
        let extension = image::guess_format(source)
            .ok()
            .map(|format| match format {
                image::ImageFormat::Jpeg => "jpg",
                image::ImageFormat::Gif => "gif",
                image::ImageFormat::Bmp => "bmp",
                image::ImageFormat::WebP => "webp",
                _ => "png",
            })
            .unwrap_or("png")
            .to_owned();
        return Ok((source.to_vec(), extension, color, width, height));
    }

    let target = limit.unwrap_or(48 * 1024);
    let has_alpha =
        decoded.color().has_alpha() && decoded.to_rgba8().pixels().any(|pixel| pixel.0[3] < 255);
    let mut working = decoded;
    let mut encoded = Vec::new();
    for _ in 0..24 {
        encoded.clear();
        if has_alpha {
            working.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Png)?;
        } else {
            let rgb = working.to_rgb8();
            let mut best = Vec::new();
            for quality in [84_u8, 72, 60, 48, 36, 28] {
                let mut candidate = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut candidate, quality)
                    .write_image(
                        rgb.as_raw(),
                        rgb.width(),
                        rgb.height(),
                        image::ExtendedColorType::Rgb8,
                    )?;
                best = candidate;
                if best.len() <= target {
                    break;
                }
            }
            encoded = best;
        }
        if encoded.len() <= target {
            break;
        }
        let (width, height) = working.dimensions();
        if width <= 16 || height <= 16 {
            break;
        }
        working = DynamicImage::ImageRgba8(image::imageops::resize(
            &working.to_rgba8(),
            ((width as f32 * 0.82).round() as u32).max(16),
            ((height as f32 * 0.82).round() as u32).max(16),
            FilterType::Lanczos3,
        ));
    }
    let (color, width, height) = decode_image(&encoded)?;
    Ok((
        encoded,
        if has_alpha { "png" } else { "jpg" }.to_owned(),
        color,
        width,
        height,
    ))
}

fn load_embedded_icon(context: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let (image, _, _) = decode_image(bytes).expect("embedded toolbar icon must be a valid image");
    let [width, height] = image.size;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    for y in 0..height {
        for x in 0..width {
            if image.pixels[y * width + x].a() > 8 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    let cropped = if min_x <= max_x && min_y <= max_y {
        let padding = 2;
        min_x = min_x.saturating_sub(padding);
        min_y = min_y.saturating_sub(padding);
        max_x = (max_x + padding).min(width - 1);
        max_y = (max_y + padding).min(height - 1);
        let cropped_width = max_x - min_x + 1;
        let cropped_height = max_y - min_y + 1;
        let mut pixels = Vec::with_capacity(cropped_width * cropped_height);
        for y in min_y..=max_y {
            pixels.extend_from_slice(&image.pixels[y * width + min_x..=y * width + max_x]);
        }
        ColorImage::new([cropped_width, cropped_height], pixels)
    } else {
        image
    };
    context.load_texture(name, cropped, TextureOptions::LINEAR)
}

fn sanitize_asset_name(name: &str) -> String {
    let mut output = String::with_capacity(name.len().min(128));
    for character in name.chars() {
        if output.len() >= 128 {
            break;
        }
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            output.push(character);
        } else if character.is_whitespace() && !output.ends_with('-') {
            output.push('-');
        }
    }
    if output.is_empty() {
        "image".to_owned()
    } else {
        output
    }
}

fn autosave_path() -> Option<PathBuf> {
    platform::recovery_file()
}

fn language_path() -> Option<PathBuf> {
    platform::config_file("language.txt")
}

fn settings_path() -> Option<PathBuf> {
    platform::config_file("settings.json")
}

fn load_settings() -> AppSettings {
    settings_path()
        .and_then(|path| storage::read_limited(&path, 64 * 1024).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &AppSettings) {
    let Some(path) = settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(settings) {
        let _ = storage::write_atomic(&path, &bytes);
    }
}

fn save_language(language: &str) {
    let Some(path) = language_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = storage::write_atomic(&path, language.as_bytes());
}

fn write_plan_safely(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    storage::write_with_backup(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        DraftWall, Selection, cycle_hit_selection, default_image_dimensions, magnet_to_wall,
        normalized_polygon_points, opening_follows_selected_wall, plan_bounds,
        process_imported_image, selectable_selections, selections_in_box,
        toggle_additive_selection, triangulate_polygon, write_plan_safely,
    };
    use crate::model::{
        Door, DoorKind, HingeSide, ObjectKind, PlanFile, PlanObject, Point, SwingSide, Wall,
        WallKind,
    };

    fn test_app(context: eframe::egui::Context) -> super::RppApp {
        let mut app = super::RppApp::from_context(context);
        app.recovery_path = None;
        app.plan = PlanFile::default();
        app.dirty = false;
        app
    }

    #[test]
    fn pinch_zooms_around_pointer_and_respects_limits() {
        use eframe::egui::{CentralPanel, Context, Event, RawInput, Rect, Sense, pos2, vec2};
        let ctx = Context::default();
        let mut app = test_app(ctx.clone());
        let pointer = pos2(320.0, 240.0);
        let mut run = |events| {
            let _ = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |root| {
                    CentralPanel::default().show(root, |ui| {
                        let rect = ui.available_rect_before_wrap();
                        let response = ui.allocate_rect(rect, Sense::click_and_drag());
                        app.handle_canvas_input(&ctx, &response, rect);
                    });
                },
            );
        };
        run(vec![Event::PointerMoved(pointer)]);
        // Establish hover before sending a native macOS pinch event.
        run(vec![]);
        drop(run);
        let initial_zoom = app.zoom;
        let initial_pan = app.pan;
        let mut canvas_rect = Rect::NOTHING;
        for factor in [1.5, 100.0, 0.0001] {
            let previous_zoom = app.zoom;
            let previous_pan = app.pan;
            let _ = ctx.run_ui(
                RawInput {
                    events: vec![Event::Zoom(factor)],
                    ..Default::default()
                },
                |root| {
                    CentralPanel::default().show(root, |ui| {
                        canvas_rect = ui.available_rect_before_wrap();
                        let response = ui.allocate_rect(canvas_rect, Sense::click_and_drag());
                        app.handle_canvas_input(&ctx, &response, canvas_rect);
                    });
                },
            );
            let before = (pointer - canvas_rect.min - previous_pan) / previous_zoom;
            let after = (pointer - canvas_rect.min - app.pan) / app.zoom;
            assert!((before - after).length() < 0.001);
            assert!(
                (app.zoom - (previous_zoom * factor).clamp(super::MIN_ZOOM, super::MAX_ZOOM)).abs()
                    < 0.0001
            );
        }
        assert!(initial_zoom > 0.0);
        assert_ne!(app.pan, initial_pan);
    }

    #[test]
    fn space_drag_pans_without_drawing_even_when_space_is_released_first() {
        use eframe::egui::{
            CentralPanel, Context, Event, Key, Modifiers, PointerButton, RawInput, Rect, Sense,
            pos2, vec2,
        };
        let ctx = Context::default();
        let mut app = test_app(ctx.clone());
        let initial_pan = app.pan;
        let initial_walls = app.plan.walls.len();
        let key = |pressed| Event::Key {
            key: Key::Space,
            physical_key: Some(Key::Space),
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let button = |position, pressed| Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let frames = [
            vec![Event::PointerMoved(pos2(300.0, 200.0))],
            vec![],
            vec![key(true), button(pos2(300.0, 200.0), true)],
            vec![Event::PointerMoved(pos2(330.0, 220.0))],
            vec![key(false)],
            vec![button(pos2(330.0, 220.0), false)],
        ];
        for events in frames {
            let _ = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
                    events,
                    ..RawInput::default()
                },
                |root| {
                    CentralPanel::default().show(root, |ui| {
                        let rect = ui.available_rect_before_wrap();
                        let response = ui.allocate_rect(rect, Sense::click_and_drag());
                        app.handle_canvas_input(&ctx, &response, rect);
                    });
                },
            );
        }
        assert_eq!(app.plan.walls.len(), initial_walls);
        assert!(app.draft.is_none());
        assert!(!app.panning_canvas);
        assert_eq!(app.pan - initial_pan, vec2(30.0, 20.0));
    }

    #[test]
    fn wall_closure_magnet_works_without_grid_snapping() {
        let wall = Wall {
            id: "wall".to_owned(),
            x1: 0.0,
            y1: 0.0,
            x2: 200.0,
            y2: 0.0,
            thickness: 14.0,
            kind: WallKind::Interior,
            color: [52, 56, 59],
            locked: false,
        };
        assert_eq!(
            magnet_to_wall(std::slice::from_ref(&wall), Point::new(198.0, 3.0), 12.0),
            Point::new(200.0, 0.0)
        );
        assert_eq!(
            magnet_to_wall(std::slice::from_ref(&wall), Point::new(100.0, 4.0), 12.0),
            Point::new(100.0, 0.0)
        );
        assert_eq!(
            magnet_to_wall(&[wall], Point::new(100.0, 20.0), 12.0),
            Point::new(100.0, 20.0)
        );
    }

    #[test]
    fn plan_bounds_include_walls_and_rotated_objects() {
        let mut plan = PlanFile::default();
        plan.walls.push(Wall {
            id: "wall-1".to_owned(),
            x1: -20.0,
            y1: 10.0,
            x2: 80.0,
            y2: 10.0,
            thickness: 14.0,
            kind: WallKind::Interior,
            color: [52, 56, 59],
            locked: false,
        });
        plan.objects.push(PlanObject {
            id: "object-1".to_owned(),
            name: String::new(),
            kind: ObjectKind::Table,
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 20.0,
            rotation: 90.0,
            fill_color: [225, 228, 226],
            stroke_color: [90, 90, 90],
            locked: false,
        });

        let (minimum, maximum) = plan_bounds(&plan).expect("plan bounds");
        assert_eq!(minimum, Point::new(-20.0, 10.0));
        assert!((maximum.x - 110.0).abs() < 0.001);
        assert!((maximum.y - 70.0).abs() < 0.001);
    }

    #[test]
    fn empty_plan_has_no_bounds() {
        assert_eq!(plan_bounds(&PlanFile::default()), None);
    }

    #[test]
    fn imported_images_start_small_and_keep_their_aspect_ratio() {
        assert_eq!(default_image_dimensions(4000, 2000), (40.0, 20.0));
        assert_eq!(default_image_dimensions(20, 10), (20.0, 10.0));
    }

    #[test]
    fn opaque_imports_are_encoded_below_the_selected_limit() {
        use image::ImageEncoder;
        let width = 512_u32;
        let height = 512_u32;
        let mut state = 0x1234_5678_u32;
        let mut pixels = Vec::with_capacity((width * height * 3) as usize);
        for _ in 0..width * height {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            pixels.extend_from_slice(&[
                (state >> 24) as u8,
                (state >> 16) as u8,
                (state >> 8) as u8,
            ]);
        }
        let mut source = Vec::new();
        image::codecs::png::PngEncoder::new(&mut source)
            .write_image(&pixels, width, height, image::ExtendedColorType::Rgb8)
            .expect("source PNG");
        let (encoded, extension, _, output_width, output_height) =
            process_imported_image(&source, Some(48 * 1024)).expect("processed image");
        assert_eq!(extension, "jpg");
        assert!(encoded.len() <= 48 * 1024);
        assert!(output_width < width && output_height < height);
    }

    #[test]
    fn repeated_clicks_cycle_through_overlapping_items() {
        let hits = vec![
            Selection::Window("window-1".to_owned()),
            Selection::Wall("wall-1".to_owned()),
            Selection::Image("image-1".to_owned()),
        ];
        assert_eq!(
            cycle_hit_selection(&hits, None),
            Some(Selection::Window("window-1".to_owned()))
        );
        assert_eq!(
            cycle_hit_selection(&hits, Some(&hits[0])),
            Some(Selection::Wall("wall-1".to_owned()))
        );
        assert_eq!(
            cycle_hit_selection(&hits, Some(&hits[2])),
            Some(Selection::Window("window-1".to_owned()))
        );
    }

    #[test]
    fn shift_click_adds_and_removes_individual_items() {
        let wall = Selection::Wall("wall-1".to_owned());
        let text = Selection::Text("text-1".to_owned());
        let image = Selection::Image("image-1".to_owned());
        let mut primary = Some(wall.clone());
        let mut additional = Vec::new();

        toggle_additive_selection(&mut primary, &mut additional, text.clone());
        toggle_additive_selection(&mut primary, &mut additional, image.clone());
        assert_eq!(primary, Some(wall.clone()));
        assert_eq!(additional, vec![text.clone(), image.clone()]);

        toggle_additive_selection(&mut primary, &mut additional, text);
        assert_eq!(primary, Some(wall.clone()));
        assert_eq!(additional, vec![image.clone()]);

        toggle_additive_selection(&mut primary, &mut additional, wall);
        assert_eq!(primary, Some(image));
        assert!(additional.is_empty());
    }

    #[test]
    fn select_all_skips_locked_items() {
        let mut plan = PlanFile::default();
        for (id, locked) in [("wall-free", false), ("wall-locked", true)] {
            plan.walls.push(Wall {
                id: id.to_owned(),
                x1: 0.0,
                y1: 0.0,
                x2: 100.0,
                y2: 0.0,
                thickness: 14.0,
                kind: WallKind::Interior,
                color: [52, 56, 59],
                locked,
            });
        }

        assert_eq!(
            selectable_selections(&plan),
            vec![Selection::Wall("wall-free".to_owned())]
        );
    }

    #[test]
    fn selection_box_uses_cad_direction_rules() {
        let mut plan = PlanFile::default();
        plan.walls.push(Wall {
            id: "inside".to_owned(),
            x1: 20.0,
            y1: 20.0,
            x2: 80.0,
            y2: 20.0,
            thickness: 10.0,
            kind: WallKind::Interior,
            color: [52, 56, 59],
            locked: false,
        });
        plan.walls.push(Wall {
            id: "crossing".to_owned(),
            x1: 70.0,
            y1: 60.0,
            x2: 140.0,
            y2: 60.0,
            thickness: 10.0,
            kind: WallKind::Interior,
            color: [52, 56, 59],
            locked: false,
        });

        let enclosing = DraftWall {
            start: Point::new(0.0, 0.0),
            end: Point::new(100.0, 100.0),
        };
        assert_eq!(
            selections_in_box(&plan, &enclosing),
            vec![Selection::Wall("inside".to_owned())]
        );

        let crossing = DraftWall {
            start: Point::new(100.0, 100.0),
            end: Point::new(0.0, 0.0),
        };
        assert_eq!(
            selections_in_box(&plan, &crossing),
            vec![
                Selection::Wall("inside".to_owned()),
                Selection::Wall("crossing".to_owned())
            ]
        );
    }

    #[test]
    fn rectangle_area_triangulates_to_its_complete_surface() {
        let triangles = triangulate_polygon(&[
            eframe::egui::Pos2::new(0.0, 0.0),
            eframe::egui::Pos2::new(100.0, 0.0),
            eframe::egui::Pos2::new(100.0, 80.0),
            eframe::egui::Pos2::new(0.0, 80.0),
        ]);
        let area = triangles
            .iter()
            .map(|triangle| {
                let [a, b, c] = *triangle;
                ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
            })
            .sum::<f32>();

        assert_eq!(triangles.len(), 2);
        assert!((area - 8_000.0).abs() < 0.001);
    }

    #[test]
    fn area_closing_point_is_removed_before_triangulation() {
        let points = normalized_polygon_points(&[
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 80.0),
            Point::new(0.0, 80.0),
            Point::new(0.0, 0.0),
        ]);

        assert_eq!(points.len(), 4);
        let screen_points = points
            .iter()
            .map(|point| eframe::egui::Pos2::new(point.x, point.y))
            .collect::<Vec<_>>();
        assert_eq!(triangulate_polygon(&screen_points).len(), 2);
    }

    #[test]
    fn selected_opening_is_not_moved_twice_with_its_wall() {
        let mut plan = PlanFile::default();
        plan.walls.push(Wall {
            id: "wall-1".to_owned(),
            x1: 0.0,
            y1: 0.0,
            x2: 200.0,
            y2: 0.0,
            thickness: 10.0,
            kind: WallKind::Interior,
            color: [52, 56, 59],
            locked: false,
        });
        plan.doors.push(Door {
            id: "door-1".to_owned(),
            wall_id: "wall-1".to_owned(),
            t: 0.5,
            width: 80.0,
            kind: DoorKind::Single,
            hinge: HingeSide::Left,
            swing: SwingSide::Inward,
            color: [70, 70, 70],
            locked: false,
        });
        let selections = vec![
            Selection::Wall("wall-1".to_owned()),
            Selection::Door("door-1".to_owned()),
        ];

        assert!(opening_follows_selected_wall(
            &plan,
            &selections[1],
            &selections
        ));
        assert!(!opening_follows_selected_wall(
            &plan,
            &selections[0],
            &selections
        ));
    }

    #[test]
    fn safe_save_keeps_previous_version() {
        let folder = std::env::temp_dir().join(format!("rpp-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("dossier temporaire");
        let path = folder.join("school-plan.rpp");
        std::fs::write(&path, b"previous version").expect("previous file");

        write_plan_safely(&path, b"new version").expect("save");

        assert_eq!(std::fs::read(&path).expect("new file"), b"new version");
        assert_eq!(
            std::fs::read(path.with_extension("rpp.bak")).expect("previous backup"),
            b"previous version"
        );
        assert!(!path.with_extension("rpp.tmp").exists());
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn safe_first_save_leaves_no_temporary_file() {
        let folder = std::env::temp_dir().join(format!("rpp-first-save-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("temporary folder");
        let path = folder.join("new-plan.rpp");

        write_plan_safely(&path, b"first version").expect("first save");

        assert_eq!(std::fs::read(&path).expect("saved file"), b"first version");
        assert!(!path.with_extension("rpp.tmp").exists());
        assert!(!path.with_extension("rpp.bak").exists());
        let _ = std::fs::remove_dir_all(folder);
    }

    fn audit_wall(id: &str, x1: f32, y1: f32, x2: f32, y2: f32) -> Wall {
        Wall {
            id: id.to_owned(),
            x1,
            y1,
            x2,
            y2,
            thickness: 14.0,
            kind: WallKind::Interior,
            color: [52, 56, 59],
            locked: false,
        }
    }

    #[test]
    fn locked_items_and_locked_layers_resist_delete_nudge_and_split() {
        let mut app = test_app(eframe::egui::Context::default());
        app.plan
            .walls
            .push(audit_wall("protected", 0.0, 0.0, 200.0, 0.0));
        app.selection = Some(Selection::Wall("protected".to_owned()));
        app.plan.walls[0].locked = true;
        let original = app.plan.clone();
        app.nudge_selected(Point::new(10.0, 0.0));
        app.split_selected_wall();
        app.remove_selected();
        assert_eq!(app.plan, original);
        app.plan.walls[0].locked = false;
        app.plan.layers[0].locked = true;
        let original = app.plan.clone();
        app.nudge_selected(Point::new(10.0, 0.0));
        app.split_selected_wall();
        app.remove_selected();
        assert_eq!(app.plan, original);
        assert!(app.past.is_empty());
    }

    #[test]
    fn grouped_nudge_evicts_only_one_history_entry_at_capacity() {
        let mut app = test_app(eframe::egui::Context::default());
        app.plan.walls = vec![
            audit_wall("a", 0.0, 0.0, 200.0, 0.0),
            audit_wall("b", 0.0, 50.0, 200.0, 50.0),
        ];
        app.selection = Some(Selection::Wall("a".to_owned()));
        app.additional_selections
            .push(Selection::Wall("b".to_owned()));
        for index in 0..100 {
            let mut snapshot = app.snapshot();
            snapshot.walls[0].color = [index, 0, 0];
            app.past.push(snapshot);
        }
        app.nudge_selected(Point::new(1.0, 0.0));
        assert_eq!(app.past.len(), 100);
        assert_eq!(app.past[0].walls[0].color, [1, 0, 0]);
        assert_eq!(app.plan.walls[0].x1, 1.0);
        app.undo();
        assert_eq!(app.plan.walls[0].x1, 0.0);
        assert_eq!(app.plan.walls[1].x1, 0.0);
    }

    #[test]
    fn invalid_edits_roll_back_without_changing_history() {
        let mut app = test_app(eframe::egui::Context::default());
        app.plan.walls.push(audit_wall("a", 0.0, 0.0, 200.0, 0.0));
        let before = app.snapshot();
        app.plan.walls[0].x1 = f32::NAN;
        app.commit(before);
        assert_eq!(app.plan.walls[0].x1, 0.0);
        assert!(app.past.is_empty());
        assert!(!app.dirty);
    }

    #[test]
    fn cancelling_a_drag_restores_its_preview_and_new_plan_resets_every_tool() {
        let mut app = test_app(eframe::egui::Context::default());
        app.plan.walls.push(audit_wall("a", 0.0, 0.0, 200.0, 0.0));
        let before = app.snapshot();
        app.group_drag = Some(super::GroupDrag {
            selections: vec![Selection::Wall("a".into())],
            last_pointer: Point::default(),
            before,
        });
        app.plan.walls[0].x1 = 20.0;
        app.cancel_interaction();
        assert_eq!(app.plan.walls[0].x1, 0.0);
        app.arrow_draft = Some(DraftWall {
            start: Point::default(),
            end: Point::new(20.0, 20.0),
        });
        app.room_draft.push(Point::new(1.0, 1.0));
        app.panning_canvas = true;
        app.new_plan();
        assert!(app.plan.walls.is_empty());
        assert!(app.arrow_draft.is_none());
        assert!(app.room_draft.is_empty());
        assert!(!app.panning_canvas);
        assert!(app.textures.contains_key(super::BUILTIN_EMERGENCY_EXIT_ID));
    }

    #[test]
    fn automatic_room_metadata_keeps_its_layer_after_geometry_changes() {
        let mut app = test_app(eframe::egui::Context::default());
        app.plan.walls = vec![
            audit_wall("a", 0.0, 0.0, 200.0, 0.0),
            audit_wall("b", 200.0, 0.0, 200.0, 200.0),
            audit_wall("c", 200.0, 200.0, 0.0, 200.0),
            audit_wall("d", 0.0, 200.0, 0.0, 0.0),
        ];
        app.refresh_automatic_rooms();
        app.plan.layers.push(crate::model::Layer {
            id: "rooms".into(),
            name: "Rooms".into(),
            ..crate::model::Layer::default()
        });
        app.plan.rooms[0].name = "Kitchen".into();
        let old_id = app.plan.rooms[0].id.clone();
        app.plan.assign_to_layer(&old_id, "rooms");
        for wall in &mut app.plan.walls {
            wall.x1 += 10.0;
            wall.x2 += 10.0;
        }
        app.refresh_automatic_rooms();
        assert_ne!(app.plan.rooms[0].id, old_id);
        assert_eq!(app.plan.rooms[0].name, "Kitchen");
        assert_eq!(app.plan.layer_id_for(&app.plan.rooms[0].id), "rooms");
        assert!(crate::format::validate_plan(&app.plan).is_ok());
    }

    #[test]
    fn malformed_settings_are_sanitized_and_recovery_errors_persist() {
        let settings = super::AppSettings {
            grid_size: 0.0,
            wall_thickness: f32::NAN,
            text_size: -1.0,
            ..super::AppSettings::default()
        }
        .sanitized();
        assert!(settings.grid_size > 0.0);
        assert!(settings.wall_thickness.is_finite());
        assert!(settings.text_size >= 1.0);
        let mut app = test_app(eframe::egui::Context::default());
        let directory = tempfile::tempdir().expect("test directory");
        app.recovery_path = Some(directory.path().to_path_buf());
        app.autosave();
        assert!(app.recovery_error.is_some());
        app.recovery_path = Some(directory.path().join("autosave.rpp"));
        app.autosave();
        assert!(app.recovery_error.is_none());
    }
    #[test]
    fn invalid_opening_widths_and_locked_layer_drag_do_not_panic_or_modify() {
        let mut app = test_app(eframe::egui::Context::default());
        app.plan.walls.push(audit_wall("tiny", 0.0, 0.0, 5.0, 0.0));
        assert_eq!(app.find_opening_position("tiny", 20.0, 0.5), None);
        assert!(!app.opening_available("tiny", f32::NAN, 1.0, None));
        app.selection = Some(Selection::Wall("tiny".into()));
        app.plan.layers[0].locked = true;
        assert!(app.begin_selected_drag(Point::new(0.0, 0.0)));
        assert!(app.wall_drag.is_none());
    }
    #[test]
    fn grouped_duplication_is_one_undo_step_at_capacity() {
        let mut app = test_app(eframe::egui::Context::default());
        app.plan.walls = vec![
            audit_wall("a", 0.0, 0.0, 200.0, 0.0),
            audit_wall("b", 0.0, 50.0, 200.0, 50.0),
        ];
        app.selection = Some(Selection::Wall("a".into()));
        app.additional_selections.push(Selection::Wall("b".into()));
        for index in 0..100 {
            let mut snapshot = app.snapshot();
            snapshot.walls[0].color = [index, 0, 0];
            app.past.push(snapshot);
        }
        app.duplicate_selected();
        assert_eq!(app.plan.walls.len(), 4);
        assert_eq!(app.past.len(), 100);
        assert_eq!(app.past[0].walls[0].color, [1, 0, 0]);
        assert!(!app.batching_edits);
        app.undo();
        assert_eq!(app.plan.walls.len(), 2);
    }
}
