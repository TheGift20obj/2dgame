mod physics;
mod monster;
mod physics_resources; 
mod player;
mod terrain;
mod loader;

use bevy::prelude::*;
use rapier2d::prelude::*;
use physics_resources::*;

use monster::MonsterPlugin;
use physics::PhysicsPlugin;
use player::PlayerPlugin;
use terrain::TerrainGenerationPlugin;
use loader::ObjectsLoaderPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        PhysicsPlugin,
        ObjectsLoaderPlugin,
        TerrainGenerationPlugin,
        PlayerPlugin,
        MonsterPlugin,
    ));
    app.run();
}