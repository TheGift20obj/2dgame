use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Monster AI difficulty. Affects senses/memory/investigation/reaction only —
/// never HP or damage (that stays in `MonsterCombatConfig`, difficulty-independent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

/// Currently active difficulty for the running session. Set by the game
/// lifecycle on Play/Respawn, read by monster AI.
#[derive(Resource, Default, Clone, Copy)]
pub struct ActiveDifficulty(pub Difficulty);

/// Tunable senses/memory/investigation/intelligence values for one difficulty
/// level. All distances are in world units (pixels); ranges are expressed as
/// tile counts at call sites for readability and converted with `TILE_SIZE`.
#[derive(Clone, Copy)]
pub struct MonsterSenseConfig {
    /// How far the monster can see, in world units.
    pub vision_range: f32,
    /// Full field-of-view width, in degrees (monster sees +/- half of this
    /// around its facing direction).
    pub vision_angle_deg: f32,
    /// Within this distance, vision ignores the facing cone entirely (still
    /// needs line of sight) — otherwise a monster can stand right next to
    /// the player and not react simply because it happened to be facing the
    /// wrong way, which reads as broken rather than "has a blind spot."
    pub close_range: f32,
    /// Passive "smell"/body-heat awareness radius, in world units — works
    /// with no facing cone, no line of sight, and no player movement/noise
    /// required at all (see `perception::sense_player_nearby`). Fills the
    /// gap vision (needs facing+LOS) and hearing (needs the player moving)
    /// both leave: a stationary, silent player standing close by. Smaller
    /// than vision range on purpose — this is "something's nearby", not
    /// "I can see/hear them clearly".
    pub proximity_range: f32,
    /// Hearing radius while the player is walking, in world units.
    pub hearing_range_walk: f32,
    /// Hearing radius while the player is running, in world units.
    pub hearing_range_run: f32,
    /// How long a last-known player position stays useful, in seconds.
    pub memory_duration: f32,
    /// Random error radius applied to a remembered/heard position, in world
    /// units. Larger = less accurate.
    pub memory_accuracy_jitter: f32,
    /// How long the monster keeps investigating a point of interest before
    /// giving up, in seconds.
    pub investigate_duration: f32,
    /// How far from the investigation target the monster wanders while
    /// searching, in world units.
    pub investigate_radius: f32,
    /// How often (seconds) the monster re-evaluates vision/hearing/state.
    /// Larger = slower to react.
    pub reaction_time: f32,
    /// 0.0..1.0 strength of movement-direction extrapolation used on Hard
    /// when predicting where a lost player is heading.
    pub prediction_strength: f32,
    /// Pack sharing: whether a monster can share what it knows about the
    /// player with packmates within `communication_range` at all. Off on
    /// Easy, minimal on Normal (live sightings only), full on Hard (also
    /// standing memory — see `share_full_memory`).
    pub telepathy_enabled: bool,
    /// Max distance (world units) shared info can be received from — not
    /// infinite, so packs only coordinate when actually near each other.
    pub communication_range: f32,
    /// If true, a monster shares its standing memory of the player (still
    /// within `memory_duration`, not just a sighting confirmed this exact
    /// tick) — Hard only. On Normal only a live, this-tick sighting is
    /// shared ("minimum" sharing).
    pub share_full_memory: bool,
    /// Flanking offset as a fraction of the distance to the shared/last-known
    /// position (not a fixed world distance): a monster that isn't the pack's
    /// current "primary" pursuer — or one reacting to relayed info it didn't
    /// sense itself — aims to one side by this fraction of that distance
    /// instead of the exact point, so packmates approach from different
    /// angles and naturally route around opposite sides of a big obstacle
    /// (a lake, a mountain, a wall cluster) instead of funneling through the
    /// same gap as one group. Capped by `flank_offset_max` so a far-off
    /// shared position doesn't send a flanker wildly off course — the aim is
    /// pack members plausibly converging on the same target from different
    /// angles, not scattering to opposite ends of the map.
    pub flank_offset_fraction: f32,
    /// Hard ceiling (world units) on the flanking offset regardless of
    /// distance — see `flank_offset_fraction`.
    pub flank_offset_max: f32,
}

