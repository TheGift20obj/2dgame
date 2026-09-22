use crate::resourses::physics_resources::*;
use crate::systems::lifecycle::AppSet;
use crate::systems::monster_ai::difficulty::{ActiveDifficulty, Difficulty};
use crate::systems::monster_ai::hivemind::PlayerEscapeModel;
use crate::systems::save::{self, ActiveSlot, MonsterSaveData};
use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// Bundles the session-state resources (and the monster query) the Save
/// button needs into one system parameter — `button_system` was already
/// close to Bevy's 16-parameter cap on a plain system function.
#[derive(SystemParam)]
struct SaveContext<'w, 's> {
    active_slot: Res<'w, ActiveSlot>,
    active_difficulty: Res<'w, ActiveDifficulty>,
    escape_model: Res<'w, PlayerEscapeModel>,
    coins: Res<'w, crate::systems::progression::Coins>,
    level: Res<'w, crate::systems::progression::PlayerLevel>,
    quests: Res<'w, crate::systems::quests::QuestBoard>,
    monster_query: Query<'w, 's, (&'static Transform, &'static MonsterAI), With<Monster>>,
}

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);
pub struct MenuPlugin;

/// Holds a permanent strong handle to the main menu's background texture.
/// Without this, every despawn of the main menu screen (Leave, BackToMenu)
/// drops the only strong handle to it, Bevy unloads the image, and the next
/// `setup_main_menu` call has to re-decode it from disk asynchronously —
/// the background then visibly pops in after the buttons, which render
/// immediately since they need no asset load. Keeping one handle alive here
/// for the app's whole lifetime means the texture is loaded once and every
/// later `setup_main_menu` call just reuses the already-resident asset.
#[derive(Resource)]
pub struct MenuAssets {
    pub background: Handle<Image>,
}

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameStatus(false))
            .insert_resource(ResumeStatus(false))
            .add_systems(Startup, init)
            .add_systems(Update, button_system.in_set(AppSet::UiIntent));
    }
}

fn init(mut commands: Commands, asset_server: Res<AssetServer>) {
    let background = asset_server.load("textures/menu.png");
    setup_main_menu(&mut commands, &asset_server, background.clone());
    commands.insert_resource(MenuAssets { background });
    commands.spawn((Camera2d, MenuCamera));
}

fn menu_button_bundle(action: MenuButtonAction) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(220.0),
            height: Val::Px(60.0),
            margin: UiRect::all(Val::Px(8.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(NORMAL_BUTTON),
        BorderColor::all(Color::BLACK),
        MenuButton(action),
    )
}

fn menu_button_text(label: &str, font: &Handle<Font>) -> impl Bundle {
    (
        Text::new(label.to_string()),
        TextFont {
            font: font.clone(),
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::WHITE),
    )
}

fn small_button_bundle(action: MenuButtonAction) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(90.0),
            height: Val::Px(40.0),
            margin: UiRect::all(Val::Px(4.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(NORMAL_BUTTON),
        BorderColor::all(Color::BLACK),
        MenuButton(action),
    )
}

fn small_button_text(label: &str, font: &Handle<Font>) -> impl Bundle {
    (
        Text::new(label.to_string()),
        TextFont {
            font: font.clone(),
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
    )
}

pub fn setup_main_menu(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    background: Handle<Image>,
) {
    let font = asset_server.load("fonts/Cantarell-Bold.ttf");

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                padding: UiRect::top(Val::Percent(13.5)),
                ..default()
            },
            ImageNode::new(background),
            MenuRoot,
        ))
        .with_children(|parent| {
            parent.spawn((Node {
                margin: UiRect::bottom(Val::Px(20.0)),
                ..default()
            },));
            parent.spawn((Node {
                margin: UiRect::bottom(Val::Px(20.0)),
                ..default()
            },));

            parent
                .spawn(menu_button_bundle(MenuButtonAction::Play))
                .with_children(|p| {
                    p.spawn(menu_button_text("Play", &font));
                });
            parent
                .spawn(menu_button_bundle(MenuButtonAction::Options))
                .with_children(|p| {
                    p.spawn(menu_button_text("Options", &font));
                });
            parent
                .spawn(menu_button_bundle(MenuButtonAction::Exit))
                .with_children(|p| {
                    p.spawn(menu_button_text("Exit", &font));
                });
        });
}

