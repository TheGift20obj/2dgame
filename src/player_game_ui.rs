use bevy::prelude::*;
use bevy::color::palettes::css::*;
use crate::physics_resources::*;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app
            //.add_systems(Startup, (spawn_health_bar, spawn_inventory_bar))
            .add_systems(Update, (update_health_bar, update_satamina_bar).run_if(|status: Res<GameStatus>| status.0));
    }
}

#[derive(Component)]
struct HealthBar;

#[derive(Component)]
struct SataminaBar;

pub fn spawn_health_bar(commands: &mut Commands) {
    // Kontener paska zdrowia
    commands
        .spawn((
            PlayerUIs,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                width: Val::Px(200.0),
                height: Val::Px(20.0),
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
                top: Val::Px(40.0),
                left: Val::Px(10.0),
                width: Val::Px(175.0),
                height: Val::Px(10.0),
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
}

pub fn spawn_inventory_bar(commands: &mut Commands) {
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
                    for _ in 0..10 {
                        builder.spawn((
                            Node {
                                width: Val::Px(50.0),
                                height: Val::Px(50.0),
                                ..default()
                            },
                            BackgroundColor(GRAY.into()), // slot
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