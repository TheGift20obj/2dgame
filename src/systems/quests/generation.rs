use super::config::QuestConfig;
use super::rewards::scaled_reward;
use super::{ObjectiveKind, Task, TaskDifficulty, now_epoch_secs};
use rand::Rng;

/// Picks a random objective kind and rolls a random target within its
/// configured range (see `QuestConfig::tuning`), then scales the
/// difficulty's base reward proportionally to how that target compares to
/// the tuning's `base_target` (see `rewards::scaled_reward`). Used both for
/// a slot's very first task and for the replacement generated once its
/// cooldown expires.
pub fn generate_task(config: &QuestConfig, difficulty: TaskDifficulty) -> Task {
    let objective = ObjectiveKind::ALL[rand::thread_rng().gen_range(0..ObjectiveKind::ALL.len())];
    let tuning = config.tuning(difficulty, objective);
    let target = rand::thread_rng().gen_range(tuning.target_min..=tuning.target_max);
    let (coin_reward, xp_reward) = scaled_reward(&tuning, target);

    Task {
        difficulty,
        objective,
        target,
        progress: 0,
        coin_reward,
        xp_reward,
        completed: false,
        cooldown_ends_at: None,
    }
}

/// Called both once at load time (to catch up any cooldown that fully
/// elapsed while the player was offline — see `Task::cooldown_ends_at`'s doc
/// comment) and every gameplay tick (`progress::tick_cooldowns`) for
/// cooldowns that finish while actively playing. Replaces a completed,
/// expired-cooldown task in place with a fresh one of the same difficulty;
/// leaves an active task, or one still on cooldown, untouched.
pub fn refresh_if_expired(config: &QuestConfig, task: &mut Task) {
    let Some(ends_at) = task.cooldown_ends_at else {
        return;
    };
    if now_epoch_secs() >= ends_at {
        *task = generate_task(config, task.difficulty);
    }
}
