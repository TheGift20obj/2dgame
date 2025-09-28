/*mod physics;
mod monster;
mod physics_resources; 
mod player;
mod terrain;
mod loader;
mod player_game_ui;
mod menu_ui;
mod eventer;*/

mod systems;
mod resourses;

use bevy::prelude::*;
use rapier2d::prelude::*;
use resourses::physics_resources::*;
use systems::menu_ui::MenuPlugin;
use systems::player_game_ui::HudPlugin;
use systems::monster::MonsterPlugin;
use systems::physics::PhysicsPlugin;
use systems::player::PlayerPlugin;
use systems::terrain::TerrainGenerationPlugin;
use systems::loader::ObjectsLoaderPlugin;
use bevy_light_2d::prelude::*;
use systems::eventer::EventerPlugin;
use std::collections::HashMap;

use bevy::window::{WindowMode, MonitorSelection};

use image::{DynamicImage, GenericImage, GenericImageView, ImageBuffer, Rgba};

use std::fs;

fn load_items_config(mut commands: Commands) {
    let data = fs::read_to_string("assets/config/items.json")
        .expect("Nie można wczytać pliku konfiguracyjnego");

    let config: ItemConfig =
        serde_json::from_str(&data).expect("Błąd parsowania pliku JSON");

    commands.insert_resource(config);
}

fn main() {
    {let sprite1 = image::open("assets/textures/monster1.png").unwrap();
    let sprite2 = image::open("assets/textures/monster_attack.png").unwrap();

    // Wyznacz wymiary nowego obrazka
    let width = sprite1.width().max(sprite2.width());
    let height = sprite1.height() + sprite2.height();

    // Stwórz nowy obraz RGBA
    let mut new_image = ImageBuffer::new(width, height);

    // Wklej pierwszy sprite (na górze)
    new_image.copy_from(&sprite1, 0, 0).unwrap();

    // Wklej drugi sprite (pod pierwszym)
    new_image.copy_from(&sprite2, 0, sprite1.height()).unwrap();

    // Zapisz nowy obrazek
    new_image.save("assets/textures/monster_combined.png").unwrap();}
    {let sprite1 = image::open("assets/textures/player_sprite.png").unwrap();
    let sprite2 = image::open("assets/textures/player_attack.png").unwrap();

    // Wyznacz wymiary nowego obrazka
    let width = sprite1.width().max(sprite2.width());
    let height = sprite1.height() + sprite2.height();

    // Stwórz nowy obraz RGBA
    let mut new_image = ImageBuffer::new(width, height);

    // Wklej pierwszy sprite (na górze)
    new_image.copy_from(&sprite1, 0, 0).unwrap();

    // Wklej drugi sprite (pod pierwszym)
    new_image.copy_from(&sprite2, 0, sprite1.height()).unwrap();

    // Zapisz nowy obrazek
    new_image.save("assets/textures/player_combined.png").unwrap();}
    let mut app = App::new();
    app.insert_resource(AtlasHandles(HashMap::new())).add_plugins((
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