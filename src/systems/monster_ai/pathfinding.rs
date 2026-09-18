use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::resourses::physics_resources::TILE_SIZE;
use crate::systems::terrain::{self, TerrainMap};

/// Grid-based 2D pathfinding and line-of-sight for monster AI, built directly
/// on the terrain's own tile grid (`TerrainMap`) instead of a separate
/// navmesh — the terrain module is already the single source of truth for
/// where walls are, at exactly `TILE_SIZE` resolution.
/// Snaps a world position to its tile's center (same convention `TerrainMap`
/// uses: tile key == world-space center of that tile).
pub fn world_to_tile(pos: Vec2) -> IVec2 {
    IVec2::new(
        (pos.x / TILE_SIZE).round() as i32 * TILE_SIZE as i32,
        (pos.y / TILE_SIZE).round() as i32 * TILE_SIZE as i32,
    )
}

pub fn tile_to_world(tile: IVec2) -> Vec2 {
    Vec2::new(tile.x as f32, tile.y as f32)
}

/// A tile is walkable if it's neither a wall nor water. For a tile that
/// hasn't been generated yet, falls back to `terrain::predict_tile`'s noise
/// prediction (the same noise generation itself will use) instead of
/// treating unloaded ground as flatly blocked — otherwise a monster chasing
/// toward the edge of the loaded area would simply refuse to go there.
fn walkable(map: &TerrainMap, tile: IVec2) -> bool {
    if map.is_generated(tile) {
        !map.is_wall(tile) && !map.is_water(tile)
    } else {
        let predicted = terrain::predict_tile(tile);
        !predicted.is_wall && !predicted.is_water
    }
}

/// True if there is an unobstructed straight line between `from` and `to`:
/// walks the tiles the segment passes through (supercover line) and fails as
/// soon as one of them is a wall (loaded, or predicted for an unloaded
/// tile — see `walkable`). This is deliberately independent from
/// pathfinding cost/heuristics — vision cares only about "any wall in the
/// way", not the shortest walkable route. Water does not block sight.
pub fn line_of_sight(map: &TerrainMap, from: Vec2, to: Vec2) -> bool {
    let start = world_to_tile(from);
    let end = world_to_tile(to);

    let step = TILE_SIZE as i32;
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let steps = (dx.abs() / step).max(dy.abs() / step);

    if steps == 0 {
        return true;
    }

    let step_x = dx as f32 / steps as f32;
    let step_y = dy as f32 / steps as f32;
    let mut fx = start.x as f32;
    let mut fy = start.y as f32;

    for _ in 0..=steps {
        let tile = IVec2::new(
            (fx / TILE_SIZE).round() as i32 * step,
            (fy / TILE_SIZE).round() as i32 * step,
        );
        let blocked = if map.is_generated(tile) {
            map.is_wall(tile)
        } else {
            terrain::predict_tile(tile).is_wall
        };
        if blocked {
            return false;
        }
        fx += step_x;
        fy += step_y;
    }
    true
}

const NEIGHBORS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

#[derive(Copy, Clone, PartialEq, Eq)]
struct OpenNode {
    cost: i64,
    tile: IVec2,
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; reverse so lowest cost pops first.
        other.cost.cmp(&self.cost)
    }
}
impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn heuristic(a: IVec2, b: IVec2) -> i64 {
    let dx = (a.x - b.x).unsigned_abs() as i64;
    let dy = (a.y - b.y).unsigned_abs() as i64;
    // Octile distance in tile-step units (matches the 8-directional moves below).
    let step = TILE_SIZE as i64;
    let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
    hi * step + lo * (1_414_213 * step / 1_000_000 - step)
}

/// Soft "another monster is already walking through here" cost overlay for
/// `find_path`, built from packmates' current path waypoints. Not a hard
/// block — a tile someone else claimed is still usable, just discouraged —
/// so two monsters independently routed toward nearby targets prefer
/// genuinely different corridors when one exists, without ever refusing the
/// only way through. Occasional overlap is still fine (and expected at a
/// real choke point); this only cuts down how often it happens.
pub struct PathClaims<'a> {
    pub claimed_by: &'a HashMap<IVec2, Entity>,
    pub self_entity: Entity,
    pub penalty: i64,
}

/// Finds a walkable path from `start` to `goal`, both world positions,
/// returning waypoints (tile centers, world space) from the first step after
/// `start` up to and including `goal`'s tile. Returns `None` if the goal
/// tile isn't walkable/reachable, or if the search exceeds `max_nodes`
/// (a hard cap so a single bad request can't spike a frame) — callers should
/// treat that as "no path", not retry immediately. `claims`, when given,
/// softly discourages (never forbids) routing through a tile another
/// monster's current path already occupies — see `PathClaims`.
pub fn find_path(
    map: &TerrainMap,
    start: Vec2,
    goal: Vec2,
    max_nodes: usize,
    claims: Option<&PathClaims>,
) -> Option<Vec<Vec2>> {
    let start_tile = world_to_tile(start);
    let goal_tile = world_to_tile(goal);

    if start_tile == goal_tile {
        return Some(Vec::new());
    }
    if !walkable(map, goal_tile) {
        return None;
    }

    let step = TILE_SIZE as i64;
    let diag_cost = 1_414_213 * step / 1_000_000;

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<IVec2, IVec2> = HashMap::new();
    let mut g_score: HashMap<IVec2, i64> = HashMap::new();

    g_score.insert(start_tile, 0);
    open.push(OpenNode {
        cost: heuristic(start_tile, goal_tile),
        tile: start_tile,
    });

    let mut visited = 0usize;

    while let Some(OpenNode { tile: current, .. }) = open.pop() {
        if current == goal_tile {
            return Some(reconstruct_path(&came_from, current));
        }
        visited += 1;
        if visited > max_nodes {
            return None;
        }

        let current_g = *g_score.get(&current).unwrap_or(&i64::MAX);

        for (dx, dy) in NEIGHBORS {
            let neighbor = IVec2::new(current.x + dx * step as i32, current.y + dy * step as i32);
            if !walkable(map, neighbor) {
                continue;
            }
            // Don't let the path cut across a diagonal wall corner.
            if dx != 0 && dy != 0 {
                let side_a = IVec2::new(current.x + dx * step as i32, current.y);
                let side_b = IVec2::new(current.x, current.y + dy * step as i32);
                if !walkable(map, side_a) || !walkable(map, side_b) {
                    continue;
                }
            }

            let mut move_cost = if dx != 0 && dy != 0 { diag_cost } else { step };
            if let Some(claims) = claims {
                let claimed_by_other = claims
                    .claimed_by
                    .get(&neighbor)
                    .is_some_and(|&owner| owner != claims.self_entity);
                if claimed_by_other {
                    move_cost = move_cost.saturating_add(claims.penalty);
                }
            }
            let tentative_g = current_g.saturating_add(move_cost);

            if tentative_g < *g_score.get(&neighbor).unwrap_or(&i64::MAX) {
                came_from.insert(neighbor, current);
                g_score.insert(neighbor, tentative_g);
                open.push(OpenNode {
                    cost: tentative_g.saturating_add(heuristic(neighbor, goal_tile)),
                    tile: neighbor,
                });
            }
        }
    }

    None
}

fn reconstruct_path(came_from: &HashMap<IVec2, IVec2>, mut current: IVec2) -> Vec<Vec2> {
    let mut path = vec![tile_to_world(current)];
    while let Some(&prev) = came_from.get(&current) {
        current = prev;
        path.push(tile_to_world(current));
    }
    path.pop(); // drop the start tile itself
    path.reverse();
    path
}
