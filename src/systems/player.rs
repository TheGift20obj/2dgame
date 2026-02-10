use bevy::prelude::*;
use crate::resourses::physics_resources::*;

use rapier2d::prelude::*;
use rapier2d::na::Point2;

pub struct PlayerPlugin;
use bevy::window::{PrimaryWindow, Window};
use bevy_2d_screen_space_lightmaps::lightmap_plugin::lightmap_plugin::*;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::render::view::Hdr;
use bevy_firefly::prelude::*;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        //app.add_systems(Startup, init);
        app.add_systems(Update, (update, animate_sprite, try_heal).run_if(|status: Res<GameStatus>, status2: Res<ResumeStatus>| status.0 && !status2.0) );
    }
}

pub fn init(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    asset_server: &Res<AssetServer>,
    texture_atlas_layouts: &mut ResMut<Assets<TextureAtlasLayout>>,
    images: &Res<Assets<Image>>,
    config: &Res<ItemConfig>,
    atlas_handles: &Res<AtlasHandles>,
) {
    let texture = asset_server.load("textures/player_combined.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 2, 5, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);
    let animation_indices = atlas_handles.0.get("walk").unwrap().clone();
    /*commands.spawn((
        PointLight2d {
            color: Color::srgb(1.0, 0.0, 0.0),
            range: 1000.0,
            ..default()
        },
        Transform::from_translation(vec3(0.0, 0.0, 1.0)),
    ));
     
    // spawn a circle occluder
    commands.spawn((
        Occluder2d::circle(10.0),
        Transform::from_translation(vec3(0.0, 50.0, 1.0)),
    ));*/
    commands.spawn((
        Camera2d,
        //LightCamera, AnyNormalCamera,
        //RenderLayers::from_layers(CAMERA_LAYER_LIGHT),
        PointLight2d {
            range: 750.0,
            intensity: 0.125,
            color: Color::WHITE,
            ..default()
        },
        FireflyConfig::default(),
        Player,
        Pending,
        RenderLayers::from_layers(CAMERA_LAYER_EFFECT),
        PlayerData::new(config),
        Mesh2d(meshes.add(Rectangle::new(50.0, 37.5))),
        Transform::from_xyz(
            0.0,
            0.0,
            0.0,
        ),
        children![(
            Sprite::from_atlas_image(
                texture,
                TextureAtlas {
                    layout: texture_atlas_layout,
                    index: animation_indices.first,
                },
            ),
            YSort { z: 0.0 },
            Transform::from_xyz(0.0, 43.0, 65.0).with_scale(Vec3::splat(2.5)),
            RenderLayers::from_layers(CAMERA_LAYER_ENTITY),
            animation_indices,
            AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
            PlayerSprite,
            AttackStatus(false),
        )]
    ));
}

fn animate_sprite(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut AnimationIndices, &mut AnimationTimer, &mut Sprite, &mut Transform, &mut AttackStatus), With<PlayerSprite>>,
    asset_server: Res<AssetServer>,
    atlas_handles: Res<AtlasHandles>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    for (mut indices, mut timer, mut sprite, mut transform, mut atack) in &mut query {
        timer.tick(time.delta());

        // Sprawdzenie czy gracz naciska klawisze ruchu
        let mut moving = false;
        let mut direction: f32 = 1.0; // 1 = prawo, -1 = lewo

        if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::KeyS) {
            if transform.scale.x < 0.0 {
                direction = -1.0;
            } else {
                direction = 1.0;
            }
            moving = true;
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            moving = true;
            direction = -1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            moving = true;
            direction = 1.0;
        }
        if atack.0 {
            direction = transform.scale.x.signum();
        }

        if moving || atack.0 {
            if atack.0 {
                let window = match windows.single() {
                    Ok(w) => w,
                    Err(_) => return, // brak okna głównego
                };

                if let Some(cursor_pos) = window.cursor_position() {
                    let screen_center_x = window.width() / 2.0;
                    let direction = if cursor_pos.x > screen_center_x { 1.0 } else { -1.0 };
                    transform.scale.x = direction * transform.scale.x.abs();
                }
            } else {
                transform.scale.x = direction * transform.scale.x.abs();
            }
            if timer.just_finished() {
                let mut last = false;
                if let Some(atlas) = &mut sprite.texture_atlas {
                    atlas.index = if atlas.index == indices.last {
                        if atack.0 {
                            last = true;
                        }
                        indices.first
                    } else {
                        atlas.index + 1
                    };
                }
                if last {
                    if atack.0 {
                        atack.0 = false;
                        let animation_indices = atlas_handles.0.get("walk").unwrap().clone();
                        if let Some(atlas) = &mut sprite.texture_atlas {
                            atlas.index = animation_indices.first;
                        }
                        *indices = animation_indices;
                        timer.reset();
                    }
                }
            }
        } else {
            // Jeśli gracz stoi, ustaw indeks na 0
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = indices.first;
            }
        }
    }
}