pub fn setup_slot_select(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    let font = asset_server.load("fonts/Cantarell-Bold.ttf");

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.95)),
            MenuRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Select a Save Slot"),
                TextFont {
                    font: font.clone(),
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
            ));

            for slot in 1..=save::SAVE_SLOT_COUNT {
                let existing = save::read_save(slot);
                parent
                    .spawn((Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    },))
                    .with_children(|row| match &existing {
                        Some(data) => {
                            row.spawn((
                                Text::new(format!(
                                    "Slot {}  HP {:.0}/{:.0}  Lv {}  Coins {}",
                                    slot, data.health, data.max_health, data.level, data.coins
                                )),
                                TextFont {
                                    font: font.clone(),
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                Node {
                                    width: Val::Px(280.0),
                                    ..default()
                                },
                            ));
                            row.spawn(small_button_bundle(MenuButtonAction::PlaySlot(slot)))
                                .with_children(|p| {
                                    p.spawn(small_button_text("Play", &font));
                                });
                            row.spawn(small_button_bundle(MenuButtonAction::ResetSlot(slot)))
                                .with_children(|p| {
                                    p.spawn(small_button_text("Reset", &font));
                                });
                            row.spawn(small_button_bundle(MenuButtonAction::DeleteSlot(slot)))
                                .with_children(|p| {
                                    p.spawn(small_button_text("Delete", &font));
                                });
                        }
                        None => {
                            row.spawn((
                                Text::new(format!("Slot {}  Empty", slot)),
                                TextFont {
                                    font: font.clone(),
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                Node {
                                    width: Val::Px(280.0),
                                    ..default()
                                },
                            ));
                            row.spawn(small_button_bundle(MenuButtonAction::PickDifficulty(slot)))
                                .with_children(|p| {
                                    p.spawn(small_button_text("Start", &font));
                                });
                        }
                    });
            }

            parent
                .spawn(menu_button_bundle(MenuButtonAction::BackToMenu))
                .with_children(|p| {
                    p.spawn(menu_button_text("Back", &font));
                });
        });
}

pub fn setup_difficulty_select(commands: &mut Commands, asset_server: &Res<AssetServer>, slot: u8) {
    let font = asset_server.load("fonts/Cantarell-Bold.ttf");

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.95)),
            MenuRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Monster AI Difficulty"),
                TextFont {
                    font: font.clone(),
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
            ));

            for (label, difficulty) in [
                ("Easy", Difficulty::Easy),
                ("Normal", Difficulty::Normal),
                ("Hard", Difficulty::Hard),
            ] {
                parent
                    .spawn(menu_button_bundle(MenuButtonAction::StartWithDifficulty(
                        slot, difficulty,
                    )))
                    .with_children(|p| {
                        p.spawn(menu_button_text(label, &font));
                    });
            }

            parent
                .spawn(menu_button_bundle(MenuButtonAction::BackToSlotSelect))
                .with_children(|p| {
                    p.spawn(menu_button_text("Back", &font));
                });
        });
}

pub fn setup_pause_menu(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    let font = asset_server.load("fonts/Cantarell-Bold.ttf");

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            MenuRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Paused"),
                TextFont {
                    font: font.clone(),
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(16.0)),
                    ..default()
                },
            ));
            parent
                .spawn(menu_button_bundle(MenuButtonAction::Resume))
                .with_children(|p| {
                    p.spawn(menu_button_text("Resume", &font));
                });
            parent
                .spawn(menu_button_bundle(MenuButtonAction::Save))
                .with_children(|p| {
                    p.spawn(menu_button_text("Save", &font));
                });
            parent
                .spawn(menu_button_bundle(MenuButtonAction::Leave))
                .with_children(|p| {
                    p.spawn(menu_button_text("Leave", &font));
                });
        });
}

pub fn setup_death_screen(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    let font = asset_server.load("fonts/Cantarell-Bold.ttf");

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.0, 0.0, 0.75)),
            MenuRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("You Died"),
                TextFont {
                    font: font.clone(),
                    font_size: 40.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(16.0)),
                    ..default()
                },
            ));
            parent
                .spawn(menu_button_bundle(MenuButtonAction::Respawn))
                .with_children(|p| {
                    p.spawn(menu_button_text("Respawn", &font));
                });
            parent
                .spawn(menu_button_bundle(MenuButtonAction::Leave))
                .with_children(|p| {
                    p.spawn(menu_button_text("Leave", &font));
                });
        });
}

