use bevy::prelude::*;
use bevy::color::palettes::css::*;
use crate::resourses::physics_resources::*;
pub struct HudPlugin;

const SCALE: f32 = 1.5;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(InventoryState::default())
            .add_systems(Update, (update_health_bar, update_satamina_bar, handle_inventory_input, update_inventory_ui, ui_use_item).run_if(|status: Res<GameStatus>, status2: Res<ResumeStatus>| status.0 && !status2.0));
    }
}

impl Default for InventoryState {
    fn default() -> Self {
        Self {
            selected: 0,
            slots: 10,
        }
    }
}

#[derive(Component)]
struct HealthBar;

#[derive(Component)]
struct SataminaBar;

pub fn spawn_health_bar(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    // Kontener paska zdrowia
    commands
        .spawn((
            PlayerUIs,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0 * SCALE),
                left: Val::Px(10.0 * SCALE),
                width: Val::Px(200.0 * SCALE),
                height: Val::Px(20.0 * SCALE),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)), // tło
        ))
        .with_children(|builder| {
            // Pasek HP
            builder.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(RED.into()),
                HealthBar,
            ));
        });

    commands
        .spawn((
            PlayerUIs,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(40.0 * SCALE),
                left: Val::Px(10.0 * SCALE),
                width: Val::Px(175.0 * SCALE),
                height: Val::Px(10.0 * SCALE),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)), // tło
        ))
        .with_children(|builder| {
            // Pasek HP
            builder.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.0, 0.8, 0.0)),
                SataminaBar,
            ));
        });

    // Pasek punktów
    commands
        .spawn((
            PlayerUIs,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(60.0 * SCALE),
                left: Val::Px(10.0 * SCALE),
                width: Val::Px(150.0),
                height: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
        ))
        .with_children(|builder| {
            builder.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Text::new(format!("Points: {}", 0)),
                TextFont {
                    font: asset_server.load("fonts/Cantarell-Bold.ttf"),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                PointText(0),
            ));
        });
}

