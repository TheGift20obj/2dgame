use crate::resourses::physics_resources::*;
use bevy::prelude::*;

/// The player's persistent coin balance — a plain resource independent of
/// any UI entity's lifetime, mutated only by gameplay logic (currently:
/// `quests::progress::apply_delta` granting a task reward). The HUD
/// (`update_progression_ui` below) only ever reads it — coins can't be
/// added or spent from UI/input code.
#[derive(Resource, Default, Clone, Copy)]
pub struct Coins(pub u64);

/// XP curve: the amount of XP required to go from `level` to `level + 1`.
/// `100` for level 1, and for every level after that
/// `requirement(level) = requirement(level - 1) + (25 + level)`. Computed
/// fresh each call instead of a hardcoded table — cheap, since it's only
/// ever called on level-up or on loading a save, never every frame (see
/// `PlayerLevel::xp_to_next`) — so the whole curve lives in one place and
/// can be changed without touching anything that uses it.
pub fn xp_requirement(level: u32) -> u32 {
    let level = level.max(1);
    let mut requirement = 100u32;
    for l in 2..=level {
        requirement += 25 + l;
    }
    requirement
}

/// Player level + XP progress toward the next one. `xp` is always the
/// carried-over progress within the *current* level (never cumulative
/// across levels), and `xp_to_next` caches `xp_requirement(level)` so the
/// HUD doesn't recompute the whole curve every frame.
#[derive(Resource, Clone, Copy)]
pub struct PlayerLevel {
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
}

impl Default for PlayerLevel {
    fn default() -> Self {
        Self::new(1, 0)
    }
}

impl PlayerLevel {
    /// Builds a level/xp pair, safely re-normalizing through `add_xp` in
    /// case `xp` is (or, after an XP-curve tweak, has become) large enough to
    /// itself cross one or more level thresholds — e.g. restoring a save
    /// written under a different curve.
    pub fn new(level: u32, xp: u32) -> Self {
        let level = level.max(1);
        let mut result = Self {
            level,
            xp: 0,
            xp_to_next: xp_requirement(level),
        };
        result.add_xp(xp);
        result
    }

    /// Adds XP, carrying any excess into as many level-ups as it takes —
    /// excess XP is never discarded. Returns how many levels were gained, so
    /// the caller can decide whether to fire a level-up notification.
    pub fn add_xp(&mut self, amount: u32) -> u32 {
        self.xp += amount;
        let mut levels_gained = 0u32;
        while self.xp >= self.xp_to_next {
            self.xp -= self.xp_to_next;
            self.level += 1;
            self.xp_to_next = xp_requirement(self.level);
            levels_gained += 1;
        }
        levels_gained
    }
}

/// Fired whenever `PlayerLevel::add_xp` crosses one or more level
/// thresholds — the HUD reacts to this to show a level-up notification
/// instead of polling `PlayerLevel` every frame for a change.
#[derive(Message)]
pub struct LevelUpEvent {
    pub new_level: u32,
}

#[derive(Component)]
struct LevelText;
#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct XpText;
#[derive(Component)]
struct CoinsText;
#[derive(Component)]
struct LevelUpToast(Timer);

pub struct ProgressionPlugin;

impl Plugin for ProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<LevelUpEvent>().add_systems(
            Update,
            (
                update_progression_ui,
                spawn_level_up_toast,
                tick_level_up_toast,
            )
                .run_if(|game: Res<GameStatus>, pause: Res<ResumeStatus>| game.0 && !pause.0)
                .in_set(crate::systems::lifecycle::AppSet::Gameplay),
        );
    }
}