fn update(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&RigidBodyHandleComponent, &mut Transform, &mut PlayerData), (With<Player>, Without<Pending>)>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut sprite_camera: Query<&mut Transform, (With<SpriteCamera>, Without<Pending>, Without<Player>)>,
    mut light_camera: Query<&mut Transform, (With<LightCamera>, Without<Pending>, Without<Player>, Without<SpriteCamera>)>,
) {
    let Ok((handle, mut transform, mut player_data)) = query.single_mut() else {
        return;
    };

    if !rigid_bodies.0.get_mut(handle.0).is_some() {
        return;
    }

    let mut rigidbody = rigid_bodies.0.get_mut(handle.0).unwrap();

    let mut dir = Vec2::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) { dir.y += 1.0; }
    if keyboard_input.pressed(KeyCode::KeyS) { dir.y -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyA) { dir.x -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyD) { dir.x += 1.0; }

    let mut speed = 200.0;

    if keyboard_input.pressed(KeyCode::ShiftLeft) && dir != Vec2::ZERO { 
        player_data.run(1.25, &time);
        speed = 350.0; 
    } else {
        player_data.rest(0.375, &time);
    }

    if player_data.satamina <= player_data.min_satamina {
        speed *= player_data.fatigue();
    }

    let velocity = if dir.length_squared() > 0.0 {
        dir.normalize() * speed
    } else {
        Vec2::ZERO
    };

    rigidbody.set_linvel(vector![velocity.x, velocity.y], true);
    //transform.translation.z = -(((WORLD_SIZE as f32*TILE_SIZE)/2.0)/64.0 + rigidbody.translation().y.round()/64.0) + 64.0;
}

fn try_heal(
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
    mut island_manager: ResMut<ResIslandManager>,
    mut commands: Commands,
    time: Res<Time>,
    mut player_query: Query<(Entity, &Transform, &mut PlayerData, &RigidBodyHandleComponent), (With<Player>, Without<Pending>)>,
    mut player_ui_query: Query<Entity, With<PlayerUIs>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    images: Res<Assets<Image>>,
    mut game_status: ResMut<GameStatus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut resume_status: ResMut<ResumeStatus>,
) {
    let (entity, transform, mut player_data, handle) = if let Ok((e, t, mut d, rb_handle)) = player_query.single_mut() {
        (e, t, d, rb_handle)
    } else {
        return;
    };

    if keyboard_input.just_pressed(KeyCode::Escape) {
        resume_status.0 = true;
        for ui_entity in player_ui_query {
            commands.entity(ui_entity).despawn();
        }
        crate::systems::menu_ui::setup_ui(&mut commands, &asset_server);
        return;
    }

    if player_data.can_heal.just_finished() && player_data.health < player_data.max_health && player_data.health > 0.0 {
        player_data.heal(1.0, &time);
    } else if player_data.health == 0.0 {
        game_status.0 = false;
        let mut colliders_clone = Vec::new();
        if let Some(rb) = rigid_bodies.0.get(handle.0) {
            for collider_handle in rb.colliders() {
                colliders_clone.push(collider_handle.clone());
            }
        }

        for collider_handle in colliders_clone {
            colliders.0.remove(collider_handle, &mut island_manager.0, &mut rigid_bodies.0, true);
        }
        rigid_bodies.0.remove(
            handle.0,
            &mut island_manager.0,
            &mut colliders.0,
            &mut ImpulseJointSet::new(),
            &mut MultibodyJointSet::new(),
            true, // usuwa powiązane collidery
        );
        commands.entity(entity).despawn();
        for ui_entity in player_ui_query {
            commands.entity(ui_entity).despawn();
        }
        crate::systems::menu_ui::setup_ui(&mut commands, &asset_server);
        commands.spawn((
            //Camera2d,
            SpriteCamera, AnyNormalCamera,
            MenuCamera
        ));
        //commands.spawn((Camera2d, Transform {translation: transform.translation, ..default()}));
        /*let texture = asset_server.load("textures/player_sprite.png");
        let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 2, 2, None, None);
        let texture_atlas_layout = texture_atlas_layouts.add(layout);
        let animation_indices = AnimationIndices { first: 0, last: 3 };
        commands.spawn((
            Camera2d,
            Player,
            Pending,
            PlayerData::new(),
            Mesh2d(meshes.add(Rectangle::new(50.0, 25.0))),
            Transform::from_xyz(
                0.0,
                0.0,
                1.0,
            ),
            children![(
                Sprite::from_atlas_image(
                    texture,
                    TextureAtlas {
                        layout: texture_atlas_layout,
                        index: animation_indices.first,
                    },
                ),
                Transform::from_xyz(0.0, 43.0, 0.0).with_scale(Vec3::splat(2.5)),
                animation_indices,
                AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
                PlayerSprite,
            )]
        ));*/
        return;
    }
    player_data.can_heal.tick(time.delta());
}