pub fn spawn_inventory_bar(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    // Kontener główny: pozycjonowany absolutnie, na dole, pełna szerokość
    commands
        .spawn((
            PlayerUIs,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(20.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(60.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center, // wyśrodkuj dzieci poziomo
                align_items: AlignItems::Center,         // wyśrodkuj pionowo
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|builder| {
            // Grid 10 slotów
            builder
                .spawn((
                    Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::px(10, 50.0), // 10 kolumn po 50px
                        column_gap: Val::Px(5.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ))
                .with_children(|builder| {
                    for i in 0..10 {
                        builder.spawn((
                            Node {
                                width: Val::Px(50.0),
                                height: Val::Px(50.0),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::NONE),
                            ImageNode::new(asset_server.load("textures/evil_dirt.png")),
                            Text::new(format!("{}", (i+1)%10)),
                            TextFont {
                                font: asset_server.load("fonts/Cantarell-Bold.ttf"),
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            children![(
                                Node {
                                    width: Val::Px(50.0),
                                    height: Val::Px(50.0),
                                    border: UiRect::all(Val::Px(2.0)),
                                    justify_content: JustifyContent::End,
                                    align_items: AlignItems::End,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(1.0, 1.0, 0.0, 0.75)),          // brak wypełnienia
                                InventorySlot(i),
                                InventoryImage("None".to_string()),
                                ImageNode::new(asset_server.load("textures/empty.png")),
                                children![(
                                    Text::new(""),
                                    TextFont {
                                        font: asset_server.load("fonts/Cantarell-Bold.ttf"),
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                )]
                            )],
                        ));
                    }
                });
        });
}

fn update_health_bar(
    mut player_query: Query<&PlayerData, (With<Player>, Without<Pending>)>,
    mut query: Query<&mut Node, With<HealthBar>>,
) {
    let player_data = if let Ok(d) = player_query.get_single_mut() {
        d
    } else {
        return;
    };

    if let Ok(mut bar) = query.get_single_mut() {
        let percent = (player_data.health / player_data.max_health).clamp(0.0, 1.0) * 100.0;
        bar.width = Val::Percent(percent);
    }
}

fn update_satamina_bar(
    mut player_query: Query<&PlayerData, (With<Player>, Without<Pending>)>,
    mut query: Query<&mut Node, With<SataminaBar>>,
) {
    let player_data = if let Ok(d) = player_query.get_single_mut() {
        d
    } else {
        return;
    };

    if let Ok(mut bar) = query.get_single_mut() {
        let percent = (player_data.satamina / player_data.max_satamina).clamp(0.0, 1.0) * 100.0;
        bar.width = Val::Percent(percent);
    }
}

fn handle_inventory_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InventoryState>,
) {
    /*// Otwieranie/zamykanie ekwipunku klawiszem I
    if keyboard.just_pressed(KeyCode::KeyI) {
        state.open = !state.open;
    }*/

    // Zmiana slotu klawiszami 1-0
    for i in 0..state.slots {
        let key = match i {
            0 => KeyCode::Digit1,
            1 => KeyCode::Digit2,
            2 => KeyCode::Digit3,
            3 => KeyCode::Digit4,
            4 => KeyCode::Digit5,
            5 => KeyCode::Digit6,
            6 => KeyCode::Digit7,
            7 => KeyCode::Digit8,
            8 => KeyCode::Digit9,
            9 => KeyCode::Digit0,
            _ => continue,
        };
        if keyboard.just_pressed(key) {
            state.selected = i;
        }
    }
}

// === Aktualizacja UI ekwipunku ===

fn update_inventory_ui(
    state: Res<InventoryState>,
    mut slots: Query<(&mut BackgroundColor, &mut ImageNode, &InventorySlot, &mut InventoryImage, &Children)>,
    mut text_q: Query<&mut Text>,
    query: Query<&PlayerData, With<Player>>,
    asset_server: Res<AssetServer>
) {
    // Pokaż/ukryj panel ekwipunku
    let Ok(player_data) = query.single() else {
        return;
    };

    // Podświetl aktywny slot
    for (mut color, mut image_node, slot, mut image, child) in &mut slots {
        let Ok(mut text) = text_q.get_mut(child[0]) else {
            continue;
        };
        if let Some(item) = player_data.inventory.get_item(slot.0 as u32) {
            if image.0 != item.id {
                image.0 = item.id.clone();
                *image_node = ImageNode::new(asset_server.load(&item.path));
                if item.amount > 0 {
                    *text = Text::new(format!("{}", item.amount));
                } else {
                    *text = Text::new("");
                }
            }
        } else {
            if image.0 != "None" {
                image.0 = "None".to_string();
                *image_node = ImageNode::new(asset_server.load("textures/empty.png"));
                *text = Text::new("");
            }
        }
        if slot.0 == state.selected {
            *color = BackgroundColor(Color::srgba(1.0, 0.0, 0.0, 0.25));
        } else {
            *color = BackgroundColor(Color::srgba(1.0, 1.0, 0.0, 0.25));
        }
    }
}

fn ui_use_item(
    mut ev_consume: EventWriter<ConsumeEvent>,
    mut ev_func: EventWriter<FunctionalEvent>,
    //keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<InventoryState>,
    query: Query<&PlayerData, With<Player>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        let Ok(player_data) = query.single() else {
            return;
        };
        if let Some(item) = player_data.inventory.get_item(state.selected as u32) {
            match item.item_type.as_str() {
                "food" => {
                    ev_consume.send(ConsumeEvent {
                        slot: state.selected as u32,
                        item_id: item.id.clone(),
                    });
                }
                "weapon" => {
                    ev_func.send(FunctionalEvent {
                        slot: state.selected as u32,
                        item_id: item.id.clone(),
                    });
                }
                _ => {} // opcjonalnie dla innych typów
            }
        }
    }
}