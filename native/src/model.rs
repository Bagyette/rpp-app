use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub id: String,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    #[serde(default = "default_wall_thickness")]
    pub thickness: f32,
    #[serde(default)]
    pub kind: WallKind,
    #[serde(default = "default_wall_color")]
    pub color: [u8; 3],
    #[serde(default)]
    pub locked: bool,
}

pub fn default_wall_color() -> [u8; 3] {
    [52, 56, 59]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanLine {
    pub id: String,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    #[serde(default = "default_line_color")]
    pub color: [u8; 3],
    #[serde(default)]
    pub locked: bool,
}

pub fn default_line_color() -> [u8; 3] {
    [75, 75, 75]
}

impl PlanLine {
    pub fn start(&self) -> Point {
        Point::new(self.x1, self.y1)
    }

    pub fn end(&self) -> Point {
        Point::new(self.x2, self.y2)
    }

    pub fn length(&self) -> f32 {
        self.start().distance(self.end())
    }
}

impl Wall {
    pub fn start(&self) -> Point {
        Point::new(self.x1, self.y1)
    }

    pub fn end(&self) -> Point {
        Point::new(self.x2, self.y2)
    }

    pub fn length(&self) -> f32 {
        self.start().distance(self.end())
    }

    pub fn point_at(&self, t: f32) -> Point {
        self.start().lerp(self.end(), t)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WallKind {
    #[default]
    Interior,
    Exterior,
}

impl WallKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interior => "interior",
            Self::Exterior => "exterior",
        }
    }
}

