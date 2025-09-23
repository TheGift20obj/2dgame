mod physics;
mod monster;
mod physics_resources; 
mod player;
mod terrain;
mod loader;
mod player_game_ui;
mod menu_ui;
mod eventer;

use bevy::prelude::*;
use rapier2d::prelude::*;
use physics_resources::*;
use menu_ui::MenuPlugin;
use player_game_ui::HudPlugin;
use monster::MonsterPlugin;
use physics::PhysicsPlugin;
use player::PlayerPlugin;
use terrain::TerrainGenerationPlugin;
use loader::ObjectsLoaderPlugin;
use bevy_light_2d::prelude::*;
use eventer::EventerPlugin;

use bevy::window::{WindowMode, MonitorSelection};

use std::fs;

fn load_items_config(mut commands: Commands) {
    let data = fs::read_to_string("assets/config/items.json")
        .expect("Nie można wczytać pliku konfiguracyjnego");

    let config: ItemConfig =
        serde_json::from_str(&data).expect("Błąd parsowania pliku JSON");

    commands.insert_resource(config);
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1920., 1080.).into(),
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }).set(ImagePlugin::default_nearest()),
        Light2dPlugin,
        MenuPlugin,
        HudPlugin,
        PhysicsPlugin,
        ObjectsLoaderPlugin,
        PlayerPlugin,
        MonsterPlugin,
        TerrainGenerationPlugin,
        EventerPlugin,
    )).add_systems(Startup, load_items_config);
    app.run();
}