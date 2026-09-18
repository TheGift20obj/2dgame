use bevy::prelude::*;

use super::difficulty::MonsterSenseConfig;
use super::pathfinding::line_of_sight;
use super::state::MonsterState;
use crate::systems::terrain::TerrainMap;

/// Published by the player each frame with its current position and whether
/// it's making walk/run noise. Monsters never read the player's transform
/// directly for hearing — only this, so hearing stays a distinct, weaker
/// channel of information than vision (see `perception::hear_player`).
#[derive(Resource, Default)]
pub struct PlayerNoise {
    pub position: Vec2,
    pub is_running: bool,
    /// False while the player isn't actually moving (no footstep noise at all).
    pub moving: bool,
}

/// A monster's short-term knowledge of the world: what it currently believes
/// about the player, and its navigation state. Everything here is
/// information the monster actually "received" (a sighting, a sound) — never
/// the player's live position for free. Kept as a sibling component to the
/// existing `MonsterAI` (health/attack-animation bookkeeping) rather than
/// merged into it, so nothing already reading `MonsterAI` elsewhere needs to
/// change.
#[derive(Component)]
pub struct MonsterPerception {
    pub state: MonsterState,
    /// Last nonzero movement direction; used as the vision cone's axis since
    /// the sprite itself only ever flips left/right.
    pub facing: Vec2,

    pub last_seen_pos: Option<Vec2>,
    pub last_seen_time: f32,
    pub last_seen_velocity: Vec2,

    pub last_heard_pos: Option<Vec2>,
    pub last_heard_time: f32,

    /// Whether the monster is currently outside the player's light and
    /// line-of-sight to it — cached from the throttled sense pass rather
    /// than recomputed every frame, since movement reads it every frame.
    pub in_darkness: bool,

    /// Offset added to a live Chase target, recomputed each throttled sense
    /// pass — zero for the pack's current "primary" pursuer, a distance-
    /// scaled sideways nudge for every other packmate currently also
    /// directly engaged, so a pack with eyes on the player still spreads out
    /// instead of funneling through the same gap as one blob. Separate from
    /// `investigate_subtarget`, which already carries the analogous offset
    /// for a monster reacting to *relayed* (not personally sensed) info.
    pub pack_flank_offset: Vec2,

    pub investigate_target: Option<Vec2>,
    pub investigate_started: f32,
    pub investigate_leg: u8,
    /// Current "search around the target" sub-point the monster is walking
    /// to, chosen fresh each leg via `state::pick_search_point`. Kept
    /// separate from `investigate_target` (the fixed anchor a leg is picked
    /// around) so the monster commits to one point instead of re-rolling
    /// every frame.
    pub investigate_subtarget: Option<Vec2>,

    /// Waypoint queue (world space, tile centers) for the current path, and
    /// which target position it was last computed for.
    pub path: Vec<Vec2>,
    pub path_index: usize,
    pub path_computed_for: Vec2,
    /// False when the last pathfinding attempt for `path_computed_for` found
    /// no route at all (as opposed to an empty path meaning "already
    /// there") — lets movement stand still instead of walking straight at
    /// an unreachable point.
    pub path_found: bool,
    /// False until the first path request has ever been made — distinguishes
    /// "no path computed yet" from a legitimate `path_computed_for` of
    /// `Vec2::ZERO` (a valid world position, e.g. near spawn).
    pub has_path_target: bool,

    /// Throttles how often vision/hearing/state are re-evaluated.
    pub sense_timer: Timer,
    /// Throttles how often a path is recomputed.
    pub path_timer: Timer,

    /// Position sampled at the last stuck-check (see `stuck_timer`), and how
    /// many consecutive checks found the monster hadn't actually moved while
    /// it was trying to. The grid path only knows tiles are walkable — it
    /// doesn't know the monster's physical collider can still clip a wall's
    /// corner and get physically wedged there, so movement watches for "no
    /// real-world progress" as the fallback signal instead.
    pub stuck_probe_pos: Vec2,
    pub stuck_timer: Timer,
    pub stuck_ticks: u8,
    /// While `unstick_until` is in the future (compared against elapsed game
    /// time), movement ignores the path and pushes perpendicular to the
    /// blocked direction instead, to physically break free of whatever it's
    /// wedged against before resuming normal path-following.
    pub unstick_dir: Vec2,
    pub unstick_until: f32,
}

impl MonsterPerception {
    pub fn new(reaction_time: f32) -> Self {
        Self {
            state: MonsterState::Idle,
            facing: Vec2::X,
            last_seen_pos: None,
            last_seen_time: 0.0,
            last_seen_velocity: Vec2::ZERO,
            last_heard_pos: None,
            last_heard_time: 0.0,
            in_darkness: false,
            pack_flank_offset: Vec2::ZERO,
            investigate_target: None,
            investigate_started: 0.0,
            investigate_leg: 0,
            investigate_subtarget: None,
            path: Vec::new(),
            path_index: 0,
            path_computed_for: Vec2::ZERO,
            path_found: true,
            has_path_target: false,
            sense_timer: Timer::from_seconds(reaction_time, TimerMode::Repeating),
            path_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            stuck_probe_pos: Vec2::ZERO,
            stuck_timer: Timer::from_seconds(0.3, TimerMode::Repeating),
            stuck_ticks: 0,
            unstick_dir: Vec2::ZERO,
            unstick_until: 0.0,
        }
    }
}

