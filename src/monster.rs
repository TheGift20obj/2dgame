use bevy::prelude::*;
use rapier2d::prelude::*;

use crate::physics_resources::*;
use crate::terrain::{WORLD_SIZE, TILE_SIZE};

#[derive(Component)]
pub struct MonsterAI {
    pub target_player: bool,
    pub random_timer: Timer,
    pub random_dir: Vec2,
    pub action_timer: Timer,
    pub action_cooldown: Timer,
}

#[derive(Resource)]
struct MonsterSpawnTimer(Timer);

#[derive(Resource)]
pub struct MonsterConfig {
    pub min_spawn_distance: f32,   // w tileach
    pub max_despawn_distance: f32, // w tileach
    pub max_monsters: usize,
    pub world_size_x: usize,       // w tileach
    pub world_size_y: usize,       // w tileach
    pub tile_size: f32,            // piksele
}

pub struct MonsterPlugin;

impl Plugin for MonsterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MonsterSpawnTimer(Timer::from_seconds(1.0, TimerMode::Repeating)))
           .insert_resource(MonsterConfig {
               min_spawn_distance: 20.0,   // spawn 20 kratek
               max_despawn_distance: 30.0, // despawn 30 kratek
               max_monsters: 10,
               world_size_x: (WORLD_SIZE/3) as usize,
               world_size_y: (WORLD_SIZE/3) as usize,
               tile_size: 64.0,
           })
           .add_systems(Update, spawn_monsters_system)
           .add_systems(Update, (monster_ai, animate_monster_sprite));
    }
}

fn spawn_monsters_system(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<MonsterSpawnTimer>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    player_query: Query<&Transform, With<Player>>,
    existing_monsters: Query<Entity, With<Monster>>,
    config: Res<MonsterConfig>,
) {
    timer.0.tick(time.delta());
    if !timer.0.finished() {
        return;
    }

    let current_count = existing_monsters.iter().count();
    if current_count >= config.max_monsters {
        return;
    }

    let player_transform = if let Ok(t) = player_query.get_single() {
        t
    } else {
        return;
    };

    let texture = asset_server.load("textures/monster1.png");
    let layout = TextureAtlasLayout::from_grid(bevy::prelude::UVec2::splat(64), 2, 2, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    let spawn_distance = config.min_spawn_distance * config.tile_size;
    let to_spawn = config.max_monsters - current_count;

    // granice mapy w pikselach
    let map_min_x = -(config.world_size_x as f32 * config.tile_size) / 2.0;
    let map_max_x = (config.world_size_x as f32 * config.tile_size) / 2.0;
    let map_min_y = -(config.world_size_y as f32 * config.tile_size) / 2.0;
    let map_max_y = (config.world_size_y as f32 * config.tile_size) / 2.0;

    for _ in 0..to_spawn {
        let mut pos;
        let mut attempts = 0;
        loop {
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            let distance = spawn_distance + rand::random::<f32>() * (10.0 * config.tile_size); // od 20 do 30 kratek
            pos = Vec2::new(
                player_transform.translation.x + distance * angle.cos(),
                player_transform.translation.y + distance * angle.sin(),
            );
            // sprawdź czy w granicach mapy
            if pos.x >= map_min_x && pos.x <= map_max_x && pos.y >= map_min_y && pos.y <= map_max_y {
                break;
            }
            attempts += 1;
            if attempts > 5 { break; } // unikamy nieskończonej pętli
        }

        let monster_animation_indices = AnimationIndices { first: 0, last: 3 };

        commands.spawn((
            Monster,
            MonsterAI {
                target_player: false,
                random_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
                random_dir: Vec2::ZERO,
                action_timer: Timer::from_seconds(0.25, TimerMode::Once),
                action_cooldown: Timer::from_seconds(2.0, TimerMode::Once),
            },
            Pending,
            Mesh2d(meshes.add(Rectangle::new(40.0, 20.0))),
            Transform::from_xyz(pos.x, pos.y, 1.0),
            children![(
                Sprite::from_atlas_image(
                    texture.clone(),
                    bevy::prelude::TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: monster_animation_indices.first,
                    },
                ),
                Transform::from_xyz(0.0, 43.0, 0.0).with_scale(Vec3::splat(2.0)),
                monster_animation_indices,
                AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                MonsterSprite,
            )],
        ));
    }
}

