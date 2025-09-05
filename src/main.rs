mod setup;
mod physics;
mod physics_resources; 

use bevy::prelude::*;
use rapier2d::prelude::*;
use physics_resources::*;
use setup::SetupPlugin;
use physics::PhysicsPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        PhysicsPlugin,
        SetupPlugin,
    ));
    app.run();
}