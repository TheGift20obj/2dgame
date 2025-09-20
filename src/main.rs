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

use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, DiagnosticsStore};

fn main() {
    let mut app = App::new();
    app.add_systems(Update, show_fps).add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        FrameTimeDiagnosticsPlugin::default(),
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


fn show_fps(diagnostics: Res<DiagnosticsStore>) {
    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
            println!("FPS: {:.2}", value);
        }
    }
}