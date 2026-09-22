mod resourses;
mod systems;

use bevy::prelude::*;
use rapier2d::prelude::*;
use resourses::physics_resources::*;
use systems::lifecycle::GameLifecyclePlugin;
use systems::loader::ObjectsLoaderPlugin;
use systems::menu_ui::MenuPlugin;
use systems::monster::MonsterPlugin;
use systems::physics::PhysicsPlugin;
use systems::player::PlayerPlugin;
use systems::player_game_ui::HudPlugin;
use systems::terrain::TerrainGenerationPlugin;
//use bevy_light_2d::prelude::*;
use bevy_2d_screen_space_lightmaps::lightmap_plugin::lightmap_plugin::LightmapPlugin;
use bevy_firefly::prelude::*;
use std::collections::HashMap;
use systems::eventer::EventerPlugin;
use systems::items::WorldItemsPlugin;
use systems::progression::ProgressionPlugin;
use systems::quests::QuestPlugin;

use bevy::window::{MonitorSelection, WindowMode};
use std::path::Path;

use image::{DynamicImage, GenericImage, GenericImageView, ImageBuffer, Rgba};

use std::fs;

fn load_items_config(mut commands: Commands) {
    let data = fs::read_to_string("assets/config/items.json")
        .expect("Nie można wczytać pliku konfiguracyjnego");

    let config: ItemConfig = serde_json::from_str(&data).expect("Błąd parsowania pliku JSON");

    commands.insert_resource(config);
}

fn main() {
    /*{
        let sprite1 = image::open("assets/textures/monster1.png").unwrap();
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
        new_image.save("assets/textures/monster_combined.png").unwrap();
    }*/
    /*{
        let sprite1 = image::open("assets/textures/player_sprite.png").unwrap();
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
        new_image.save("assets/textures/player_combined.png").unwrap();
    }*/
    /* Monster 2's combined sheet — same vertical-stack pipeline as above
       (walk on top, attack in the middle, jump on the bottom), plus two
       fixes the source art needed that the other sheets didn't:
        1. `monster2_attack.png` (512x288, 128x144 frames) is wider than
           `monster2.png`/`monster2_jump.png` (320x160, 80x80 frames) — it's
           scaled down by one uniform factor (both axes, 0.625x) to 320x180
           so every section shares Monster 1's convention of one consistent
           width, not stretched/squeezed on one axis.
        2. Monster 2's source art faces right by default; Monster 1's
           (`monster1.png`/`monster_attack.png`) faces left, which is what
           the shared flip logic in `monster::monster_ai` assumes (it
           mirrors `rb_transform.scale.x` moving right, leaves it unflipped
           moving left). Every frame is mirrored in place (position/order
           unchanged, only content) so Monster 2 doesn't walk backwards.
       See `docs/monster2.md` for the resulting layout/offsets.
    {
        use image::imageops;

        fn flip_grid_in_place(img: &image::DynamicImage, cols: u32, rows: u32) -> image::DynamicImage {
            let (w, h) = (img.width(), img.height());
            let (cell_w, cell_h) = (w / cols, h / rows);
            let mut out = ImageBuffer::new(w, h);
            for row in 0..rows {
                for col in 0..cols {
                    let (x, y) = (col * cell_w, row * cell_h);
                    let cell = img.view(x, y, cell_w, cell_h).to_image();
                    out.copy_from(&imageops::flip_horizontal(&cell), x, y).unwrap();
                }
            }
            image::DynamicImage::ImageRgba8(out)
        }

        let top = image::open("assets/textures/monster2.png").unwrap();
        let middle_raw = image::open("assets/textures/monster2_attack.png").unwrap();
        let bottom = image::open("assets/textures/monster2_jump.png").unwrap();

        let scale = top.width() as f64 / middle_raw.width() as f64;
        let middle_h = (middle_raw.height() as f64 * scale).round() as u32;
        let middle = middle_raw.resize_exact(top.width(), middle_h, imageops::FilterType::Nearest);

        let top = flip_grid_in_place(&top, 4, 2);
        let middle = flip_grid_in_place(&middle, 4, 2);
        let bottom = flip_grid_in_place(&bottom, 4, 2);

        let width = top.width().max(middle.width()).max(bottom.width());
        let height = top.height() + middle.height() + bottom.height();

        let mut combined = ImageBuffer::new(width, height);
        combined.copy_from(&top, 0, 0).unwrap();
        combined.copy_from(&middle, 0, top.height()).unwrap();
        combined.copy_from(&bottom, 0, top.height() + middle.height()).unwrap();

        combined.save("assets/textures/monster2_combined.png").unwrap();
    }*/
    let mut app = App::new();
    app.insert_resource(AtlasHandles(HashMap::new()))
        .insert_resource(ClearColor(Color::NONE))
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: (1920_u32, 1080_u32).into(),
                        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            //Light2dPlugin,
            //ScreenSpaceLightmapPlugin,
            //LightmapPlugin,
            FireflyPlugin,
            MenuPlugin,
            HudPlugin,
            PhysicsPlugin,
            ObjectsLoaderPlugin,
            PlayerPlugin,
            MonsterPlugin,
            TerrainGenerationPlugin,
            EventerPlugin,
            WorldItemsPlugin,
            GameLifecyclePlugin,
            QuestPlugin,
            ProgressionPlugin,
        ))
        .add_systems(Startup, load_items_config);
    app.run();
}
