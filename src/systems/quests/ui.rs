use super::{QuestBoard, QuestUiState, TaskCompletedEvent, now_epoch_secs};
use crate::resourses::physics_resources::*;
use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct TaskCompletedToast(Timer);

#[derive(Component)]
pub(crate) struct QuestPanelBody;
#[derive(Component)]
pub(crate) struct QuestPanelHeader;
#[derive(Component)]
pub(crate) struct QuestSlotHeaderText(usize);
#[derive(Component)]
pub(crate) struct QuestSlotFill(usize);
#[derive(Component)]
pub(crate) struct QuestSlotProgressText(usize);
#[derive(Component)]
pub(crate) struct QuestSlotRewardText(usize);

/// Formats a number with `,` thousands separators — "1,500" not "1500", to
/// match the spec's reward formatting.
pub fn format_thousands(n: u64) -> String {
    let digits = n.to_string();
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*byte as char);
    }
    out
}

fn format_mmss(total_secs: u64) -> String {
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}

/// Spawns the collapsed-by-default task panel on the side of the screen.
/// Called only from `player_game_ui::spawn_gameplay_hud`, alongside the
/// rest of the gameplay HUD — tagged `PlayerUIs` + `GameplayHud` so it's
/// cleaned up and hidden by exactly the same lifecycle those already are.
pub fn spawn_quest_panel(commands: &mut Commands, assets: &Res<AssetServer>) {
    let font = assets.load("fonts/Cantarell-Bold.ttf");
    commands
        .spawn((
            PlayerUIs,
            GameplayHud,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.),
                right: Val::Px(10.),
                width: Val::Px(300.),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Button,
                QuestPanelHeader,
                Node {
                    width: Val::Percent(100.),
                    padding: UiRect::all(Val::Px(8.)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.08, 0.9)),
            ))
            .with_children(|header| {
                header.spawn((
                    Text::new("TASKS  [T]"),
                    TextFont {
                        font: font.clone(),
                        font_size: 18.,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((
                QuestPanelBody,
                Node {
                    width: Val::Percent(100.),
                    flex_direction: FlexDirection::Column,
                    display: Display::None,
                    row_gap: Val::Px(10.),
                    padding: UiRect::all(Val::Px(8.)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.88)),
            ))
            .with_children(|body| {
                for slot in 0..3 {
                    body.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            QuestSlotHeaderText(slot),
                            Text::new(""),
                            TextFont {
                                font: font.clone(),
                                font_size: 16.,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                        row.spawn((
                            Node {
                                width: Val::Percent(100.),
                                height: Val::Px(14.),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                QuestSlotFill(slot),
                                Node {
                                    width: Val::Percent(0.),
                                    height: Val::Percent(100.),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.3, 0.65, 0.9)),
                            ));
                        });
                        row.spawn((
                            QuestSlotProgressText(slot),
                            Text::new(""),
                            TextFont {
                                font: font.clone(),
                                font_size: 14.,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                        row.spawn((
                            QuestSlotRewardText(slot),
                            Text::new(""),
                            TextFont {
                                font: font.clone(),
                                font_size: 14.,
                                ..default()
                            },
                            TextColor(Color::srgb(0.85, 0.7, 0.2)),
                        ));
                    });
                }
            });
        });
}

pub fn toggle_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<QuestUiState>,
    header_query: Query<&Interaction, (With<QuestPanelHeader>, Changed<Interaction>)>,
) {
    let mut toggle = keyboard.just_pressed(KeyCode::KeyT);
    for interaction in &header_query {
        if *interaction == Interaction::Pressed {
            toggle = true;
        }
    }
    if toggle {
        state.expanded = !state.expanded;
    }
}

#[allow(clippy::type_complexity)]
pub fn update_panel(
    state: Res<QuestUiState>,
    board: Res<QuestBoard>,
    mut body_query: Query<&mut Node, With<QuestPanelBody>>,
    mut header_texts: Query<
        (&QuestSlotHeaderText, &mut Text),
        (Without<QuestSlotProgressText>, Without<QuestSlotRewardText>),
    >,
    mut fills: Query<(&QuestSlotFill, &mut Node), Without<QuestPanelBody>>,
    mut progress_texts: Query<
        (&QuestSlotProgressText, &mut Text),
        (Without<QuestSlotHeaderText>, Without<QuestSlotRewardText>),
    >,
    mut reward_texts: Query<
        (&QuestSlotRewardText, &mut Text),
        (Without<QuestSlotHeaderText>, Without<QuestSlotProgressText>),
    >,
) {
    if let Ok(mut node) = body_query.single_mut() {
        node.display = if state.expanded {
            Display::Flex
        } else {
            Display::None
        };
    }
    if !state.expanded {
        return;
    }

    for (marker, mut text) in &mut header_texts {
        let task = &board.slots[marker.0];
        let suffix = if task.completed { "  (COMPLETED)" } else { "" };
        *text = Text::new(format!(
            "{} — {}{}",
            task.difficulty.label(),
            task.objective.label(),
            suffix
        ));
    }
    for (marker, mut node) in &mut fills {
        let task = &board.slots[marker.0];
        node.width = Val::Percent(task.progress_fraction() * 100.0);
    }
    for (marker, mut text) in &mut progress_texts {
        let task = &board.slots[marker.0];
        *text = Text::new(if task.completed {
            let remaining = task
                .cooldown_ends_at
                .unwrap_or(0)
                .saturating_sub(now_epoch_secs());
            format!("New task in {}", format_mmss(remaining))
        } else {
            format!(
                "{} / {}{}",
                task.progress,
                task.target,
                task.objective.unit()
            )
        });
    }
    for (marker, mut text) in &mut reward_texts {
        let task = &board.slots[marker.0];
        *text = Text::new(format!(
            "Reward: {} Coins + {} XP",
            format_thousands(task.coin_reward),
            task.xp_reward
        ));
    }
}

/// Flashes a short "Task Completed!" toast the instant a task's reward is
/// granted (see `progress::apply_delta`) — a one-off notification alongside
/// the panel's own static "(COMPLETED)" label, which only a player who
/// already has the panel open would otherwise notice.
pub fn spawn_task_completed_toast(
    mut events: MessageReader<TaskCompletedEvent>,
    board: Res<QuestBoard>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    for event in events.read() {
        let task = &board.slots[event.slot];
        let font = asset_server.load("fonts/Cantarell-Bold.ttf");
        commands.spawn((
            PlayerUIs,
            TaskCompletedToast(Timer::from_seconds(2.5, TimerMode::Once)),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(36.),
                left: Val::Percent(50.),
                margin: UiRect::left(Val::Px(-150.)),
                width: Val::Px(300.),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Text::new(format!(
                "Task Completed: {} {}!",
                task.difficulty.label(),
                task.objective.label()
            )),
            TextFont {
                font,
                font_size: 24.,
                ..default()
            },
            TextColor(Color::srgb(0.4, 0.9, 0.5)),
        ));
    }
}

pub fn tick_task_completed_toast(
    time: Res<Time>,
    mut commands: Commands,
    mut toasts: Query<(Entity, &mut TaskCompletedToast)>,
) {
    for (entity, mut toast) in &mut toasts {
        toast.0.tick(time.delta());
        if toast.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
