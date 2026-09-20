use crate::resourses::physics_resources::*;
use bevy::color::palettes::css::*;
use bevy::prelude::*;

pub struct HudPlugin;
const SLOT: f32 = 58.0;

/// Item currently carried by the inventory cursor. It is removed from its
/// source slot immediately, so a second click can place it in any slot or
/// drop it into the world.
#[derive(Resource, Default)]
struct HeldInventoryItem(Option<Item>);

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(InventoryState::default())
            .insert_resource(HeldInventoryItem::default())
            .add_systems(
                Update,
                (
                    update_health,
                    update_stamina,
                    update_score,
                    toggle_inventory,
                    update_inventory,
                    interact_with_inventory,
                    use_hotbar_item,
                )
                    .run_if(|game: Res<GameStatus>, pause: Res<ResumeStatus>| game.0 && !pause.0)
                    .in_set(crate::systems::lifecycle::AppSet::Gameplay),
            );
    }
}

impl Default for InventoryState {
    fn default() -> Self {
        Self {
            selected: 0,
            slots: 10,
            open: false,
        }
    }
}

pub fn spawn_health_bar(commands: &mut Commands, assets: &Res<AssetServer>, points: u32) {
    spawn_stat(
        commands,
        Vec2::new(10., 10.),
        Vec2::new(300., 28.),
        RED.into(),
        HealthBar,
    );
    spawn_stat(
        commands,
        Vec2::new(10., 48.),
        Vec2::new(260., 14.),
        Color::srgb(0., 0.8, 0.).into(),
        SataminaBar,
    );
    commands.spawn((
        PlayerUIs,
        GameplayHud,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(72.),
            left: Val::Px(10.),
            ..default()
        },
        Text::new(format!("Points: {points}")),
        TextFont {
            font: assets.load("fonts/Cantarell-Bold.ttf"),
            font_size: 18.,
            ..default()
        },
        TextColor(Color::WHITE),
        PointText(points),
    ));
}

fn spawn_stat<T: Component>(
    commands: &mut Commands,
    at: Vec2,
    size: Vec2,
    color: BackgroundColor,
    marker: T,
) {
    commands.spawn((
        PlayerUIs,
        GameplayHud,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(at.y),
            left: Val::Px(at.x),
            width: Val::Px(size.x),
            height: Val::Px(size.y),
            ..default()
        },
        BackgroundColor(Color::srgb(0.08, 0.08, 0.08)),
        children![(
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                ..default()
            },
            color,
            marker
        )],
    ));
}

pub fn spawn_inventory_bar(commands: &mut Commands, assets: &Res<AssetServer>) {
    commands
        .spawn((
            PlayerUIs,
            InventoryHotbar,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(22.),
                left: Val::Percent(50.),
                margin: UiRect::left(Val::Px(-325.)),
                width: Val::Px(650.),
                height: Val::Px(72.),
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(10, SLOT),
                column_gap: Val::Px(8.),
                padding: UiRect::all(Val::Px(7.)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.88)),
        ))
        .with_children(|parent| {
            for slot in 0..10 {
                spawn_slot(parent, assets, slot, true);
            }
        });
    commands
        .spawn((
            PlayerUIs,
            InventoryOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                display: Display::None,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0., 0., 0., 0.68)),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(720.),
                        height: Val::Px(390.),
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::px(10, SLOT),
                        grid_template_rows: RepeatedGridTrack::px(4, SLOT),
                        column_gap: Val::Px(10.),
                        row_gap: Val::Px(12.),
                        padding: UiRect::all(Val::Px(20.)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.10, 0.13)),
                ))
                .with_children(|grid| {
                    for row in 0..4 {
                        for col in 0..10 {
                            let slot = if row == 3 { col } else { 10 + row * 10 + col };
                            spawn_slot(grid, assets, slot, row == 3);
                        }
                    }
                });
        });
}

fn spawn_slot(
    parent: &mut ChildSpawnerCommands,
    assets: &Res<AssetServer>,
    slot: usize,
    hotbar: bool,
) {
    let background = if hotbar {
        Color::srgba(0.40, 0.31, 0.10, 0.96)
    } else {
        Color::srgba(0.20, 0.20, 0.25, 0.96)
    };
    parent.spawn((
        Node {
            width: Val::Px(SLOT),
            height: Val::Px(SLOT),
            justify_content: JustifyContent::End,
            align_items: AlignItems::End,
            ..default()
        },
        BackgroundColor(background),
        Button,
        InventorySlot(slot),
        InventoryImage("None".into()),
        ImageNode::new(assets.load("textures/empty.png")),
        children![(
            Text::new(""),
            TextFont {
                font: assets.load("fonts/Cantarell-Bold.ttf"),
                font_size: 16.,
                ..default()
            },
            TextColor(Color::WHITE)
        )],
    ));
}

fn toggle_inventory(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InventoryState>,
    mut ui_nodes: Query<(
        &mut Node,
        Option<&InventoryHotbar>,
        Option<&InventoryOverlay>,
        Option<&GameplayHud>,
    )>,
) {
    if keyboard.just_pressed(KeyCode::KeyE) {
        state.open = !state.open;
        for (mut node, hotbar, overlay, hud) in &mut ui_nodes {
            if hotbar.is_some() {
                node.display = if state.open {
                    Display::None
                } else {
                    Display::Grid
                };
            } else if overlay.is_some() {
                node.display = if state.open {
                    Display::Flex
                } else {
                    Display::None
                };
            } else if hud.is_some() {
                node.display = if state.open {
                    Display::None
                } else {
                    Display::Flex
                };
            }
        }
    }
    if !state.open {
        let keys = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
            KeyCode::Digit0,
        ];
        for (slot, key) in keys.into_iter().enumerate() {
            if keyboard.just_pressed(key) {
                state.selected = slot;
            }
        }
    }
}

