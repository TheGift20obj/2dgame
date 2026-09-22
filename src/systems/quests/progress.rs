use super::config::QuestConfig;
use super::generation;
use super::{ObjectiveKind, QuestBoard, TaskCompletedEvent, now_epoch_secs};
use crate::resourses::physics_resources::*;
use crate::systems::progression::{Coins, LevelUpEvent, PlayerLevel};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// Bundles the progression side-effects of completing a task (granting the
/// reward, and the events that announce it) into one system param, so each
/// event-reading `track_*` system below stays at a handful of top-level
/// parameters regardless of how many objective kinds it drives.
#[derive(SystemParam)]
pub struct QuestOutputs<'w> {
    config: Res<'w, QuestConfig>,
    coins: ResMut<'w, Coins>,
    level: ResMut<'w, PlayerLevel>,
    completed: MessageWriter<'w, TaskCompletedEvent>,
    level_up: MessageWriter<'w, LevelUpEvent>,
}

/// Adds `amount` of progress to every active (not yet completed) slot whose
/// objective matches, and grants the reward + starts the cooldown for any
/// slot that reaches its target as a result. This is the single place task
/// rewards are granted — `completed` is flipped in the same step progress
/// crosses the target, so a task can never be completed (and its reward
/// granted) twice, whether from a duplicate event this tick or from the UI
/// being reopened/reconnected later (the UI never calls this — it only
/// reads `QuestBoard`, see `ui::update_panel`).
fn apply_delta(
    board: &mut QuestBoard,
    outputs: &mut QuestOutputs,
    objective: ObjectiveKind,
    amount: u32,
) {
    if amount == 0 {
        return;
    }
    for (index, task) in board.slots.iter_mut().enumerate() {
        if task.completed || task.objective != objective {
            continue;
        }
        task.progress = (task.progress + amount).min(task.target);
        if task.progress >= task.target {
            task.completed = true;
            task.cooldown_ends_at =
                Some(now_epoch_secs() + outputs.config.cooldown_secs(task.difficulty));
            outputs.coins.0 = outputs.coins.0.saturating_add(task.coin_reward);
            let levels_gained = outputs.level.add_xp(task.xp_reward);
            outputs.completed.write(TaskCompletedEvent { slot: index });
            if levels_gained > 0 {
                outputs.level_up.write(LevelUpEvent {
                    new_level: outputs.level.level,
                });
            }
        }
    }
}

/// Grants each kill's randomized `xp_reward` (see `MonsterKilledEvent`)
/// directly to the player, in addition to advancing any active "kill
/// monsters" task — two independent rewards from the same confirmed kill.
/// Each event is delivered to this reader exactly once (standard
/// `MessageReader` semantics), so a kill can't grant XP twice regardless of
/// UI state, respawns, or reconnects.
pub fn track_kills(
    mut events: MessageReader<MonsterKilledEvent>,
    mut board: ResMut<QuestBoard>,
    mut outputs: QuestOutputs,
) {
    let mut count = 0u32;
    for event in events.read() {
        count += 1;
        let levels_gained = outputs.level.add_xp(event.xp_reward);
        if levels_gained > 0 {
            outputs.level_up.write(LevelUpEvent {
                new_level: outputs.level.level,
            });
        }
    }
    apply_delta(&mut board, &mut outputs, ObjectiveKind::KillMonsters, count);
}

pub fn track_hits(
    mut events: MessageReader<MonsterHitEvent>,
    mut board: ResMut<QuestBoard>,
    mut outputs: QuestOutputs,
) {
    let mut hits = 0u32;
    let mut damage = 0u32;
    for event in events.read() {
        hits += 1;
        damage += event.damage.round().max(0.0) as u32;
    }
    apply_delta(&mut board, &mut outputs, ObjectiveKind::HitMonsters, hits);
    apply_delta(&mut board, &mut outputs, ObjectiveKind::DealDamage, damage);
}

pub fn track_consumption(
    mut events: MessageReader<ItemConsumedEvent>,
    mut board: ResMut<QuestBoard>,
    mut outputs: QuestOutputs,
) {
    let mut apples = 0u32;
    let mut food = 0u32;
    for event in events.read() {
        if event.item_id == "apple_red" {
            apples += 1;
        }
        if event.item_type == "food" {
            food += 1;
        }
    }
    apply_delta(
        &mut board,
        &mut outputs,
        ObjectiveKind::ConsumeApples,
        apples,
    );
    apply_delta(&mut board, &mut outputs, ObjectiveKind::ConsumeFood, food);
}

pub fn track_collection(
    mut events: MessageReader<ItemCollectedEvent>,
    mut board: ResMut<QuestBoard>,
    mut outputs: QuestOutputs,
) {
    let total: u32 = events.read().map(|event| event.amount).sum();
    apply_delta(&mut board, &mut outputs, ObjectiveKind::CollectItems, total);
}

/// How many world units make up one "meter" for `TravelDistance` task
/// purposes — reuses the tile grid as the game's natural distance scale.
const WORLD_UNITS_PER_METER: f32 = TILE_SIZE;

/// Tracks the player's position frame-to-frame to accumulate whole meters
/// traveled into `TravelDistance` tasks. Kept as its own resource (not
/// derived from `Transform` directly) so a session boundary (Play/Respawn/
/// Leave — see `reset_distance_tracker`) can cleanly zero it instead of the
/// player's position jumping (death location -> spawn point) reading as a
/// free burst of "distance traveled".
#[derive(Resource, Default)]
pub struct PlayerDistanceTracker {
    last_pos: Option<Vec2>,
    meter_accum: f32,
}

pub fn reset_distance_tracker(
    mut play: MessageReader<PlayRequested>,
    mut respawn: MessageReader<RespawnRequested>,
    mut leave: MessageReader<LeaveRequested>,
    mut tracker: ResMut<PlayerDistanceTracker>,
) {
    let reset = play.read().next().is_some()
        || respawn.read().next().is_some()
        || leave.read().next().is_some();
    if reset {
        *tracker = PlayerDistanceTracker::default();
    }
}

pub fn track_distance(
    player: Query<&Transform, (With<Player>, Without<Pending>)>,
    mut tracker: ResMut<PlayerDistanceTracker>,
    mut board: ResMut<QuestBoard>,
    mut outputs: QuestOutputs,
) {
    let Ok(transform) = player.single() else {
        tracker.last_pos = None;
        return;
    };
    let pos = transform.translation.xy();
    let Some(last) = tracker.last_pos else {
        tracker.last_pos = Some(pos);
        return;
    };
    tracker.last_pos = Some(pos);
    tracker.meter_accum += last.distance(pos) / WORLD_UNITS_PER_METER;
    let whole_meters = tracker.meter_accum.floor();
    if whole_meters >= 1.0 {
        tracker.meter_accum -= whole_meters;
        apply_delta(
            &mut board,
            &mut outputs,
            ObjectiveKind::TravelDistance,
            whole_meters as u32,
        );
    }
}

/// Regenerates any slot whose cooldown has finished — driven purely by the
/// real-world clock (`now_epoch_secs`), so this also correctly fires the
/// very first time it runs after a cooldown that fully elapsed while the
/// pause menu was open or the slot's cooldown outlasted the play session
/// (the same catch-up also runs once at load — see
/// `lifecycle::handle_play_requested`).
pub fn tick_cooldowns(config: Res<QuestConfig>, mut board: ResMut<QuestBoard>) {
    for task in board.slots.iter_mut() {
        generation::refresh_if_expired(&config, task);
    }
}