pub fn default_wall_thickness() -> f32 {
    14.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Door {
    pub id: String,
    #[serde(rename = "wallId")]
    pub wall_id: String,
    pub t: f32,
    pub width: f32,
    #[serde(default)]
    pub kind: DoorKind,
    #[serde(default)]
    pub hinge: HingeSide,
    #[serde(default)]
    pub swing: SwingSide,
    #[serde(default = "default_opening_color")]
    pub color: [u8; 3],
    #[serde(default)]
    pub locked: bool,
}

pub fn default_opening_color() -> [u8; 3] {
    [70, 70, 70]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub id: String,
    #[serde(rename = "wallId")]
    pub wall_id: String,
    pub t: f32,
    pub width: f32,
    #[serde(default)]
    pub kind: WindowKind,
    #[serde(default = "default_window_color")]
    pub color: [u8; 3],
    #[serde(default)]
    pub locked: bool,
}

pub fn default_window_color() -> [u8; 3] {
    [45, 115, 145]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowKind {
    #[default]
    Standard,
    Bay,
}

impl WindowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Bay => "bay",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoorKind {
    #[default]
    Single,
    Double,
    Sliding,
}

impl DoorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Double => "double",
            Self::Sliding => "sliding",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HingeSide {
    #[default]
    Left,
    Right,
}

impl HingeSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwingSide {
    #[default]
    Inward,
    Outward,
}

impl SwingSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inward => "inward",
            Self::Outward => "outward",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanFile {
    pub version: u32,
    pub walls: Vec<Wall>,
    #[serde(default)]
    pub lines: Vec<PlanLine>,
    pub doors: Vec<Door>,
    #[serde(default)]
    pub windows: Vec<Window>,
    #[serde(default)]
    pub objects: Vec<PlanObject>,
    #[serde(default)]
    pub images: Vec<ImageObject>,
    #[serde(default)]
    pub texts: Vec<TextObject>,
    #[serde(default)]
    pub areas: Vec<AreaShape>,
    #[serde(default)]
    pub compasses: Vec<CompassRose>,
    #[serde(default)]
    pub arrows: Vec<PlanArrow>,
    #[serde(default)]
    pub rooms: Vec<Room>,
    #[serde(default = "default_layers")]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub layer_assignments: Vec<LayerAssignment>,
}

impl Default for PlanFile {
    fn default() -> Self {
        Self {
            version: 1,
            walls: Vec::new(),
            lines: Vec::new(),
            doors: Vec::new(),
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

impl PlanFile {
    pub fn layer_id_for(&self, item_id: &str) -> &str {
        self.layer_assignments
            .iter()
            .find(|assignment| assignment.item_id == item_id)
            .map(|assignment| assignment.layer_id.as_str())
            .unwrap_or(DEFAULT_LAYER_ID)
    }

    pub fn layer_for(&self, item_id: &str) -> Option<&Layer> {
        let layer_id = self.layer_id_for(item_id);
        self.layers.iter().find(|layer| layer.id == layer_id)
    }

    pub fn item_visible(&self, item_id: &str) -> bool {
        self.layer_for(item_id).is_none_or(|layer| layer.visible)
    }

    pub fn item_editable(&self, item_id: &str) -> bool {
        self.layer_for(item_id)
            .is_none_or(|layer| layer.visible && !layer.locked)
    }

    pub fn assign_to_layer(&mut self, item_id: &str, layer_id: &str) {
        self.layer_assignments
            .retain(|assignment| assignment.item_id != item_id);
        if layer_id != DEFAULT_LAYER_ID {
            self.layer_assignments.push(LayerAssignment {
                item_id: item_id.to_owned(),
                layer_id: layer_id.to_owned(),
            });
        }
    }

    pub fn prune_layer_assignments(&mut self) {
        let ids: HashSet<&str> = self
            .walls
            .iter()
            .map(|item| item.id.as_str())
            .chain(self.lines.iter().map(|item| item.id.as_str()))
            .chain(self.doors.iter().map(|item| item.id.as_str()))
            .chain(self.windows.iter().map(|item| item.id.as_str()))
            .chain(self.objects.iter().map(|item| item.id.as_str()))
            .chain(self.images.iter().map(|item| item.id.as_str()))
            .chain(self.texts.iter().map(|item| item.id.as_str()))
            .chain(self.areas.iter().map(|item| item.id.as_str()))
            .chain(self.rooms.iter().map(|item| item.id.as_str()))
            .chain(self.compasses.iter().map(|item| item.id.as_str()))
            .chain(self.arrows.iter().map(|item| item.id.as_str()))
            .collect();
        self.layer_assignments
            .retain(|assignment| ids.contains(assignment.item_id.as_str()));
    }

    pub fn remove_layer_assignment(&mut self, item_id: &str) {
        self.layer_assignments
            .retain(|assignment| assignment.item_id != item_id);
    }
}

/// Per-render/export index; avoids repeated linear scans of all layer assignments.
pub struct LayerLookup<'a> {
    assignments: HashMap<&'a str, &'a str>,
    layers: HashMap<&'a str, &'a Layer>,
}

impl<'a> LayerLookup<'a> {
    pub fn new(plan: &'a PlanFile) -> Self {
        Self {
            assignments: plan
                .layer_assignments
                .iter()
                .map(|assignment| (assignment.item_id.as_str(), assignment.layer_id.as_str()))
                .collect(),
            layers: plan
                .layers
                .iter()
                .map(|layer| (layer.id.as_str(), layer))
                .collect(),
        }
    }

    pub fn layer_id_for(&self, item_id: &str) -> &str {
        self.assignments
            .get(item_id)
            .copied()
            .unwrap_or(DEFAULT_LAYER_ID)
    }

    pub fn item_visible(&self, item_id: &str) -> bool {
        self.layers
            .get(self.layer_id_for(item_id))
            .is_none_or(|layer| layer.visible)
    }

    pub fn item_opacity(&self, item_id: &str) -> f32 {
        self.layers
            .get(self.layer_id_for(item_id))
            .map_or(1.0, |layer| layer.opacity.clamp(0.0, 1.0))
    }
}

pub const DEFAULT_LAYER_ID: &str = "default";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "default_layer_opacity")]
    pub opacity: f32,
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            id: DEFAULT_LAYER_ID.to_owned(),
            name: "Default".to_owned(),
            visible: true,
            locked: false,
            opacity: 1.0,
        }
    }
}

pub fn default_layers() -> Vec<Layer> {
    vec![Layer::default()]
}

