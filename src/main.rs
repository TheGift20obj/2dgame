mod setup;
mod physics;
mod monster;
mod physics_resources; 

use bevy::prelude::*;
use rapier2d::prelude::*;
use physics_resources::*;
use setup::SetupPlugin;
use monster::MonsterPlugin;
use physics::PhysicsPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        PhysicsPlugin,
        SetupPlugin,
        MonsterPlugin,
    ));
    app.run();
}