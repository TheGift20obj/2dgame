use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// One monster's currently "broadcast" awareness of the player — recorded
/// only for a monster that currently trusts its *own* info (a live sighting,
/// or on Hard still-standing memory — see `MonsterSenseConfig`), never a
/// relayed one, so alerts can't feed back into more alerts and cascade into
/// false omniscience.
#[derive(Clone, Copy)]
pub struct PackAlert {
    pub source: Entity,
    pub source_pos: Vec2,
    pub player_pos: Vec2,
}

/// Difficulty-gated "pack telepathy" (off on Easy, minimal on Normal, full
/// on Hard): lets a monster that currently trusts its own info about the
/// player share it with packmates within `MonsterSenseConfig::
/// communication_range`, instead of every monster only ever knowing what it
/// personally sensed. Alerts recorded during one `monster_ai` pass are
/// consulted at the start of the next one (one frame stale — imperceptible
/// for this purpose, and avoids same-tick ordering hazards between monsters
/// processed earlier/later in the same query).
#[derive(Resource, Default)]
pub struct MonsterHiveMind {
    pub alerts: Vec<PackAlert>,
}

/// Deterministic left/right flanking side for `entity`, so multiple monsters
/// reacting to the same alert spread out to approach from different angles
/// instead of converging on the identical point.
pub fn flank_side(entity: Entity) -> f32 {
    if entity.index().index().is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// Hard-only "basic learning": a pack-wide, exponentially-smoothed estimate
/// of which way the player tends to run once spotted, built up from every
/// Hard monster's confirmed sightings over the whole session (unlike
/// `MonsterHiveMind`, this persists — it's a slowly-refined trend, not a
/// this-tick broadcast). Used to give prediction/search a plausible
/// direction even for a monster that never personally saw the player (e.g.
/// one only reacting to a relayed alert), and to make Hard search patterns
/// lean toward historically-likely escape routes instead of pure chance —
/// deliberately simple (a running direction average, not a trained model),
/// matching a 2D top-down game's actual needs rather than real ML.
#[derive(Resource, Default)]
pub struct PlayerEscapeModel {
    pub avg_flee_dir: Vec2,
    samples: u32,
}

impl PlayerEscapeModel {
    /// Blends a freshly observed player velocity into the running average.
    /// Ignored if the player wasn't actually moving (no direction to learn
    /// from). Blend weight fades from 1.0 (first sample) down to `min_blend`
    /// as more samples accumulate, so the estimate settles rather than
    /// chasing every single sighting.
    pub fn observe(&mut self, velocity: Vec2) {
        const MIN_BLEND: f32 = 0.08;
        if velocity.length_squared() < 0.0001 {
            return;
        }
        let dir = velocity.normalize();
        if self.samples == 0 {
            self.avg_flee_dir = dir;
        } else {
            let blend = (1.0 / (self.samples as f32 + 1.0)).max(MIN_BLEND);
            self.avg_flee_dir =
                (self.avg_flee_dir + (dir - self.avg_flee_dir) * blend).normalize_or_zero();
        }
        self.samples = self.samples.saturating_add(1);
    }

    /// Current sample count, for persisting alongside `avg_flee_dir` (see
    /// `save::SaveData::pack_escape_samples`) so a restored estimate keeps
    /// settling at the same rate instead of acting like a brand new "first
    /// sample" that would let one post-load sighting overwrite it outright.
    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Restores a previously-saved estimate — used when loading a save, so
    /// the pack doesn't "forget" what it learned last session just because
    /// the player left and came back.
    pub fn restore(&mut self, avg_flee_dir: Vec2, samples: u32) {
        self.avg_flee_dir = avg_flee_dir;
        self.samples = samples;
    }
}

/// Bundles the pack-coordination resources `monster_ai` needs into one
/// system parameter — `MonsterHiveMind` and `PlayerEscapeModel` are always
/// used together, and Bevy caps a plain system function at 16 parameters.
#[derive(SystemParam)]
pub struct PackComms<'w> {
    pub hive_mind: ResMut<'w, MonsterHiveMind>,
    pub escape_model: ResMut<'w, PlayerEscapeModel>,
}