/// How many monsters are kept spawned around the player at once. Population
/// only — never HP/damage, same rule as `MonsterSenseConfig`.
#[derive(Clone, Copy)]
pub struct MonsterPopulationConfig {
    pub max_monsters: usize,
}

pub fn population_config(difficulty: Difficulty) -> MonsterPopulationConfig {
    match difficulty {
        Difficulty::Easy => MonsterPopulationConfig { max_monsters: 1 },
        Difficulty::Normal => MonsterPopulationConfig { max_monsters: 3 },
        Difficulty::Hard => MonsterPopulationConfig { max_monsters: 6 },
    }
}

use crate::resourses::physics_resources::TILE_SIZE;

pub fn sense_config(difficulty: Difficulty) -> MonsterSenseConfig {
    match difficulty {
        Difficulty::Easy => MonsterSenseConfig {
            vision_range: 5.5 * TILE_SIZE,
            vision_angle_deg: 60.0,
            close_range: 1.0 * TILE_SIZE,
            proximity_range: 1.75 * TILE_SIZE,
            hearing_range_walk: 2.5 * TILE_SIZE,
            hearing_range_run: 6.0 * TILE_SIZE,
            memory_duration: 2.5,
            memory_accuracy_jitter: 2.5 * TILE_SIZE,
            investigate_duration: 4.0,
            investigate_radius: 2.0 * TILE_SIZE,
            reaction_time: 0.45,
            prediction_strength: 0.0,
            telepathy_enabled: false,
            communication_range: 0.0,
            share_full_memory: false,
            flank_offset_fraction: 0.0,
            flank_offset_max: 0.0,
        },
        Difficulty::Normal => MonsterSenseConfig {
            vision_range: 7.0 * TILE_SIZE,
            vision_angle_deg: 90.0,
            close_range: 1.5 * TILE_SIZE,
            proximity_range: 3.0 * TILE_SIZE,
            hearing_range_walk: 4.0 * TILE_SIZE,
            hearing_range_run: 9.0 * TILE_SIZE,
            memory_duration: 5.0,
            memory_accuracy_jitter: 1.0 * TILE_SIZE,
            investigate_duration: 7.0,
            investigate_radius: 3.0 * TILE_SIZE,
            reaction_time: 0.25,
            prediction_strength: 0.15,
            // "Minimum" sharing: only a live sighting is relayed, only to
            // monsters already fairly close, and the flank nudge is subtle.
            telepathy_enabled: true,
            communication_range: 6.0 * TILE_SIZE,
            share_full_memory: false,
            flank_offset_fraction: 0.08,
            flank_offset_max: 1.5 * TILE_SIZE,
        },
        Difficulty::Hard => MonsterSenseConfig {
            vision_range: 9.5 * TILE_SIZE,
            vision_angle_deg: 120.0,
            close_range: 2.25 * TILE_SIZE,
            proximity_range: 4.5 * TILE_SIZE,
            hearing_range_walk: 6.0 * TILE_SIZE,
            hearing_range_run: 13.0 * TILE_SIZE,
            memory_duration: 9.0,
            memory_accuracy_jitter: 0.25 * TILE_SIZE,
            investigate_duration: 11.0,
            investigate_radius: 4.5 * TILE_SIZE,
            reaction_time: 0.12,
            prediction_strength: 0.5,
            // Full sharing: standing memory (not just live sightings) is
            // relayed, over a longer range, with a flank offset strong
            // enough to route around a genuinely different side of a big
            // obstacle without pack members scattering apart pointlessly.
            telepathy_enabled: true,
            communication_range: 16.0 * TILE_SIZE,
            share_full_memory: true,
            flank_offset_fraction: 0.2,
            flank_offset_max: 3.5 * TILE_SIZE,
        },
    }
}