fn update_inventory(
    mut slots: Query<(
        &mut ImageNode,
        &InventorySlot,
        &mut InventoryImage,
        &Children,
    )>,
    mut texts: Query<&mut Text>,
    player: Query<&PlayerData, With<Player>>,
    assets: Res<AssetServer>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    for (mut icon, slot, mut cache, children) in &mut slots {
        let Ok(mut text) = texts.get_mut(children[0]) else {
            continue;
        };
        if let Some(item) = player.inventory.get_item(slot.0 as u32) {
            if cache.0 != item.id {
                cache.0 = item.id.clone();
                *icon = ImageNode::new(assets.load(&item.path));
            }
            *text = Text::new(if item.id == "sword_basic" {
                String::new()
            } else {
                item.amount.to_string()
            });
        } else if cache.0 != "None" {
            cache.0 = "None".into();
            *icon = ImageNode::new(assets.load("textures/empty.png"));
            *text = Text::new("");
        }
    }
}
fn update_health(
    p: Query<&PlayerData, (With<Player>, Without<Pending>)>,
    mut q: Query<&mut Node, With<HealthBar>>,
) {
    if let (Ok(p), Ok(mut n)) = (p.single(), q.single_mut()) {
        n.width = Val::Percent((p.health / p.max_health * 100.).clamp(0., 100.));
    }
}
fn update_stamina(
    p: Query<&PlayerData, (With<Player>, Without<Pending>)>,
    mut q: Query<&mut Node, With<SataminaBar>>,
) {
    if let (Ok(p), Ok(mut n)) = (p.single(), q.single_mut()) {
        n.width = Val::Percent((p.satamina / p.max_satamina * 100.).clamp(0., 100.));
    }
}
fn update_score(score: Res<Score>, mut q: Query<(&mut Text, &mut PointText)>) {
    if score.is_changed() {
        for (mut t, mut p) in &mut q {
            p.0 = score.0;
            *t = Text::new(format!("Points: {}", score.0));
        }
    }
}

fn interact_with_inventory(
    state: Res<InventoryState>,
    mouse: Res<ButtonInput<MouseButton>>,
    clicked_slots: Query<(&Interaction, &InventorySlot), (Changed<Interaction>, With<Button>)>,
    all_slots: Query<&Interaction, With<InventorySlot>>,
    mut held: ResMut<HeldInventoryItem>,
    mut player: Query<
        (&Transform, &FacingDirection, &mut PlayerData),
        (With<Player>, Without<Pending>),
    >,
    mut commands: Commands,
    assets: Res<AssetServer>,
) {
    if !state.open {
        return;
    }
    let Ok((transform, facing, mut data)) = player.single_mut() else {
        return;
    };

    let take_one = mouse.pressed(MouseButton::Right);
    let mut clicked_a_slot = false;
    for (interaction, slot) in &clicked_slots {
        if *interaction != Interaction::Pressed {
            continue;
        }
        clicked_a_slot = true;
        let slot = slot.0 as u32;
        if held.0.is_none() {
            held.0 = if take_one {
                data.inventory.remove_one(slot)
            } else {
                data.inventory.remove_item(slot)
            };
            continue;
        }

        let held_item = held.0.take().expect("held item checked above");
        match data.inventory.items.get_mut(&slot) {
            None => {
                data.inventory.items.insert(slot, held_item);
            }
            Some(existing) if existing.id == held_item.id && existing.id != "sword_basic" => {
                existing.amount = existing.amount.saturating_add(held_item.amount);
            }
            Some(existing) => {
                let previous = std::mem::replace(existing, held_item);
                held.0 = Some(previous);
            }
        }
    }

    if !clicked_a_slot
        && (mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right))
        && !all_slots
            .iter()
            .any(|interaction| *interaction == Interaction::Pressed)
    {
        let Some(item) = held.0.take() else {
            return;
        };
        // BLOKADA MIECZA: może zmieniać slot, ale nie może zostać wyrzucony.
        if item.id == "sword_basic" {
            held.0 = Some(item);
            return;
        }
        let position = transform.translation.xy() + facing.0.normalize_or_zero() * 72.0;
        crate::systems::items::spawn_world_item(&mut commands, &assets, item, position);
    }
}

fn use_hotbar_item(
    mut food: ResMut<Messages<ConsumeEvent>>,
    mut functional: ResMut<Messages<FunctionalEvent>>,
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<InventoryState>,
    player: Query<&PlayerData, With<Player>>,
) {
    if state.open || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(player) = player.single() else {
        return;
    };
    let Some(item) = player.inventory.get_item(state.selected as u32) else {
        return;
    };
    match item.item_type.as_str() {
        "food" => {
            food.write(ConsumeEvent {
                slot: state.selected as u32,
                item_id: item.id.clone(),
            });
        }
        "weapon" => {
            functional.write(FunctionalEvent {
                slot: state.selected as u32,
                item_id: item.id.clone(),
            });
        }
        _ => {}
    }
}
