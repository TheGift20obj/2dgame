use bevy::prelude::*;

use super::difficulty::MonsterSenseConfig;
use super::perception::{
    MonsterPerception, PlayerNoise, can_see_player, hear_player, sense_player_nearby,
};
use crate::systems::terrain::TerrainMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterState {
    /// Default random-wander behavior (unchanged from the pre-existing AI).
    Idle,
    /// Moving to (and searching around) a point of interest: a last-known
    /// player position whose memory expired, or a heard sound.
    Investigate,
    /// Actively pursuing a player the monster currently has information
    /// about (sees it, or saw it recently enough to still trust the memory).
    Chase,
    /// In melee range of a player the monster currently sees (or has just
    /// seen) — movement stops, the attack animation lifecycle takes over.
    Attack,
}

/// Re-evaluates a monster's knowledge of the player and decides its state
/// for the upcoming interval, following this priority (highest first):
/// 1. Player currently visible -> Chase or Attack (never omniscient: this is
///    the only path that can set the state to Attack).
/// 2. Was Chasing/Attacking and lost sight -> keep chasing the last known
///    position while memory lasts, then fall back to Investigate.
/// 3. A fresh sound is heard, or (lacking that) the player is passively
///    sensed nearby by proximity/"smell" even if silent and unseen -> the
///    monster isn't purely eyes-and-ears, but proximity sensing is short
///    range and only ever leads to Investigate, never straight to Chase.
/// 4. Already investigating with no new information -> give up after
///    `investigate_duration` and return to Idle.
#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    perception: &mut MonsterPerception,
    map: &TerrainMap,
    monster_pos: Vec2,
    player_pos: Vec2,
    noise: &PlayerNoise,
    cfg: &MonsterSenseConfig,
    attack_range: f32,
    now: f32,
    self_entity: Entity,
    other_monsters: &[(Entity, Vec2)],
) {
    let visible = can_see_player(
        map,
        monster_pos,
        perception.facing,
        player_pos,
        cfg,
        self_entity,
        other_monsters,
    );

    if visible {
        if let Some(prev_pos) = perception.last_seen_pos {
            let dt = (now - perception.last_seen_time).max(0.01);
            perception.last_seen_velocity = (player_pos - prev_pos) / dt;
        }
        perception.last_seen_pos = Some(player_pos);
        perception.last_seen_time = now;
        perception.investigate_target = None;

        let distance = monster_pos.distance(player_pos);
        perception.state = if distance <= attack_range {
            MonsterState::Attack
        } else {
            MonsterState::Chase
        };
        return;
    }

    match perception.state {
        MonsterState::Chase | MonsterState::Attack => {
            if let Some(last_pos) = perception.last_seen_pos {
                let elapsed = now - perception.last_seen_time;
                if elapsed <= cfg.memory_duration {
                    perception.state = MonsterState::Chase;
                } else {
                    perception.last_seen_pos = None;
                    perception.investigate_target = Some(last_pos);
                    perception.investigate_started = now;
                    perception.investigate_leg = 0;
                    perception.investigate_subtarget = None;
                    perception.state = MonsterState::Investigate;
                }
            } else {
                perception.state = MonsterState::Idle;
            }
        }
        MonsterState::Idle | MonsterState::Investigate => {
            // Hearing needs the player actually making noise; proximity
            // sensing doesn't, so it's what catches a stationary, silent
            // player standing close by that neither vision (wrong facing)
            // nor hearing (no noise) would otherwise ever notice.
            let sensed_pos = hear_player(monster_pos, noise, cfg)
                .or_else(|| sense_player_nearby(monster_pos, player_pos, cfg));
            if let Some(heard_pos) = sensed_pos {
                let is_new_sound = perception.last_heard_pos.is_none()
                    || now - perception.last_heard_time > cfg.reaction_time;
                perception.last_heard_pos = Some(heard_pos);
                perception.last_heard_time = now;
                if is_new_sound || perception.state == MonsterState::Idle {
                    perception.investigate_target = Some(heard_pos);
                    perception.investigate_started = now;
                    perception.investigate_leg = 0;
                    perception.investigate_subtarget = None;
                }
                perception.state = MonsterState::Investigate;
            } else if perception.state == MonsterState::Investigate
                && now - perception.investigate_started > cfg.investigate_duration
            {
                perception.state = MonsterState::Idle;
                perception.investigate_target = None;
            }
        }
    }
}

/// Roughly the player's running speed (see `player::update`'s `speed =
/// 350.0` while sprinting) — used only as a plausible magnitude for the
/// pack's learned escape *direction* when a monster has no personal velocity
/// reading of its own yet (e.g. its very first sighting this encounter, with
/// nothing previous to diff against). An approximation, not a claim the
/// monster somehow knows the player's exact speed.
const ASSUMED_FLEE_SPEED: f32 = 350.0;

/// A predicted pursuit point for a just-lost player: extrapolates from the
/// last confirmed sighting using its estimated velocity, scaled by the
/// difficulty's `prediction_strength` (0 on Easy/Normal in practice, since
/// their table entries are 0.0/0.15 — meaningfully "guessing ahead" is a Hard
/// trait). Never used to invent information the monster never received; it
/// only reshapes information already stored in `last_seen_pos` — falling
/// back to the pack's shared, *learned* escape direction (see
/// `PlayerEscapeModel`, zero unless Hard) only once the sighting has gone
/// stale (the monster is chasing memory, not current vision) *and* it has no
/// personal velocity reading either. Critically, while the sighting is still
/// fresh (the monster currently sees the player, including standing right
/// there not moving) this always uses the real, live velocity reading —
/// zero if the player is genuinely stationary — never the learned fallback,
/// which previously caused monsters to path toward a phantom point near a
/// player they were actively looking straight at instead of the player
/// itself the moment the player stopped moving.
pub fn predicted_chase_target(
    perception: &MonsterPerception,
    cfg: &MonsterSenseConfig,
    escape_bias: Vec2,
    now: f32,
) -> Option<Vec2> {
    perception.last_seen_pos.map(|pos| {
        let fresh = now - perception.last_seen_time <= cfg.reaction_time.max(0.05);
        let velocity = if fresh || perception.last_seen_velocity.length_squared() > 0.0001 {
            perception.last_seen_velocity
        } else {
            escape_bias * ASSUMED_FLEE_SPEED
        };
        pos + velocity * cfg.prediction_strength.clamp(0.0, 1.0)
    })
}

/// Picks a random point within `radius` of `center` for investigate-state
/// "search the area" wandering, leaning toward `bias_dir` (the pack's
/// learned escape direction — zero unless Hard) by `bias_strength` (0 =
/// pure chance, 1 = always toward `bias_dir`) instead of a uniformly random
/// angle, so search patterns favor historically-likely escape routes without
/// being fully deterministic.
pub fn pick_search_point(center: Vec2, radius: f32, bias_dir: Vec2, bias_strength: f32) -> Vec2 {
    let angle = rand::random::<f32>() * std::f32::consts::TAU;
    let random_dir = Vec2::new(angle.cos(), angle.sin());
    let dir = if bias_dir.length_squared() > 0.0001 {
        let blended = random_dir * (1.0 - bias_strength) + bias_dir.normalize() * bias_strength;
        blended.normalize_or_zero()
    } else {
        random_dir
    };
    let dir = if dir.length_squared() < 0.0001 {
        random_dir
    } else {
        dir
    };
    let dist = rand::random::<f32>().sqrt() * radius;
    center + dir * dist
}
