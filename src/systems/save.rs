use crate::resourses::physics_resources::*;
use crate::systems::monster_ai::difficulty::Difficulty;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub const SAVE_SLOT_COUNT: u8 = 4;

/// Which save slot the current run belongs to, if any. Owned by the game
/// lifecycle: set when a slot is picked to play, cleared when leaving.
#[derive(Resource, Default)]
pub struct ActiveSlot(pub Option<u8>);

/// What must survive across a player death, captured before the dead
/// player entity is despawned so a later Respawn/Leave can still use it.
/// Points aren't tracked here — `Score` is a plain resource independent of
/// any entity's lifetime, so it survives death/respawn on its own.
#[derive(Resource, Default)]
pub struct PendingRespawn {
    pub inventory: Option<HashMap<u32, Item>>,
}

/// One monster's persisted state — just enough that leaving and coming back
/// can't be used to reset a monster the player was fighting/fleeing to full
/// health at a fresh position (position + HP only; short-term perception
/// like current investigate target isn't worth persisting across a whole
/// session gap).
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct MonsterSaveData {
    pub position: (f32, f32),
    pub health: f32,
}

/// Everything a save slot persists. Terrain is not included: it's fully
/// deterministic from a fixed seed, so it doesn't need to be saved.
#[derive(Serialize, Deserialize, Clone)]
pub struct SaveData {
    pub health: f32,
    pub max_health: f32,
    pub satamina: f32,
    pub min_satamina: f32,
    pub max_satamina: f32,
    pub position: (f32, f32),
    pub inventory: HashMap<u32, Item>,
    pub points: u32,
    /// Monster AI difficulty this run was started with. Defaults to Normal
    /// for saves written before this field existed.
    #[serde(default)]
    pub difficulty: Difficulty,
    /// Every monster that existed when the player left, so leaving and
    /// rejoining can't be used to cheaply reset the current threat (a
    /// wounded pack back to full health at a fresh spawn distance, an
    /// actively-chasing monster gone entirely). Empty for saves written
    /// before this field existed, or if no monsters existed at the time.
    #[serde(default)]
    pub monsters: Vec<MonsterSaveData>,
    /// The pack's learned player-escape-direction estimate (see
    /// `monster_ai::hivemind::PlayerEscapeModel`) — Hard-only in practice,
    /// harmless to persist regardless. Defaults to "nothing learned yet".
    #[serde(default)]
    pub pack_escape_dir: (f32, f32),
    #[serde(default)]
    pub pack_escape_samples: u32,
}

fn saves_dir() -> PathBuf {
    PathBuf::from("saves")
}

pub fn save_path(slot: u8) -> PathBuf {
    saves_dir().join(format!("slot_{}.json", slot))
}

pub fn write_save(slot: u8, data: &SaveData) {
    let _ = fs::create_dir_all(saves_dir());
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = fs::write(save_path(slot), json);
    }
}

pub fn read_save(slot: u8) -> Option<SaveData> {
    let content = fs::read_to_string(save_path(slot)).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn delete_save(slot: u8) {
    let _ = fs::remove_file(save_path(slot));
}

/// Builds a `PlayerData` from saved values, keeping fields that aren't
/// persisted (timers) at their normal defaults.
pub fn player_data_from_save(save: &SaveData) -> PlayerData {
    PlayerData {
        health: save.health,
        max_health: save.max_health,
        inventory: Inventory {
            items: save.inventory.clone(),
            capacity: 16,
        },
        can_heal: Timer::from_seconds(DEFAULT_HEAL_COOLDOWN_SECONDS, TimerMode::Once),
        satamina: save.satamina,
        min_satamina: save.min_satamina,
        max_satamina: save.max_satamina,
        time_heal: Timer::from_seconds(0.375, TimerMode::Once),
        time_satamina: Timer::from_seconds(0.025, TimerMode::Once),
    }
}

/// Captures the current player's (and the current monsters') state into a
/// `SaveData`, for writing to disk.
#[allow(clippy::too_many_arguments)]
pub fn capture_save_data(
    transform: &Transform,
    player_data: &PlayerData,
    points: u32,
    difficulty: Difficulty,
    monsters: Vec<MonsterSaveData>,
    pack_escape_dir: (f32, f32),
    pack_escape_samples: u32,
) -> SaveData {
    SaveData {
        health: player_data.health,
        max_health: player_data.max_health,
        satamina: player_data.satamina,
        min_satamina: player_data.min_satamina,
        max_satamina: player_data.max_satamina,
        position: (transform.translation.x, transform.translation.y),
        inventory: player_data.inventory.items.clone(),
        points,
        difficulty,
        monsters,
        pack_escape_dir,
        pack_escape_samples,
    }
}