fn button_system(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, Option<&MenuButton>),
        (Changed<Interaction>, With<Button>),
    >,
    menu_root_query: Query<Entity, With<MenuRoot>>,
    mut exit: MessageWriter<AppExit>,
    asset_server: Res<AssetServer>,
    mut play_requested: MessageWriter<PlayRequested>,
    mut leave_requested: MessageWriter<LeaveRequested>,
    mut respawn_requested: MessageWriter<RespawnRequested>,
    mut resume_status: ResMut<ResumeStatus>,
    player_query: Query<(&Transform, &PlayerData), With<Player>>,
    menu_assets: Res<MenuAssets>,
    mut mouse_input: ResMut<ButtonInput<MouseButton>>,
    save_ctx: SaveContext,
) {
    for (interaction, mut bg_color, menu_button) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *bg_color = PRESSED_BUTTON.into();
                if let Some(btn) = menu_button {
                    // This click just landed on a menu button — don't let it also
                    // register as a world/inventory left-click this same frame.
                    // Without this, e.g. Resume flips the gameplay gate on and the
                    // player entity already exists (pause never despawns it), so
                    // the very click that closed the pause menu would otherwise
                    // also fire an item-use against whatever slot is selected.
                    mouse_input.clear_just_pressed(MouseButton::Left);
                    match btn.0 {
                        MenuButtonAction::Play => {
                            for root in menu_root_query.iter() {
                                commands.entity(root).despawn();
                            }
                            setup_slot_select(&mut commands, &asset_server);
                        }
                        MenuButtonAction::BackToMenu => {
                            for root in menu_root_query.iter() {
                                commands.entity(root).despawn();
                            }
                            setup_main_menu(
                                &mut commands,
                                &asset_server,
                                menu_assets.background.clone(),
                            );
                        }
                        MenuButtonAction::PlaySlot(slot) => {
                            // The lifecycle despawns the slot-select screen and its
                            // camera itself, in the same batch as spawning the player —
                            // not here, to avoid a frame with neither in place. This
                            // slot already has a save, so its own saved difficulty
                            // applies (None here).
                            play_requested.write(PlayRequested {
                                slot,
                                difficulty: None,
                            });
                        }
                        MenuButtonAction::PickDifficulty(slot) => {
                            for root in menu_root_query.iter() {
                                commands.entity(root).despawn();
                            }
                            setup_difficulty_select(&mut commands, &asset_server, slot);
                        }
                        MenuButtonAction::StartWithDifficulty(slot, difficulty) => {
                            // The lifecycle despawns this screen and its camera itself,
                            // in the same batch as spawning the player.
                            play_requested.write(PlayRequested {
                                slot,
                                difficulty: Some(difficulty),
                            });
                        }
                        MenuButtonAction::BackToSlotSelect => {
                            for root in menu_root_query.iter() {
                                commands.entity(root).despawn();
                            }
                            setup_slot_select(&mut commands, &asset_server);
                        }
                        MenuButtonAction::ResetSlot(slot) => {
                            save::delete_save(slot);
                            for root in menu_root_query.iter() {
                                commands.entity(root).despawn();
                            }
                            setup_slot_select(&mut commands, &asset_server);
                        }
                        MenuButtonAction::DeleteSlot(slot) => {
                            save::delete_save(slot);
                            for root in menu_root_query.iter() {
                                commands.entity(root).despawn();
                            }
                            setup_slot_select(&mut commands, &asset_server);
                        }
                        MenuButtonAction::Exit => {
                            exit.write(AppExit::Success);
                        }
                        MenuButtonAction::Resume => {
                            resume_status.0 = false;
                            for root in menu_root_query.iter() {
                                commands.entity(root).despawn();
                            }
                            // Coins/XP/level/quest progress are resources
                            // independent of any UI entity's lifetime, so
                            // they survived the pause menu despawning the
                            // old HUD above — `spawn_gameplay_hud` just
                            // rebuilds the display, it never touches them.
                            crate::systems::player_game_ui::spawn_gameplay_hud(
                                &mut commands,
                                &asset_server,
                            );
                        }
                        MenuButtonAction::Save => {
                            if let Some(slot) = save_ctx.active_slot.0 {
                                if let Ok((transform, player_data)) = player_query.single() {
                                    let monsters: Vec<MonsterSaveData> = save_ctx
                                        .monster_query
                                        .iter()
                                        .map(|(transform, ai)| MonsterSaveData {
                                            position: (
                                                transform.translation.x,
                                                transform.translation.y,
                                            ),
                                            health: ai.health,
                                        })
                                        .collect();
                                    let pack_escape_dir = (
                                        save_ctx.escape_model.avg_flee_dir.x,
                                        save_ctx.escape_model.avg_flee_dir.y,
                                    );
                                    save::write_save(
                                        slot,
                                        &save::capture_save_data(
                                            transform,
                                            player_data,
                                            save_ctx.active_difficulty.0,
                                            monsters,
                                            pack_escape_dir,
                                            save_ctx.escape_model.samples(),
                                            save_ctx.coins.0,
                                            save_ctx.level.level,
                                            save_ctx.level.xp,
                                            save_ctx.quests.slots.to_vec(),
                                        ),
                                    );
                                }
                            }
                        }
                        MenuButtonAction::Leave => {
                            // The lifecycle despawns this screen and its camera itself,
                            // in the same batch as spawning the main menu's own.
                            leave_requested.write(LeaveRequested);
                        }
                        MenuButtonAction::Respawn => {
                            // The lifecycle despawns the death screen and its camera
                            // itself, in the same batch as spawning the new player.
                            respawn_requested.write(RespawnRequested);
                        }
                        MenuButtonAction::Options => {
                            // no-op (placeholder)
                        }
                    }
                }
            }
            Interaction::Hovered => {
                *bg_color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                *bg_color = NORMAL_BUTTON.into();
            }
        }
    }
}