fn default_true() -> bool {
    true
}

fn default_layer_opacity() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerAssignment {
    pub item_id: String,
    pub layer_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoomSource {
    #[default]
    Automatic,
    Manual,
}

impl RoomSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoomKind {
    #[default]
    Custom,
    LivingRoom,
    Bedroom,
    Kitchen,
    Bathroom,
    Hallway,
    Office,
    Garage,
    Storage,
}

impl RoomKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Custom => "custom",
            Self::LivingRoom => "living-room",
            Self::Bedroom => "bedroom",
            Self::Kitchen => "kitchen",
            Self::Bathroom => "bathroom",
            Self::Hallway => "hallway",
            Self::Office => "office",
            Self::Garage => "garage",
            Self::Storage => "storage",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: RoomKind,
    #[serde(default)]
    pub source: RoomSource,
    pub points: Vec<Point>,
    #[serde(default = "default_room_fill_color")]
    pub fill_color: [u8; 3],
    #[serde(default = "default_true")]
    pub fill_visible: bool,
    #[serde(default)]
    pub locked: bool,
}

impl Room {
    pub fn area_square_centimeters(&self) -> f32 {
        polygon_area(&self.points)
    }

    pub fn perimeter_centimeters(&self) -> f32 {
        if self.points.len() < 2 {
            return 0.0;
        }
        self.points
            .iter()
            .zip(self.points.iter().cycle().skip(1))
            .take(self.points.len())
            .map(|(a, b)| a.distance(*b))
            .sum()
    }
}

pub fn default_room_fill_color() -> [u8; 3] {
    [220, 230, 239]
}

pub fn polygon_area(points: &[Point]) -> f32 {
    let Some(origin) = points.first() else {
        return 0.0;
    };
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| {
            let ax = f64::from(a.x) - f64::from(origin.x);
            let ay = f64::from(a.y) - f64::from(origin.y);
            let bx = f64::from(b.x) - f64::from(origin.x);
            let by = f64::from(b.y) - f64::from(origin.y);
            ax * by - bx * ay
        })
        .sum::<f64>()
        .abs() as f32
        * 0.5
}

pub fn truncate_text(value: &mut String, maximum_bytes: usize) {
    if value.len() <= maximum_bytes {
        return;
    }
    let mut boundary = maximum_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanObject {
    pub id: String,
    #[serde(default)]
    pub kind: ObjectKind,
    #[serde(default)]
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default = "default_object_fill_color")]
    pub fill_color: [u8; 3],
    #[serde(default = "default_object_stroke_color")]
    pub stroke_color: [u8; 3],
    #[serde(default)]
    pub locked: bool,
}

impl PlanObject {
    pub fn display_name<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.name.trim().is_empty() {
            fallback
        } else {
            self.name.trim()
        }
    }
}

pub fn default_object_fill_color() -> [u8; 3] {
    [225, 228, 226]
}