/// Roughly a monster's own body width (see the `Rectangle::new(40.0, 42.5)`
/// spawn collider) — another monster's body counts as blocking the sightline
/// only when it passes within this distance of the straight line to the
/// player, so a packmate merely *near* the line (not actually in the way)
/// doesn't falsely block vision.
const MONSTER_BODY_BLOCK_RADIUS: f32 = 22.0;

/// True if any of `other_monsters` (excluding `self_entity`) has a body
/// between `from` and `to` — i.e. its position falls within
/// `MONSTER_BODY_BLOCK_RADIUS` of the segment and strictly between the two
/// endpoints, not merely near one end or beyond either.
fn blocked_by_monster_body(
    from: Vec2,
    to: Vec2,
    self_entity: Entity,
    other_monsters: &[(Entity, Vec2)],
) -> bool {
    let to_target = to - from;
    let dist = to_target.length();
    if dist < 0.0001 {
        return false;
    }
    let dir = to_target / dist;
    other_monsters.iter().any(|&(other, pos)| {
        if other == self_entity {
            return false;
        }
        let along = (pos - from).dot(dir);
        if along <= MONSTER_BODY_BLOCK_RADIUS || along >= dist - MONSTER_BODY_BLOCK_RADIUS {
            // Not between the two endpoints (with a little margin so a
            // monster standing right next to either end doesn't count).
            return false;
        }
        let closest = from + dir * along;
        closest.distance(pos) < MONSTER_BODY_BLOCK_RADIUS
    })
}

/// Range + field-of-view + line-of-sight check. Returns true only if the
/// player is within `vision_range`, inside the `vision_angle_deg` cone around
/// `facing`, no wall tile lies between monster and player, and no *other*
/// monster's body is directly in the way either — except within
/// `cfg.close_range`, where the facing cone is ignored (LOS and the
/// monster-body check still apply): something essentially touching the
/// monster registers regardless of which way it's currently facing, same as
/// a real creature would notice via peripheral vision/general proximity
/// rather than only dead ahead.
pub fn can_see_player(
    map: &TerrainMap,
    monster_pos: Vec2,
    facing: Vec2,
    player_pos: Vec2,
    cfg: &MonsterSenseConfig,
    self_entity: Entity,
    other_monsters: &[(Entity, Vec2)],
) -> bool {
    let to_player = player_pos - monster_pos;
    let distance = to_player.length();
    if distance > cfg.vision_range {
        return false;
    }

    // A monster that has never faced anywhere yet sees in every direction
    // rather than nowhere.
    if facing.length_squared() > 0.0001 && distance > cfg.close_range {
        let half_angle = (cfg.vision_angle_deg * 0.5).to_radians();
        let facing_n = facing.normalize();
        let to_player_n = to_player / distance;
        let cos_angle = facing_n.dot(to_player_n).clamp(-1.0, 1.0);
        if cos_angle.acos() > half_angle {
            return false;
        }
    }

    if !line_of_sight(map, monster_pos, player_pos) {
        return false;
    }

    !blocked_by_monster_body(monster_pos, player_pos, self_entity, other_monsters)
}

/// Distance-only hearing check (deliberately not gated by line of sight —
/// see requirement that hearing must differ from vision). Returns an
/// approximate sound location (jittered by the difficulty's memory accuracy)
/// if the player is currently making noise within the appropriate walk/run
/// radius, or `None` if silent/out of range.
pub fn hear_player(
    monster_pos: Vec2,
    noise: &PlayerNoise,
    cfg: &MonsterSenseConfig,
) -> Option<Vec2> {
    if !noise.moving {
        return None;
    }
    let radius = if noise.is_running {
        cfg.hearing_range_run
    } else {
        cfg.hearing_range_walk
    };
    if monster_pos.distance(noise.position) > radius {
        return None;
    }
    Some(jitter(noise.position, cfg.memory_accuracy_jitter))
}

/// Passive, omnidirectional proximity awareness ("smell"/body heat, not
/// eyes): unlike `can_see_player`, needs no facing cone or line of sight —
/// unlike `hear_player`, doesn't need the player making any noise either.
/// Real creatures aren't purely sight-and-sound; a monster standing/wandering
/// near a silent, motionless player should still eventually notice *someone
/// is there*, just not exactly where — same approximate-location treatment
/// as a heard sound (jittered, never the exact position), and it feeds into
/// Investigate rather than an instant Chase, since it's "something's here",
/// not "I can see them". Deliberately short range (`cfg.proximity_range`,
/// smaller than vision range) so it fills the specific gap — a stationary,
/// quiet player close by — without making real distance stealth pointless.
pub fn sense_player_nearby(
    monster_pos: Vec2,
    player_pos: Vec2,
    cfg: &MonsterSenseConfig,
) -> Option<Vec2> {
    if cfg.proximity_range <= 0.0 {
        return None;
    }
    if monster_pos.distance(player_pos) > cfg.proximity_range {
        return None;
    }
    Some(jitter(player_pos, cfg.memory_accuracy_jitter))
}

/// Offsets `pos` by a random vector inside a disk of radius `magnitude`,
/// modelling an approximate (never exact) remembered/heard location.
pub fn jitter(pos: Vec2, magnitude: f32) -> Vec2 {
    if magnitude <= 0.0001 {
        return pos;
    }
    let angle = rand::random::<f32>() * std::f32::consts::TAU;
    let radius = rand::random::<f32>().sqrt() * magnitude;
    pos + Vec2::new(angle.cos(), angle.sin()) * radius
}
