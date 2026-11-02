use std::collections::{HashMap, HashSet};

use crate::model::{Point, Wall};

const GEOMETRY_EPSILON: f32 = 0.01;
const NODE_MERGE_EPSILON: f32 = 0.5;

#[derive(Clone, Copy)]
struct Segment {
    start: Point,
    end: Point,
}

/// Finds the bounded faces formed by wall center lines.
///
/// Intersections and T-junctions are split before the planar graph is walked,
/// so adjacent rooms can share a wall without producing one large room.
#[derive(Debug)]
pub struct ComplexityLimit;

pub fn detect_room_polygons(walls: &[Wall]) -> Result<Vec<Vec<Point>>, ComplexityLimit> {
    if walls.len() > 4096 {
        return Err(ComplexityLimit);
    }
    let segments = split_at_intersections(walls)?;
    let (nodes, edges) = build_graph(&segments);
    if edges.is_empty() {
        return Ok(Vec::new());
    }

    let mut neighbors = vec![Vec::<usize>::new(); nodes.len()];
    for &(a, b) in &edges {
        neighbors[a].push(b);
        neighbors[b].push(a);
    }
    for (index, list) in neighbors.iter_mut().enumerate() {
        list.sort_by(|a, b| {
            angle(nodes[index], nodes[*a]).total_cmp(&angle(nodes[index], nodes[*b]))
        });
        list.dedup();
    }

    let mut visited = HashSet::<(usize, usize)>::new();
    let mut rooms = Vec::new();
    for &(a, b) in &edges {
        for (start, next) in [(a, b), (b, a)] {
            if visited.contains(&(start, next)) {
                continue;
            }
            let Some(face) = trace_face(start, next, &neighbors, &nodes, &mut visited) else {
                continue;
            };
            let signed = signed_area(&face);
            if signed > GEOMETRY_EPSILON && face.len() >= 3 {
                rooms.push(simplify_collinear(face));
            }
        }
    }
    rooms.retain(|points| points.len() >= 3 && signed_area(points) > GEOMETRY_EPSILON);
    rooms.sort_by(|a, b| {
        polygon_center(a)
            .y
            .total_cmp(&polygon_center(b).y)
            .then_with(|| polygon_center(a).x.total_cmp(&polygon_center(b).x))
    });
    Ok(rooms)
}

pub fn polygon_center(points: &[Point]) -> Point {
    let Some(origin) = points.first() else {
        return Point::default();
    };
    let mut signed_twice_area = 0.0_f64;
    let mut x = 0.0_f64;
    let mut y = 0.0_f64;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        let ax = f64::from(a.x) - f64::from(origin.x);
        let ay = f64::from(a.y) - f64::from(origin.y);
        let bx = f64::from(b.x) - f64::from(origin.x);
        let by = f64::from(b.y) - f64::from(origin.y);
        let cross = ax * by - bx * ay;
        signed_twice_area += cross;
        x += (ax + bx) * cross;
        y += (ay + by) * cross;
    }
    if signed_twice_area.abs() <= f64::from(GEOMETRY_EPSILON) {
        return Point::new(
            (points.iter().map(|point| f64::from(point.x)).sum::<f64>() / points.len() as f64)
                as f32,
            (points.iter().map(|point| f64::from(point.y)).sum::<f64>() / points.len() as f64)
                as f32,
        );
    }
    Point::new(
        (f64::from(origin.x) + x / (3.0 * signed_twice_area)) as f32,
        (f64::from(origin.y) + y / (3.0 * signed_twice_area)) as f32,
    )
}

pub fn point_in_polygon(point: Point, polygon: &[Point]) -> bool {
    let mut inside = false;
    for (a, b) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
    {
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
    }
    inside
}

