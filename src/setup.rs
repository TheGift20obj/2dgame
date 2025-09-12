use bevy::prelude::*;
use bevy::render::mesh::{Mesh, VertexAttributeValues, Indices, PrimitiveTopology};
use rapier2d::prelude::*;
use rapier2d::na::Point2;
use noise::{NoiseFn, Fbm, Perlin};


use crate::physics_resources::*;

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup, terrain));
        app.add_systems(Update, (update, animate_player_sprite, animate_water_sprite));
        app.add_systems(Update, inspect_mesh_data);
    }
}

fn animate_player_sprite(
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

fn animate_water_sprite(
    time: Res<Time>,
    mut query: Query<(&AnimationIndices, &mut AnimationTimer, &mut Sprite, &mut Transform), With<WaterSprite>>,
) {
    for (indices, mut timer, mut sprite, mut transform) in &mut query {
        timer.tick(time.delta());
        if timer.just_finished() {
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

fn layer_checker(
    players: Query<&Transform, (With<Player>, Without<Pending>)>,
    monsters: Query<&Transform, (With<Monster>, Without<Pending>, Without<Player>)>,
    mut wall_query: Query<&mut Transform, (With<Wall>, Without<Pending>, Without<Player>, Without<Monster>)>,
) {
    for transform in players.iter().chain(monsters.iter()) {
        for mut wall_transform in &mut wall_query.iter_mut() {

            if (transform.translation.y+25.0/2.0) <= (wall_transform.translation.y-(64.0/(64.0/32.0))/2.0) {
                wall_transform.translation.z = -1.0;
            } else {
                wall_transform.translation.z = 1.0;
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
    images: Res<Assets<Image>>,
) {
    let texture = asset_server.load("textures/player_sprite.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 2, 2, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);
    let animation_indices = AnimationIndices { first: 0, last: 3 };

    //commands.spawn(Camera2d);
    commands.spawn((
        Camera2d,
        Player,
        Pending,
        Mesh2d(meshes.add(Rectangle::new(50.0, 25.0))),
        //MeshMaterial2d(materials.add(Color::hsl(0.25, 0.95, 0.7))),
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

pub fn terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let terrain_noise = Fbm::<Perlin>::new(921925);
    let path_noise = Fbm::<Perlin>::new(5342756);
    let biome_noise = Fbm::<Perlin>::new(2683467); // nowy noise dla biomów

    let world_size_x = 64;
    let world_size_y = 64;
    let tile_size = 64.0;

    let x_offset = (world_size_x as f32 * tile_size) / 2.0;
    let y_offset = (world_size_y as f32 * tile_size) / 2.0;

    // atlas wody
    let water_texture = asset_server.load("textures/water.png");
    let water_layout = TextureAtlasLayout::from_grid(UVec2::splat(32), 2, 2, None, None);
    let water_atlas = texture_atlas_layouts.add(water_layout);

    for gx in 0..world_size_x {
        for gy in 0..world_size_y {
            let x = gx as f32 * tile_size - x_offset;
            let y = gy as f32 * tile_size - y_offset;

            // === wybór biomu ===
            let biome_val = biome_noise.get([gx as f64 / 128.0, gy as f64 / 128.0]);
            let biome = "normal";

            // === noise terenu w obrębie biomu ===
            let terrain_val = terrain_noise.get([gx as f64 / 15.0, gy as f64 / 15.0]);

            let mut texture_path = match biome {
                "snow" => {
                    if terrain_val < -0.45 {
                        "textures/water"
                    } else if terrain_val < -0.25 {
                        "textures/ice.png"
                    } else {
                        "textures/snow.png"
                    }
                }
                "evil" => {
                    if terrain_val < -0.45 {
                        "textures/water"
                    } else if terrain_val < -0.25 {
                        "textures/evil_dirt.png"
                    } else if terrain_val < 0.3 {
                        "textures/evil_grass.png"
                    } else {
                        "textures/evil_stone.png"
                    }
                }
                _ => {
                    // normal
                    if terrain_val < -0.45 {
                        "textures/water"
                    } else if terrain_val < -0.25 {
                        "textures/sand.png"
                    } else if terrain_val < 0.0 {
                        "textures/dirt.png"
                    } else if terrain_val < 0.3 {
                        "textures/grass.png"
                    } else {
                        "textures/stone.png"
                    }
                }
            };

            // path tylko w normal i evil
            if biome != "snow"
                && (texture_path.ends_with("dirt.png") || texture_path.ends_with("grass.png"))
            {
                let path_val = path_noise.get([gx as f64 / 8.0, gy as f64 / 8.0]);
                if path_val.abs() < 0.05 {
                    texture_path = "textures/path.png";
                }
            }

            // === Spawn Floor ===
            if texture_path == "textures/water" {
                // woda animowana
                commands.spawn((
                    Floor,
                    Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
                    Transform::from_xyz(x, y, -3.0),
                    children![(
                        Sprite::from_atlas_image(
                            water_texture.clone(),
                            TextureAtlas {
                                layout: water_atlas.clone(),
                                index: 0,
                            },
                        ),
                        Transform::from_scale(Vec3::splat(tile_size / 32.0)),
                        AnimationIndices { first: 0, last: 3 },
                        AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                        WaterSprite,
                    )],
                ));
            } else {
                commands.spawn((
                    Floor,
                    Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
                    Transform::from_xyz(x, y, -3.0),
                    children![(
                        Sprite::from_image(asset_server.load(texture_path)),
                        Transform::from_scale(Vec3::splat(tile_size / 32.0)),
                    )],
                ));
            }

            // === Ściany tylko na stone/evil_stone ===
            if texture_path == "textures/stone.png" || texture_path == "textures/evil_stone.png" {
                let wall_val = terrain_noise.get([gx as f64 / 6.0, gy as f64 / 6.0, 999.0]);
                if wall_val > 0.0 {
                    spawn_wall(&mut commands, &mut meshes, &asset_server, x, y, tile_size, y_offset);
                }
            }
        }
    }

    // === barierka wokół świata ===
    for gx in 0..world_size_x {
        for gy in 0..world_size_y {
            if gx == 0 || gy == 0 || gx == world_size_x - 1 || gy == world_size_y - 1 {
                let x = gx as f32 * tile_size - x_offset;
                let y = gy as f32 * tile_size - y_offset;
                spawn_wall(&mut commands, &mut meshes, &asset_server, x, y, tile_size, y_offset);
            }
        }
    }
}


fn spawn_wall(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    asset_server: &Res<AssetServer>,
    x: f32,
    y: f32,
    tile_size: f32,
    y_offset: f32,
) {
    commands.spawn((
        Wall,
        Pending,
        Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
        Transform::from_xyz(x, y, -(y_offset/64.0 + y/64.0)+64.0),
        children![(
            Sprite::from_image(asset_server.load("textures/main_wall.png")),
            Transform::from_xyz(0.0, 0.0, 0.0)
                .with_scale(Vec3::splat(tile_size / 32.0)),
        ),(
            Sprite::from_image(asset_server.load("textures/side_wall.png")),
            Transform::from_xyz(-tile_size, 0.0, 0.05)
                .with_scale(Vec3::splat(tile_size / 32.0)),
        ),(
            Sprite::from_image(asset_server.load("textures/up_wall.png")),
            Transform::from_xyz(0.0, tile_size, 0.05)
                .with_scale(Vec3::splat(tile_size / 32.0)),
        ),(
            Sprite::from_image(asset_server.load("textures/corner_wall.png")),
            Transform::from_xyz(-tile_size, tile_size, 0.1)
                .with_scale(Vec3::splat(tile_size / 32.0)),
        )],
    ));
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