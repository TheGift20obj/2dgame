//! The task/quest system: 3 always-active task slots (one per difficulty —
//! see `TaskDifficulty::ALL`), each independently generated, tracked,
//! completed, rewarded and put on a real-world cooldown. Responsibilities
//! are split one-per-file so the "generic objective/progress architecture"
//! stays generic instead of one god-module:
//!
//! - `config`: all the tunable numbers (target ranges, base rewards,
//!   cooldown durations) — nothing here does any logic.
//! - `generation`: rolls a fresh task (or a same-difficulty replacement)
//!   from `config`'s tuning.
//! - `rewards`: the reward-scaling formula generation calls into.
//! - `progress`: reacts to gameplay-fact events (a kill, a hit, a
//!   consumption, a pickup, distance moved) and grants rewards/starts
//!   cooldowns the instant a task's target is reached.
//! - `ui`: the collapsible side panel — reads `QuestBoard`, never writes to
//!   it (see `progress::apply_delta`'s doc comment on why rewards can only
//!   ever be granted from there).
//!
//! Persistence (`Task` is `Serialize`/`Deserialize` and doubles directly as
//! the save format) and session wiring (loading/saving, offline cooldown
//! catch-up) live in `save::SaveData` and `lifecycle`, alongside how every
//! other piece of session state (inventory, monsters, ...) is
//! already handled — see those modules rather than duplicating that flow
//! here.

pub mod config;
pub mod generation;
pub mod progress;
pub mod rewards;
pub mod ui;

pub use config::QuestConfig;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// The current real-world time as whole seconds since the Unix epoch — the
/// clock task cooldowns are measured against instead of gameplay/frame time,
/// so a cooldown keeps counting down even while the game isn't running (see
/// `progress::tick_cooldowns` and `generation::refresh_if_expired`). This is
/// a single-player, no-backend game, so the local OS clock is the most
/// trustworthy time source this architecture has; a networked/live-service
/// build would swap this one function for a server-issued timestamp without
/// touching any of its call sites.
pub fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskDifficulty {
    Easy,
    Medium,
    Hard,
}

impl TaskDifficulty {
    pub const ALL: [TaskDifficulty; 3] = [
        TaskDifficulty::Easy,
        TaskDifficulty::Medium,
        TaskDifficulty::Hard,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TaskDifficulty::Easy => "Easy",
            TaskDifficulty::Medium => "Medium",
            TaskDifficulty::Hard => "Hard",
        }
    }
}

/// One kind of objective a task can track. Adding a new task type touches
/// exactly four places: a new variant here, a tuning row per difficulty in
/// `QuestConfig::default`, a `label`/`unit` entry below, and one
/// `progress::apply_delta` call wired to whatever gameplay event reports
/// progress for it. Generation, reward scaling, cooldown handling,
/// persistence and the UI are all already generic over every variant and
/// need no changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveKind {
    KillMonsters,
    DealDamage,
    HitMonsters,
    TravelDistance,
    ConsumeApples,
    ConsumeFood,
    CollectItems,
}

impl ObjectiveKind {
    pub const ALL: [ObjectiveKind; 7] = [
        ObjectiveKind::KillMonsters,
        ObjectiveKind::DealDamage,
        ObjectiveKind::HitMonsters,
        ObjectiveKind::TravelDistance,
        ObjectiveKind::ConsumeApples,
        ObjectiveKind::ConsumeFood,
        ObjectiveKind::CollectItems,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ObjectiveKind::KillMonsters => "Kill Monsters",
            ObjectiveKind::DealDamage => "Deal Damage to Monsters",
            ObjectiveKind::HitMonsters => "Hit Monsters",
            ObjectiveKind::TravelDistance => "Travel Distance",
            ObjectiveKind::ConsumeApples => "Consume Apples",
            ObjectiveKind::ConsumeFood => "Consume Food",
            ObjectiveKind::CollectItems => "Collect Items",
        }
    }

    /// Suffix appended after the raw numbers in the progress bar label.
    pub fn unit(self) -> &'static str {
        match self {
            ObjectiveKind::TravelDistance => " m",
            ObjectiveKind::DealDamage => " dmg",
            _ => "",
        }
    }
}