/*pub fn spawn_monsters(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let texture = asset_server.load("textures/monster1.png");
    let layout = TextureAtlasLayout::from_grid(bevy::prelude::UVec2::splat(64), 2, 2, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    let positions = [
        Vec2::new(-500.0, -500.0),
        Vec2::new(500.0, -500.0),
        Vec2::new(-500.0, 500.0),
        Vec2::new(500.0, 500.0),
    ];

    for pos in positions {
        let monster_animation_indices = AnimationIndices { first: 0, last: 3 };

        commands.spawn((
            Monster,
            MonsterAI {
                target_player: false,
                random_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
                random_dir: Vec2::ZERO,
                action_timer: Timer::from_seconds(0.25, TimerMode::Once),
                action_cooldown: Timer::from_seconds(2.0, TimerMode::Once),
            },
            Pending,
            Mesh2d(meshes.add(Rectangle::new(40.0, 20.0))),
            Transform::from_xyz(pos.x, pos.y, 1.0),
            children![(
                Sprite::from_atlas_image(
                    texture.clone(),
                    bevy::prelude::TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: monster_animation_indices.first,
                    },
                ),
                Transform::from_xyz(0.0, 43.0, 0.0).with_scale(Vec3::splat(2.0)),
                monster_animation_indices,
                AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                MonsterSprite,
            )],
        ));
    }
}*/