pub fn default_object_stroke_color() -> [u8; 3] {
    [90, 90, 90]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObjectKind {
    #[default]
    Rectangle,
    Bed,
    Table,
    Sofa,
    Wardrobe,
    Desk,
}

impl ObjectKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            Self::Bed => "bed",
            Self::Table => "table",
            Self::Sofa => "sofa",
            Self::Wardrobe => "wardrobe",
            Self::Desk => "desk",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageObject {
    pub id: String,
    pub asset_id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextObject {
    pub id: String,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default = "default_text_color")]
    pub color: [u8; 3],
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaShape {
    pub id: String,
    pub points: Vec<Point>,
    #[serde(default = "default_area_fill_color")]
    pub fill_color: [u8; 3],
    #[serde(default = "default_area_stroke_color")]
    pub stroke_color: [u8; 3],
    #[serde(default)]
    pub locked: bool,
}

impl AreaShape {
    pub fn area_square_centimeters(&self) -> f32 {
        polygon_area(&self.points)
    }
}

pub fn default_area_fill_color() -> [u8; 3] {
    [214, 230, 210]
}

pub fn default_area_stroke_color() -> [u8; 3] {
    [70, 110, 70]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompassRose {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub rotation: f32,
    #[serde(default = "default_compass_color")]
    pub color: [u8; 3],
    #[serde(default)]
    pub locked: bool,
}

pub fn default_compass_color() -> [u8; 3] {
    [52, 56, 59]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanArrow {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub length: f32,
    pub width: f32,
    #[serde(default)]
    pub style: ArrowStyle,
    #[serde(default)]
    pub orientation: u8,
    #[serde(default = "default_arrow_color")]
    pub color: [u8; 3],
    #[serde(default)]
    pub locked: bool,
}

impl PlanArrow {
    pub const ORIENTATION_COUNT: u8 = 64;
    pub const DEGREES_PER_STEP: f32 = 360.0 / Self::ORIENTATION_COUNT as f32;

    pub fn angle_degrees(&self) -> f32 {
        self.orientation as f32 * Self::DEGREES_PER_STEP
    }

    pub fn end(&self) -> Point {
        let angle = self.angle_degrees().to_radians();
        Point::new(
            self.x + self.length * angle.cos(),
            self.y + self.length * angle.sin(),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrowStyle {
    #[default]
    Open,
    OpenBold,
    Hollow,
    Solid,
}

impl ArrowStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::OpenBold => "open-bold",
            Self::Hollow => "hollow",
            Self::Solid => "solid",
        }
    }
}

pub fn default_arrow_color() -> [u8; 3] {
    [52, 56, 59]
}

pub fn default_text_color() -> [u8; 3] {
    [52, 56, 59]
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectAsset {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub walls: Vec<Wall>,
    pub lines: Vec<PlanLine>,
    pub doors: Vec<Door>,
    pub windows: Vec<Window>,
    pub objects: Vec<PlanObject>,
    pub images: Vec<ImageObject>,
    pub texts: Vec<TextObject>,
    pub areas: Vec<AreaShape>,
    pub compasses: Vec<CompassRose>,
    pub arrows: Vec<PlanArrow>,
    pub rooms: Vec<Room>,
    pub layers: Vec<Layer>,
    pub layer_assignments: Vec<LayerAssignment>,
    pub assets: Arc<Vec<ProjectAsset>>,
}

impl Snapshot {
    pub fn from_project(plan: &PlanFile, assets: &Arc<Vec<ProjectAsset>>) -> Self {
        Self {
            walls: plan.walls.clone(),
            lines: plan.lines.clone(),
            doors: plan.doors.clone(),
            windows: plan.windows.clone(),
            objects: plan.objects.clone(),
            images: plan.images.clone(),
            texts: plan.texts.clone(),
            areas: plan.areas.clone(),
            compasses: plan.compasses.clone(),
            arrows: plan.arrows.clone(),
            rooms: plan.rooms.clone(),
            layers: plan.layers.clone(),
            layer_assignments: plan.layer_assignments.clone(),
            assets: Arc::clone(assets),
        }
    }

    pub fn matches_project(&self, plan: &PlanFile, assets: &Arc<Vec<ProjectAsset>>) -> bool {
        (Arc::ptr_eq(&self.assets, assets) || self.assets == *assets)
            && self.walls == plan.walls
            && self.lines == plan.lines
            && self.doors == plan.doors
            && self.windows == plan.windows
            && self.objects == plan.objects
            && self.images == plan.images
            && self.texts == plan.texts
            && self.areas == plan.areas
            && self.compasses == plan.compasses
            && self.arrows == plan.arrows
            && self.rooms == plan.rooms
            && self.layers == plan.layers
            && self.layer_assignments == plan.layer_assignments
    }

    /// Owned allocations retained by an undo entry; image bytes are counted separately.
    pub fn memory_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>();
        bytes += self.walls.capacity() * std::mem::size_of::<Wall>();
        bytes += self
            .walls
            .iter()
            .map(|item| item.id.capacity())
            .sum::<usize>();
        bytes += self.lines.capacity() * std::mem::size_of::<PlanLine>();
        bytes += self
            .lines
            .iter()
            .map(|item| item.id.capacity())
            .sum::<usize>();
        bytes += self.doors.capacity() * std::mem::size_of::<Door>();
        bytes += self
            .doors
            .iter()
            .map(|item| item.id.capacity() + item.wall_id.capacity())
            .sum::<usize>();
        bytes += self.windows.capacity() * std::mem::size_of::<Window>();
        bytes += self
            .windows
            .iter()
            .map(|item| item.id.capacity() + item.wall_id.capacity())
            .sum::<usize>();
        bytes += self.objects.capacity() * std::mem::size_of::<PlanObject>();
        bytes += self
            .objects
            .iter()
            .map(|item| item.id.capacity() + item.name.capacity())
            .sum::<usize>();
        bytes += self.images.capacity() * std::mem::size_of::<ImageObject>();
        bytes += self
            .images
            .iter()
            .map(|item| item.id.capacity() + item.asset_id.capacity())
            .sum::<usize>();
        bytes += self.texts.capacity() * std::mem::size_of::<TextObject>();
        bytes += self
            .texts
            .iter()
            .map(|item| item.id.capacity() + item.text.capacity())
            .sum::<usize>();
        bytes += self.areas.capacity() * std::mem::size_of::<AreaShape>();
        bytes += self
            .areas
            .iter()
            .map(|item| item.id.capacity() + item.points.capacity() * std::mem::size_of::<Point>())
            .sum::<usize>();
        bytes += self.compasses.capacity() * std::mem::size_of::<CompassRose>();
        bytes += self
            .compasses
            .iter()
            .map(|item| item.id.capacity())
            .sum::<usize>();
        bytes += self.arrows.capacity() * std::mem::size_of::<PlanArrow>();
        bytes += self
            .arrows
            .iter()
            .map(|item| item.id.capacity())
            .sum::<usize>();
        bytes += self.rooms.capacity() * std::mem::size_of::<Room>();
        bytes += self
            .rooms
            .iter()
            .map(|item| {
                item.id.capacity()
                    + item.name.capacity()
                    + item.points.capacity() * std::mem::size_of::<Point>()
            })
            .sum::<usize>();
        bytes += self.layers.capacity() * std::mem::size_of::<Layer>();
        bytes += self
            .layers
            .iter()
            .map(|item| item.id.capacity() + item.name.capacity())
            .sum::<usize>();
        bytes += self.layer_assignments.capacity() * std::mem::size_of::<LayerAssignment>();
        bytes += self
            .layer_assignments
            .iter()
            .map(|item| item.item_id.capacity() + item.layer_id.capacity())
            .sum::<usize>();
        bytes
    }

    pub fn restore(self, plan: &mut PlanFile, assets: &mut Arc<Vec<ProjectAsset>>) {
        plan.walls = self.walls;
        plan.lines = self.lines;
        plan.doors = self.doors;
        plan.windows = self.windows;
        plan.objects = self.objects;
        plan.images = self.images;
        plan.texts = self.texts;
        plan.areas = self.areas;
        plan.compasses = self.compasses;
        plan.arrows = self.arrows;
        plan.rooms = self.rooms;
        plan.layers = self.layers;
        plan.layer_assignments = self.layer_assignments;
        *assets = self.assets;
    }
}

#[cfg(test)]
mod audit_tests {
    use super::{Point, polygon_area, truncate_text};

    #[test]
    fn polygon_area_does_not_cancel_at_large_offsets() {
        let points = [
            Point::new(10_000_000.0, 10_000_000.0),
            Point::new(10_001_000.0, 10_000_000.0),
            Point::new(10_001_000.0, 10_001_000.0),
            Point::new(10_000_000.0, 10_001_000.0),
        ];
        assert_eq!(polygon_area(&points), 1_000_000.0);
    }

    #[test]
    fn text_limits_preserve_utf8_boundaries() {
        let mut text = "é😄é".to_owned();
        truncate_text(&mut text, 5);
        assert_eq!(text, "é");
    }
}
