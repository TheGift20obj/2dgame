use super::config::ObjectiveTuning;

/// Scales a tuning's base reward linearly by how the actual generated target
/// compares to its configured `base_target` — e.g. a target 20% above the
/// base pays 20% more of both Coins and XP, rounded to whole units and never
/// below 1 of each (so even a randomly-generated minimum-target task still
/// visibly rewards the player). Shared by every objective kind, so a new
/// task type gets fair scaling for free just by adding its tuning row to
/// `QuestConfig::default` — nothing here needs to change.
pub fn scaled_reward(tuning: &ObjectiveTuning, actual_target: u32) -> (u64, u32) {
    if tuning.base_target == 0 {
        return (tuning.base_coins, tuning.base_xp);
    }
    let ratio = actual_target as f64 / tuning.base_target as f64;
    let coins = ((tuning.base_coins as f64) * ratio).round().max(1.0) as u64;
    let xp = ((tuning.base_xp as f64) * ratio).round().max(1.0) as u32;
    (coins, xp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_reward_proportionally_to_generated_target() {
        // Spec example: base 10 kills -> 500 coins + 250 xp; a generated
        // target of 12 should scale both proportionally, not stay fixed.
        let tuning = ObjectiveTuning {
            target_min: 8,
            target_max: 12,
            base_target: 10,
            base_coins: 500,
            base_xp: 250,
        };
        let (coins, xp) = scaled_reward(&tuning, 12);
        assert_eq!(coins, 600);
        assert_eq!(xp, 300);
    }

    #[test]
    fn reward_never_rounds_down_to_zero() {
        let tuning = ObjectiveTuning {
            target_min: 1,
            target_max: 1,
            base_target: 1000,
            base_coins: 1,
            base_xp: 1,
        };
        let (coins, xp) = scaled_reward(&tuning, 1);
        assert!(coins >= 1);
        assert!(xp >= 1);
    }
}