fn monster_ai(
    time: Res<Time>,
    mut player_query: Query<(&Transform, &mut PlayerData), (With<Player>, Without<Pending>)>,
    mut query: Query<(&mut MonsterAI, &mut RigidBodyHandleComponent, &mut Transform, Entity), (With<Monster>, Without<Player>, Without<Pending>)>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
    mut island_manager: ResMut<ResIslandManager>,
    mut commands: Commands,
    config: Res<MonsterConfig>,
    bodies_query: Query<(&Transform), (Or<(With<Wall>, With<Floor>)>, Without<Pending>, With<RigidBodyHandleComponent>, Without<Player>, Without<Monster>)>,
) {
    let (player_transform, mut player_data_some): (Transform, Option<Mut<PlayerData>>) =
    if let Ok((t, mut d)) = player_query.get_single_mut() {
        (t.clone(), Some(d)) // <- klonujemy Transform, żeby mieć wartość
    } else {
        (Transform::default(), None)
    };

    let tile_size = 64.0;
    let see_distance = 5.0 * tile_size;
    let forget_distance = 10.0 * tile_size;
    let despawn_distance = config.max_despawn_distance * tile_size;
    let action_distance = 1.25 * tile_size;
    let speed = 80.0; // wolniejsze

    for (mut ai, rb_handle, mut rb_transform, entity) in &mut query {
        if let Some(rigid_body) = rigid_bodies.0.get_mut(rb_handle.0) {
            let monster_pos = Vec2::new(
                rigid_body.position().translation.x,
                rigid_body.position().translation.y,
            );
            let player_pos = player_transform.translation.xy();
            let distance = monster_pos.distance(player_pos);

            // despawn jeśli zbyt daleko
            if distance > despawn_distance {
                let mut colliders_clone = Vec::new();
                if let Some(rb) = rigid_bodies.0.get(rb_handle.0) {
                    for collider_handle in rb.colliders() {
                        colliders_clone.push(collider_handle.clone());
                    }
                }

                for collider_handle in colliders_clone {
                    colliders.0.remove(collider_handle, &mut island_manager.0, &mut rigid_bodies.0, true);
                }
                rigid_bodies.0.remove(
                    rb_handle.0,
                    &mut island_manager.0,
                    &mut colliders.0,
                    &mut ImpulseJointSet::new(),
                    &mut MultibodyJointSet::new(),
                    true, // usuwa powiązane collidery
                );
                commands.entity(entity).despawn_recursive();
                continue;
            }
            let mut next = true;
            for transform in bodies_query {
                let min_x = transform.translation.x - config.tile_size/2.0;
                let max_x = transform.translation.x + config.tile_size/2.0;
                let min_y = transform.translation.y - config.tile_size/2.0;
                let max_y = transform.translation.y + config.tile_size/2.0;

                if monster_pos.x >= min_x && monster_pos.x <= max_x && monster_pos.y >= min_y && monster_pos.y <= max_y {
                    next = false;
                    break;
                }
            }
            if !next {
                let mut colliders_clone = Vec::new();
                if let Some(rb) = rigid_bodies.0.get(rb_handle.0) {
                    for collider_handle in rb.colliders() {
                        colliders_clone.push(collider_handle.clone());
                    }
                }

                for collider_handle in colliders_clone {
                    colliders.0.remove(collider_handle, &mut island_manager.0, &mut rigid_bodies.0, true);
                }
                rigid_bodies.0.remove(
                    rb_handle.0,
                    &mut island_manager.0,
                    &mut colliders.0,
                    &mut ImpulseJointSet::new(),
                    &mut MultibodyJointSet::new(),
                    true, // usuwa powiązane collidery
                );
                commands.entity(entity).despawn_recursive();
                continue;
            }
            // AI logika
            if ai.target_player {
                if let Some(ref mut player_data) = player_data_some {
                    if distance > forget_distance {
                        ai.target_player = false;
                        ai.random_dir = Vec2::new(rand_dir(), rand_dir());
                        ai.random_timer.reset();
                        ai.action_timer.reset();
                    } else if distance < action_distance {
                        ai.action_timer.tick(time.delta());
                        if ai.action_timer.finished() && ai.action_cooldown.finished() {
                            player_data.damage(20.0);
                            player_data.can_heal.reset();
                            ai.action_cooldown.reset();
                            ai.action_timer.reset();
                        }
                    } else {
                        ai.action_timer.reset();
                    }
                } else {
                    ai.target_player = false;
                    ai.random_dir = Vec2::new(rand_dir(), rand_dir());
                    ai.random_timer.reset();
                    ai.action_timer.reset();
                }
            } else {
                if distance < see_distance && player_data_some.is_some() {
                    ai.target_player = true;
                }
            }

            ai.action_cooldown.tick(time.delta());

            // ruch
            let dir = if ai.target_player {
                (player_pos - monster_pos).normalize_or_zero()
            } else {
                ai.random_timer.tick(time.delta());
                if ai.random_timer.just_finished() {
                    ai.random_dir = Vec2::new(rand_dir(), rand_dir()).normalize_or_zero();
                }
                ai.random_dir
            };

            let velocity = dir * speed;
            rigid_body.set_linvel(vector![velocity.x, velocity.y], true);
            rigid_body.lock_rotations(true, true);
            rigid_body.set_body_type(RigidBodyType::Dynamic, true);
            if dir.x < 0.0 {
                if rb_transform.scale.x < 0.0 {
                    rb_transform.scale.x *= -1.0;
                }
            } else {
                if rb_transform.scale.x > 0.0 {
                    rb_transform.scale.x *= -1.0;
                }
            }
            rb_transform.translation.z = -(((WORLD_SIZE as f32*TILE_SIZE)/2.0)/64.0 + rigid_body.translation().y.round()/64.0) + 64.0;
        }
    }
}

fn rand_dir() -> f32 {
    // losowa wartość między -1 a 1
    (rand::random::<f32>() - 0.5) * 2.0
}

fn animate_monster_sprite(
    time: Res<Time>,
    mut query: Query<(&AnimationIndices, &mut AnimationTimer, &mut Sprite), With<MonsterSprite>>,
) {
    for (indices, mut timer, mut sprite) in &mut query {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = if atlas.index == indices.last {
                    indices.first
                } else {
                    atlas.index + 1
                };
            }
        }
    }
}
