use bevy::prelude::*;
use bevy::render::mesh::{Mesh, VertexAttributeValues, Indices, PrimitiveTopology};
use rapier2d::prelude::*;
use rapier2d::na::Point2;

use crate::physics_resources::*;


#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Floor;

#[derive(Component)]
pub struct Wall;

#[derive(Component)]
pub struct Pending;

pub struct SetupPlugin;

#[derive(Component)]
struct AnimationIndices {
    first: usize,
    last: usize,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
        app.add_systems(Update, update).add_systems(Update, animate_sprite);
        app.add_systems(Update, inspect_mesh_data);
    }
}

fn animate_sprite(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&AnimationIndices, &mut AnimationTimer, &mut Sprite, &mut Transform)>,
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

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let texture = asset_server.load("textures/player_sprite.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 2, 2, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);
    let animation_indices = AnimationIndices { first: 0, last: 3 };

    commands.spawn(Camera2d);
    commands.spawn((
        Player,
        Pending,
        Mesh2d(meshes.add(Rectangle::new(50.0, 100.0))),
        //MeshMaterial2d(materials.add(Color::hsl(0.25, 0.95, 0.7))),
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
            Transform::from_scale(Vec3::splat(2.5)),
            animation_indices,
            AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
        )]
    ));
    commands.spawn((
        Wall,
        Pending,
        Mesh2d(meshes.add(Rectangle::new(50.0, 450.0))),
        MeshMaterial2d(materials.add(Color::hsl(0.85, 0.15, 0.47))),
        Transform::from_xyz(
            225.0,
            0.0,
            0.0,
        ),
    ));
    commands.spawn((
        Floor,
        Mesh2d(meshes.add(Rectangle::new(500.0, 500.0))),
        MeshMaterial2d(materials.add(Color::hsl(0.865, 0.195, 0.27))),
        Transform::from_xyz(
            0.0,
            0.0,
            -1.0,
        ),
    ));
}

fn update(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&RigidBodyHandleComponent, (With<Player>, Without<Pending>)>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
) {
    let Ok(handle) = query.single_mut() else {
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
}



fn inspect_mesh_data(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    query: Query<(Entity, &Mesh2d, &Transform, Option<&Player>, Option<&Wall>), With<Pending>>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
) {
    for (entity, mesh_handle, transform, player, wall) in &query {
        if let Some(mesh) = meshes.get(&mesh_handle.0) {
            if let Some((vertices, indices)) = handle_mesh(mesh, transform) {
                let rigid_body = if player.is_some() {
                    RigidBodyBuilder::dynamic().soft_ccd_prediction(0.0).lock_rotations()
                } else if wall.is_some() {
                    RigidBodyBuilder::fixed()
                } else {
                    RigidBodyBuilder::dynamic().soft_ccd_prediction(0.0).lock_rotations()
                }.translation(vector![transform.translation.x, transform.translation.y])
                    .build();
                let rb_handle = rigid_bodies.0.insert(rigid_body);
                let collider = ColliderBuilder::trimesh_with_flags(vertices, indices, TriMeshFlags::MERGE_DUPLICATE_VERTICES).expect("REASON")
                    .restitution(0.0)
                    .friction(0.5)
                    .restitution_combine_rule(CoefficientCombineRule::Average)
                    .friction_combine_rule(CoefficientCombineRule::Average)
                    .build();
                let col_handle = colliders.0.insert_with_parent(collider, rb_handle, &mut rigid_bodies.0);
                commands.entity(entity).insert((
                    RigidBodyHandleComponent(rb_handle),
                ));
                commands.entity(entity).remove::<Pending>();
            }
        }
    }
}

fn handle_mesh(mesh: &Mesh, transform: &Transform) -> Option<(Vec<Point2<f32>>, Vec<[u32; 3]>)> {
    let scale = transform.scale;

    let positions: Vec<Point2<f32>> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions
            .iter()
            .map(|&[x, y, _z]| {
                Point2::new(
                    x * scale.x,
                    y * scale.y,
                )
            })
            .collect(),
        _ => return None,
    };

    let indices: Vec<u32> = match mesh.indices() {
        Some(Indices::U32(indices)) => indices.clone(),
        Some(Indices::U16(indices)) => indices.iter().map(|&i| i as u32).collect(),
        _ => return None,
    };

    let triangles: Vec<[u32; 3]> = indices
        .chunks(3)
        .filter_map(|chunk| {
            if chunk.len() == 3 {
                Some([chunk[0], chunk[1], chunk[2]])
            } else {
                None
            }
        })
        .collect();

    Some((positions, triangles))
}