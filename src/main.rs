mod physics;
mod monster;
mod physics_resources; 
mod player;
mod terrain;
mod loader;
mod player_game_ui;
mod menu_ui;

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

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        MenuPlugin,
        HudPlugin,
        PhysicsPlugin,
        ObjectsLoaderPlugin,
        TerrainGenerationPlugin,
        PlayerPlugin,
        MonsterPlugin,
    ));
    app.run();
}