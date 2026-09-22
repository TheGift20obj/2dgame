use super::{ObjectiveKind, TaskDifficulty};
use bevy::prelude::*;
use std::collections::HashMap;

/// Tunable numbers for one (difficulty, objective) pair. `target_min`/
/// `target_max` is the randomized range a freshly generated task's objective
/// amount is drawn from (see `generation::generate_task`); `base_target`/
/// `base_coins`/`base_xp` is the reference point reward scaling is computed
/// against (see `rewards::scaled_reward`) — a generated target above/below
/// `base_target` scales the reward proportionally, so a randomly harder task
/// always pays more and a randomly easier one always pays less.
#[derive(Clone, Copy)]
pub struct ObjectiveTuning {
    pub target_min: u32,
    pub target_max: u32,
    pub base_target: u32,
    pub base_coins: u64,
    pub base_xp: u32,
}

/// All quest numbers in one place — per-difficulty cooldowns and the
/// per-objective target/reward tuning above. A `Resource` (not hardcoded
/// consts scattered through the generation/reward logic) so the numbers can
/// be retuned in one place, the same role `MonsterCombatConfig` plays for
/// combat.
#[derive(Resource)]
pub struct QuestConfig {
    cooldowns: HashMap<TaskDifficulty, u64>,
    objectives: HashMap<(TaskDifficulty, ObjectiveKind), ObjectiveTuning>,
}

#[allow(clippy::too_many_arguments)]
fn insert_tuning(
    map: &mut HashMap<(TaskDifficulty, ObjectiveKind), ObjectiveTuning>,
    difficulty: TaskDifficulty,
    objective: ObjectiveKind,
    target_min: u32,
    target_max: u32,
    base_target: u32,
    base_coins: u64,
    base_xp: u32,
) {
    map.insert(
        (difficulty, objective),
        ObjectiveTuning {
            target_min,
            target_max,
            base_target,
            base_coins,
            base_xp,
        },
    );
}

impl Default for QuestConfig {
    fn default() -> Self {
        use ObjectiveKind::*;
        use TaskDifficulty::*;

        let mut cooldowns = HashMap::new();
        cooldowns.insert(Easy, 5 * 60);
        cooldowns.insert(Medium, 20 * 60);
        cooldowns.insert(Hard, 45 * 60);

        let mut objectives = HashMap::new();

        // Easy — canonical reward 100 Coins + 30 XP at the base target.
        insert_tuning(&mut objectives, Easy, KillMonsters, 3, 5, 3, 100, 30);
        insert_tuning(&mut objectives, Easy, DealDamage, 80, 150, 100, 100, 30);
        insert_tuning(&mut objectives, Easy, HitMonsters, 5, 8, 6, 100, 30);
        insert_tuning(
            &mut objectives,
            Easy,
            TravelDistance,
            300,
            500,
            400,
            100,
            30,
        );
        insert_tuning(&mut objectives, Easy, ConsumeApples, 3, 5, 3, 100, 30);
        insert_tuning(&mut objectives, Easy, ConsumeFood, 3, 5, 3, 100, 30);
        insert_tuning(&mut objectives, Easy, CollectItems, 3, 6, 4, 100, 30);

        // Medium — canonical reward 500 Coins + 250 XP at the base target.
        insert_tuning(&mut objectives, Medium, KillMonsters, 8, 12, 10, 500, 250);
        insert_tuning(&mut objectives, Medium, DealDamage, 400, 600, 500, 500, 250);
        insert_tuning(&mut objectives, Medium, HitMonsters, 20, 30, 25, 500, 250);
        insert_tuning(
            &mut objectives,
            Medium,
            TravelDistance,
            800,
            1200,
            1000,
            500,
            250,
        );
        insert_tuning(&mut objectives, Medium, ConsumeApples, 10, 15, 12, 500, 250);
        insert_tuning(&mut objectives, Medium, ConsumeFood, 10, 15, 12, 500, 250);
        insert_tuning(&mut objectives, Medium, CollectItems, 12, 20, 15, 500, 250);

        // Hard — canonical reward 1,500 Coins + 1,000 XP at the base target.
        insert_tuning(&mut objectives, Hard, KillMonsters, 20, 30, 25, 1500, 1000);
        insert_tuning(
            &mut objectives,
            Hard,
            DealDamage,
            1200,
            1800,
            1500,
            1500,
            1000,
        );
        insert_tuning(&mut objectives, Hard, HitMonsters, 50, 75, 60, 1500, 1000);
        insert_tuning(
            &mut objectives,
            Hard,
            TravelDistance,
            2500,
            3500,
            3000,
            1500,
            1000,
        );
        insert_tuning(&mut objectives, Hard, ConsumeApples, 18, 25, 20, 1500, 1000);
        insert_tuning(&mut objectives, Hard, ConsumeFood, 18, 25, 20, 1500, 1000);
        insert_tuning(&mut objectives, Hard, CollectItems, 20, 30, 25, 1500, 1000);

        Self {
            cooldowns,
            objectives,
        }
    }
}

impl QuestConfig {
    pub fn cooldown_secs(&self, difficulty: TaskDifficulty) -> u64 {
        self.cooldowns.get(&difficulty).copied().unwrap_or(300)
    }

    pub fn tuning(&self, difficulty: TaskDifficulty, objective: ObjectiveKind) -> ObjectiveTuning {
        self.objectives
            .get(&(difficulty, objective))
            .copied()
            .unwrap_or(ObjectiveTuning {
                target_min: 1,
                target_max: 1,
                base_target: 1,
                base_coins: 0,
                base_xp: 0,
            })
    }
}
