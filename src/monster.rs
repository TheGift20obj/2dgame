use bevy::prelude::*;
use rapier2d::prelude::*;

use crate::physics_resources::*;

#[derive(Component)]
pub struct MonsterAI {
    pub target_player: bool,
    pub random_timer: Timer,
    pub random_dir: Vec2,
    pub action_timer: Timer,
    pub action_couldown: Timer,
}

pub struct MonsterPlugin;

impl Plugin for MonsterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_monsters)
           .add_systems(Update, monster_ai)
           .add_systems(Update, animate_monster_sprite);
    }
}

pub fn spawn_monsters(
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
                action_couldown: Timer::from_seconds(2.0, TimerMode::Once),
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

fn monster_ai(
    time: Res<Time>,
    mut player_query: Query<(&Transform, &mut PlayerData), (With<Player>, Without<Pending>)>,
    mut query: Query<(&mut MonsterAI, &mut RigidBodyHandleComponent, &mut Transform), (With<Monster>, Without<Player>, Without<Pending>)>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
) {
    let (player_transform, mut player_data) = if let Ok((t, mut d)) = player_query.get_single_mut() {
        (t, d)
    } else {
        return;
    };

    let tile_size = 64.0;
    let see_distance = 5.0 * tile_size;
    let forget_distance = 10.0 * tile_size;
    let action_distance = 0.75 * tile_size;
    let speed = 80.0; // wolniejsze

    for (mut ai, rb_handle, mut rb_transform) in &mut query {
        if let Some(rigid_body) = rigid_bodies.0.get_mut(rb_handle.0) {
            let monster_pos = Vec2::new(
                rigid_body.position().translation.x,
                rigid_body.position().translation.y,
            );
            let player_pos = player_transform.translation.xy();
            let distance = monster_pos.distance(player_pos);
            // AI logika
            if ai.target_player {
                if distance > forget_distance {
                    ai.target_player = false;
                    ai.random_dir = Vec2::new(rand_dir(), rand_dir());
                    ai.random_timer.reset();
                    ai.action_timer.reset();
                } else if distance < action_distance {
                    ai.action_timer.tick(time.delta());
                    if ai.action_timer.finished() && ai.action_couldown.finished() {
                        player_data.damage(12.5);
                        player_data.can_heal.reset();
                        println!("You got damaged (-12.5 hp) you have now {}", player_data.health);
                        ai.action_couldown.reset();
                        ai.action_timer.reset();
                    }
                } else {
                    ai.action_timer.reset();
                }
            } else {
                if distance < see_distance {
                    ai.target_player = true;
                }
            }

            ai.action_couldown.tick(time.delta());

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
            rb_transform.translation.z = -(2048.0/64.0 + rigid_body.translation().y.round()/64.0) + 64.0;
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
