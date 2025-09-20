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

use bevy_light_2d::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        Light2dPlugin,
        MenuPlugin,
        HudPlugin,
        PhysicsPlugin,
        ObjectsLoaderPlugin,
        PlayerPlugin,
        MonsterPlugin,
        TerrainGenerationPlugin,
    ));
    app.run();
}