/// Spawns the always-on Level/XP/Coins HUD block. Called only from
/// `player_game_ui::spawn_gameplay_hud`, alongside the rest of the gameplay
/// HUD — tagged `PlayerUIs` + `GameplayHud` so it's cleaned up and hidden by
/// exactly the same lifecycle those already are.
pub fn spawn_progression_ui(commands: &mut Commands, assets: &Res<AssetServer>) {
    let font = assets.load("fonts/Cantarell-Bold.ttf");
    commands
        .spawn((
            PlayerUIs,
            GameplayHud,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.),
                left: Val::Percent(50.),
                margin: UiRect::left(Val::Px(-150.)),
                width: Val::Px(300.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.),
                padding: UiRect::all(Val::Px(6.)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.75)),
        ))
        .with_children(|parent| {
            parent.spawn((
                LevelText,
                Text::new("Level 1"),
                TextFont {
                    font: font.clone(),
                    font_size: 18.,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.),
                        height: Val::Px(12.),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        XpFill,
                        Node {
                            width: Val::Percent(0.),
                            height: Val::Percent(100.),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.55, 0.35, 0.85)),
                    ));
                });
            parent.spawn((
                XpText,
                Text::new("0 / 100 XP"),
                TextFont {
                    font: font.clone(),
                    font_size: 14.,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                CoinsText,
                Text::new("Coins: 0"),
                TextFont {
                    font: font.clone(),
                    font_size: 16.,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.8, 0.25)),
            ));
        });
}

fn update_progression_ui(
    coins: Res<Coins>,
    level: Res<PlayerLevel>,
    mut level_texts: Query<&mut Text, (With<LevelText>, Without<XpText>, Without<CoinsText>)>,
    mut xp_texts: Query<&mut Text, (With<XpText>, Without<LevelText>, Without<CoinsText>)>,
    mut coin_texts: Query<&mut Text, (With<CoinsText>, Without<LevelText>, Without<XpText>)>,
    mut xp_fill: Query<&mut Node, With<XpFill>>,
) {
    for mut text in &mut level_texts {
        *text = Text::new(format!("Level {}", level.level));
    }
    for mut text in &mut xp_texts {
        *text = Text::new(format!(
            "{} / {} XP",
            crate::systems::quests::ui::format_thousands(level.xp as u64),
            crate::systems::quests::ui::format_thousands(level.xp_to_next as u64)
        ));
    }
    for mut text in &mut coin_texts {
        *text = Text::new(format!(
            "Coins: {}",
            crate::systems::quests::ui::format_thousands(coins.0)
        ));
    }
    for mut node in &mut xp_fill {
        let frac = if level.xp_to_next == 0 {
            0.0
        } else {
            (level.xp as f32 / level.xp_to_next as f32).clamp(0.0, 1.0)
        };
        node.width = Val::Percent(frac * 100.0);
    }
}

/// One toast per level-up event, even if several land the same frame (e.g. a
/// big XP grant crossing multiple thresholds at once) — each is independent
/// and despawns itself on its own timer (`tick_level_up_toast`).
fn spawn_level_up_toast(
    mut events: MessageReader<LevelUpEvent>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    for event in events.read() {
        let font = asset_server.load("fonts/Cantarell-Bold.ttf");
        commands.spawn((
            PlayerUIs,
            LevelUpToast(Timer::from_seconds(2.5, TimerMode::Once)),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(28.),
                left: Val::Percent(50.),
                margin: UiRect::left(Val::Px(-150.)),
                width: Val::Px(300.),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Text::new(format!("LEVEL UP! Now Level {}", event.new_level)),
            TextFont {
                font,
                font_size: 34.,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.85, 0.2)),
        ));
    }
}

fn tick_level_up_toast(
    time: Res<Time>,
    mut commands: Commands,
    mut toasts: Query<(Entity, &mut LevelUpToast)>,
) {
    for (entity, mut toast) in &mut toasts {
        toast.0.tick(time.delta());
        if toast.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_curve_matches_spec_examples() {
        assert_eq!(xp_requirement(1), 100);
        assert_eq!(xp_requirement(2), 127);
        assert_eq!(xp_requirement(3), 155);
        assert_eq!(xp_requirement(4), 184);
    }

    #[test]
    fn excess_xp_carries_over_on_level_up() {
        let mut level = PlayerLevel::new(1, 0);
        let gained = level.add_xp(150);
        assert_eq!(gained, 1);
        assert_eq!(level.level, 2);
        assert_eq!(level.xp, 50);
        assert_eq!(level.xp_to_next, 127);
    }

    #[test]
    fn multiple_level_ups_from_one_grant_are_all_applied() {
        let mut level = PlayerLevel::new(1, 0);
        // 100 (L1->2) + 127 (L2->3) + 10 leftover.
        let gained = level.add_xp(100 + 127 + 10);
        assert_eq!(gained, 2);
        assert_eq!(level.level, 3);
        assert_eq!(level.xp, 10);
    }
}