/// One task's full state — a currently-tracked objective, or the record of
/// one just completed and waiting out its cooldown. Doubles as both the
/// runtime state (`QuestBoard::slots`) and the persisted save format
/// (`save::SaveData::quests`): there's nothing UI- or session-only about it,
/// so one type serves both instead of a parallel save DTO that could drift
/// out of sync with it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Task {
    pub difficulty: TaskDifficulty,
    pub objective: ObjectiveKind,
    pub target: u32,
    pub progress: u32,
    pub coin_reward: u64,
    pub xp_reward: u32,
    pub completed: bool,
    /// Real-world epoch-seconds timestamp (see `now_epoch_secs`) the
    /// cooldown ends at. `None` while the task is active; set the instant
    /// it's completed (see `progress::apply_delta`) and cleared again the
    /// instant a replacement task is generated (see
    /// `generation::refresh_if_expired`).
    pub cooldown_ends_at: Option<u64>,
}

impl Task {
    pub fn progress_fraction(&self) -> f32 {
        if self.target == 0 {
            1.0
        } else {
            (self.progress as f32 / self.target as f32).clamp(0.0, 1.0)
        }
    }
}

/// The player's 3 active tasks, one per difficulty (index matches
/// `TaskDifficulty::ALL`) — see the module docs.
#[derive(Resource)]
pub struct QuestBoard {
    pub slots: [Task; 3],
}

impl Default for QuestBoard {
    /// Placeholder-only: real values are always assigned by
    /// `lifecycle::handle_play_requested` before any UI can read them (fresh
    /// tasks for a new slot, restored + cooldown-caught-up ones for an
    /// existing save) — this exists purely so the resource has *something*
    /// to hold before the first Play.
    fn default() -> Self {
        Self {
            slots: TaskDifficulty::ALL.map(|difficulty| Task {
                difficulty,
                objective: ObjectiveKind::KillMonsters,
                target: 1,
                progress: 0,
                coin_reward: 0,
                xp_reward: 0,
                completed: false,
                cooldown_ends_at: None,
            }),
        }
    }
}

/// UI-only: whether the task panel is expanded. Starts collapsed (`false` is
/// the derived default) so it doesn't take up screen space until the player
/// asks for it.
#[derive(Resource, Default)]
pub struct QuestUiState {
    pub expanded: bool,
}

/// Fired the instant a task's reward is granted — UI reacts to this to flash
/// the "Completed" state instead of polling every frame for a change.
#[derive(Message)]
pub struct TaskCompletedEvent {
    pub slot: usize,
}

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(QuestConfig::default())
            .insert_resource(QuestUiState::default())
            .insert_resource(progress::PlayerDistanceTracker::default())
            .add_message::<TaskCompletedEvent>()
            .add_systems(
                Update,
                progress::reset_distance_tracker.in_set(crate::systems::lifecycle::AppSet::Lifecycle),
            )
            .add_systems(
                Update,
                (
                    progress::track_kills,
                    progress::track_hits,
                    progress::track_consumption,
                    progress::track_collection,
                    progress::track_distance,
                    ui::toggle_panel,
                    ui::update_panel,
                    ui::spawn_task_completed_toast,
                    ui::tick_task_completed_toast,
                )
                    .run_if(|game: Res<crate::resourses::physics_resources::GameStatus>,
                             pause: Res<crate::resourses::physics_resources::ResumeStatus>| {
                        game.0 && !pause.0
                    })
                    .in_set(crate::systems::lifecycle::AppSet::Gameplay),
            )
            .add_systems(
                Update,
                // Real-world cooldowns keep ticking even while the pause
                // menu is open — only a fully unloaded session (no active
                // slot at all) stops this, unlike the gameplay systems
                // above which also stop while merely paused.
                progress::tick_cooldowns
                    .run_if(|game: Res<crate::resourses::physics_resources::GameStatus>| game.0)
                    .in_set(crate::systems::lifecycle::AppSet::Gameplay),
            );
    }
}
