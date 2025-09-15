use bevy::prelude::*;
use crate::physics_resources::*;

use rapier2d::prelude::*;
use rapier2d::na::Point2;

pub struct Item {
    pub value: String,
}

pub struct Inventory {
    pub items: Vec<Item>,
    pub capacity: u32,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            capacity: 16,
            items: Vec::new(),
        }
    }
}

#[derive(Component)]
pub struct PlayerData {
    pub health: f32,
    pub inventory: Inventory,
}

impl PlayerData {
    pub fn new() -> Self {
        Self {
            health: 100.0,
            inventory: Inventory::new(),
        }
    }

    pub fn heal(&mut self, value: f32) {
        self.health = (self.health + value).clamp(0.0, 100.0);
    }

    pub fn damage(&mut self, value: f32) {
        self.health = (self.health - value).clamp(0.0, 100.0);
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        app.add_systems(Update, (update, animate_sprite));
    }
}

fn init(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    images: Res<Assets<Image>>,
) {
    let texture = asset_server.load("textures/player_sprite.png");
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
    ));
}

fn animate_sprite(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&AnimationIndices, &mut AnimationTimer, &mut Sprite, &mut Transform), With<PlayerSprite>>,
) {
    for (indices, mut timer, mut sprite, mut transform) in &mut query {
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

        if moving {
            transform.scale.x = direction * transform.scale.x.abs();
            if timer.just_finished() {
                if let Some(atlas) = &mut sprite.texture_atlas {
                    atlas.index = if atlas.index == indices.last {
                        indices.first
                    } else {
                        atlas.index + 1
                    };
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
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&RigidBodyHandleComponent, &mut Transform), (With<Player>, Without<Pending>)>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
) {
    let Ok((handle, mut transform)) = query.single_mut() else {
        return;
    };

    let mut rigidbody = rigid_bodies.0.get_mut(handle.0).unwrap();

    let mut dir = Vec2::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) { dir.y += 1.0; }
    if keyboard_input.pressed(KeyCode::KeyS) { dir.y -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyA) { dir.x -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyD) { dir.x += 1.0; }

    let mut speed = 200.0;

    if keyboard_input.pressed(KeyCode::ShiftLeft) { speed = 350.0; }

    let velocity = if dir.length_squared() > 0.0 {
        dir.normalize() * speed
    } else {
        Vec2::ZERO
    };

    rigidbody.set_linvel(vector![velocity.x, velocity.y], true);
    transform.translation.z = -(2048.0/64.0 + rigidbody.translation().y.round()/64.0) + 64.0;
}