pub fn stable_room_id(points: &[Point]) -> String {
    let mut values = points
        .iter()
        .map(|point| {
            (
                (point.x * 10.0).round() as i64,
                (point.y * 10.0).round() as i64,
            )
        })
        .collect::<Vec<_>>();
    if let Some((index, _)) = values.iter().enumerate().min_by_key(|(_, value)| **value) {
        values.rotate_left(index);
    }
    let mut hash = 0xcbf29ce484222325_u64;
    for (x, y) in values {
        for byte in x.to_le_bytes().into_iter().chain(y.to_le_bytes()) {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("room-auto-{hash:016x}")
}

fn split_at_intersections(walls: &[Wall]) -> Result<Vec<Segment>, ComplexityLimit> {
    let base = walls
        .iter()
        .filter(|wall| wall.length() > GEOMETRY_EPSILON)
        .map(|wall| Segment {
            start: wall.start(),
            end: wall.end(),
        })
        .collect::<Vec<_>>();
    let mut parameters = vec![vec![0.0_f32, 1.0]; base.len()];
    let mut candidate_count = 0;
    for first in 0..base.len() {
        for second in first + 1..base.len() {
            let a = base[first];
            let b = base[second];
            if a.start.x.min(a.end.x) > b.start.x.max(b.end.x) + NODE_MERGE_EPSILON
                || b.start.x.min(b.end.x) > a.start.x.max(a.end.x) + NODE_MERGE_EPSILON
                || a.start.y.min(a.end.y) > b.start.y.max(b.end.y) + NODE_MERGE_EPSILON
                || b.start.y.min(b.end.y) > a.start.y.max(a.end.y) + NODE_MERGE_EPSILON
            {
                continue;
            }
            candidate_count += 1;
            if candidate_count > 200_000 {
                return Err(ComplexityLimit);
            }
            if let Some((a, b)) = segment_intersection(base[first], base[second]) {
                parameters[first].push(a);
                parameters[second].push(b);
            }
        }
    }
    let mut result = Vec::new();
    for (segment, values) in base.into_iter().zip(parameters.iter_mut()) {
        values.sort_by(f32::total_cmp);
        values.dedup_by(|a, b| (*a - *b).abs() <= GEOMETRY_EPSILON);
        for pair in values.windows(2) {
            let start = segment.start.lerp(segment.end, pair[0]);
            let end = segment.start.lerp(segment.end, pair[1]);
            if start.distance(end) > GEOMETRY_EPSILON {
                if result.len() >= 10_000 {
                    return Err(ComplexityLimit);
                }
                result.push(Segment { start, end });
            }
        }
    }
    Ok(result)
}

fn segment_intersection(first: Segment, second: Segment) -> Option<(f32, f32)> {
    let r = Point::new(first.end.x - first.start.x, first.end.y - first.start.y);
    let s = Point::new(second.end.x - second.start.x, second.end.y - second.start.y);
    let denominator = cross(r, s);
    if denominator.abs() <= GEOMETRY_EPSILON {
        return None;
    }
    let offset = Point::new(
        second.start.x - first.start.x,
        second.start.y - first.start.y,
    );
    let t = cross(offset, s) / denominator;
    let u = cross(offset, r) / denominator;
    ((-GEOMETRY_EPSILON..=1.0 + GEOMETRY_EPSILON).contains(&t)
        && (-GEOMETRY_EPSILON..=1.0 + GEOMETRY_EPSILON).contains(&u))
    .then_some((t.clamp(0.0, 1.0), u.clamp(0.0, 1.0)))
}

fn build_graph(segments: &[Segment]) -> (Vec<Point>, Vec<(usize, usize)>) {
    let mut nodes = Vec::<Point>::new();
    let mut unique_edges = HashSet::<(usize, usize)>::new();
    let mut buckets = HashMap::<(i64, i64), Vec<usize>>::new();
    for segment in segments {
        let a = node_for(segment.start, &mut nodes, &mut buckets);
        let b = node_for(segment.end, &mut nodes, &mut buckets);
        if a != b {
            unique_edges.insert(if a < b { (a, b) } else { (b, a) });
        }
    }
    (nodes, unique_edges.into_iter().collect())
}

fn node_for(
    point: Point,
    nodes: &mut Vec<Point>,
    buckets: &mut HashMap<(i64, i64), Vec<usize>>,
) -> usize {
    let cell = (
        (point.x / NODE_MERGE_EPSILON).floor() as i64,
        (point.y / NODE_MERGE_EPSILON).floor() as i64,
    );
    let mut matching = None;
    for x in cell.0 - 1..=cell.0 + 1 {
        for y in cell.1 - 1..=cell.1 + 1 {
            if let Some(indices) = buckets.get(&(x, y)) {
                for &index in indices {
                    if nodes[index].distance(point) <= NODE_MERGE_EPSILON {
                        matching = Some(matching.map_or(index, |old: usize| old.min(index)));
                    }
                }
            }
        }
    }
    if let Some(index) = matching {
        return index;
    }
    let index = nodes.len();
    nodes.push(point);
    buckets.entry(cell).or_default().push(index);
    index
}

fn trace_face(
    start: usize,
    next: usize,
    neighbors: &[Vec<usize>],
    nodes: &[Point],
    visited: &mut HashSet<(usize, usize)>,
) -> Option<Vec<Point>> {
    let mut polygon = Vec::new();
    let mut previous = start;
    let mut current = next;
    for _ in 0..=visited.len() + neighbors.len() * 2 + 4 {
        if !visited.insert((previous, current)) {
            return ((previous, current) == (start, next)).then_some(polygon);
        }
        polygon.push(nodes[previous]);
        let list = &neighbors[current];
        let incoming = list.iter().position(|candidate| *candidate == previous)?;
        let following = list[(incoming + list.len() - 1) % list.len()];
        previous = current;
        current = following;
        if (previous, current) == (start, next) {
            return Some(polygon);
        }
    }
    None
}

fn simplify_collinear(points: Vec<Point>) -> Vec<Point> {
    if points.len() <= 3 {
        return points;
    }
    let mut result = Vec::new();
    for index in 0..points.len() {
        let previous = points[(index + points.len() - 1) % points.len()];
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        let a = Point::new(current.x - previous.x, current.y - previous.y);
        let b = Point::new(next.x - current.x, next.y - current.y);
        if cross(a, b).abs() > GEOMETRY_EPSILON {
            result.push(current);
        }
    }
    result
}

fn signed_area(points: &[Point]) -> f32 {
    let Some(origin) = points.first() else {
        return 0.0;
    };
    (points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| {
            (f64::from(a.x) - f64::from(origin.x)) * (f64::from(b.y) - f64::from(origin.y))
                - (f64::from(b.x) - f64::from(origin.x)) * (f64::from(a.y) - f64::from(origin.y))
        })
        .sum::<f64>()
        * 0.5) as f32
}

fn angle(origin: Point, target: Point) -> f32 {
    (target.y - origin.y).atan2(target.x - origin.x)
}

fn cross(a: Point, b: Point) -> f32 {
    a.x * b.y - a.y * b.x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{WallKind, default_wall_color};

    fn wall(id: &str, x1: f32, y1: f32, x2: f32, y2: f32) -> Wall {
        Wall {
            id: id.to_owned(),
            x1,
            y1,
            x2,
            y2,
            thickness: 14.0,
            kind: WallKind::Interior,
            color: default_wall_color(),
            locked: false,
        }
    }

    #[test]
    fn detects_one_rectangular_room() {
        let rooms = detect_room_polygons(&[
            wall("a", 0.0, 0.0, 400.0, 0.0),
            wall("b", 400.0, 0.0, 400.0, 300.0),
            wall("c", 400.0, 300.0, 0.0, 300.0),
            wall("d", 0.0, 300.0, 0.0, 0.0),
        ])
        .expect("small wall graph");
        assert_eq!(rooms.len(), 1);
        assert!((signed_area(&rooms[0]) - 120_000.0).abs() < 0.1);
    }

    #[test]
    fn shared_wall_creates_two_rooms() {
        let rooms = detect_room_polygons(&[
            wall("top", 0.0, 0.0, 400.0, 0.0),
            wall("right", 400.0, 0.0, 400.0, 200.0),
            wall("bottom", 400.0, 200.0, 0.0, 200.0),
            wall("left", 0.0, 200.0, 0.0, 0.0),
            wall("middle", 200.0, 0.0, 200.0, 200.0),
        ])
        .expect("small wall graph");
        assert_eq!(rooms.len(), 2);
        assert!(
            rooms
                .iter()
                .all(|room| (signed_area(room) - 40_000.0).abs() < 0.1)
        );
    }

    #[test]
    fn open_outline_is_not_a_room() {
        let rooms = detect_room_polygons(&[
            wall("a", 0.0, 0.0, 100.0, 0.0),
            wall("b", 100.0, 0.0, 100.0, 100.0),
            wall("c", 100.0, 100.0, 0.0, 100.0),
        ])
        .expect("small wall graph");
        assert!(rooms.is_empty());
    }

    #[test]
    fn pathological_graphs_are_bounded_and_large_coordinates_keep_centroids() {
        let walls = vec![wall("same", 0.0, 0.0, 100.0, 100.0); 4097];
        assert!(detect_room_polygons(&walls).is_err());
        let points = [
            Point::new(10_000_000.0, 10_000_000.0),
            Point::new(10_001_000.0, 10_000_000.0),
            Point::new(10_001_000.0, 10_001_000.0),
            Point::new(10_000_000.0, 10_001_000.0),
        ];
        assert_eq!(
            polygon_center(&points),
            Point::new(10_000_500.0, 10_000_500.0)
        );
        assert_eq!(signed_area(&points), 1_000_000.0);
    }
}
