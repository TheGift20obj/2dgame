use bevy::prelude::*;
use crate::physics_resources::*;

use noise::{NoiseFn, Fbm, Perlin};
use std::collections::HashSet;

use rapier2d::prelude::ImpulseJointSet;
use rapier2d::prelude::MultibodyJointSet;

#[derive(Resource, Default)]
pub struct TerrainMap {
    pub generated: HashSet<IVec2>, // np. współrzędne kafelków
}

pub const WORLD_SIZE: i32 = 16;   // liczba kafelków widocznych w danym "obszarze"
pub const TILE_SIZE: f32 = 64.0;

pub struct TerrainGenerationPlugin;

impl Plugin for TerrainGenerationPlugin {
    fn build(&self, app: &mut App) {
         app.insert_resource(TerrainMap::default())
            .add_systems(Startup, init_terrain)
            .add_systems(Update, (update_terrain, animate_sprite));
    }
}

// === GENERACJA STARTOWA ===
fn init_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut terrain_map: ResMut<TerrainMap>,
) {
    let center = IVec2::ZERO;
    generate_area(
        &mut commands,
        &mut meshes,
        &asset_server,
        &mut texture_atlas_layouts,
        &mut terrain_map,
        center,
    );
}

fn update_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut terrain_map: ResMut<TerrainMap>,
    player_q: Query<&Transform, With<Player>>,

    // do usuwania fizyki
    mut colliders: ResMut<ResColliderSet>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut island_manager: ResMut<ResIslandManager>,
    query_phys: Query<(Entity, &Transform, &RigidBodyHandleComponent), Or<(With<Floor>, With<Wall>)>>,
    query_non_phys: Query<(Entity, &Transform), (Or<(With<Floor>, With<Wall>)>, Without<RigidBodyHandleComponent>)>,
) {
    let player_transform = if let Ok(d) = player_q.get_single() {
        d
    } else {
        return;
    };
    let player_pos = player_transform.translation.truncate();
    let center = IVec2::new(
        (player_pos.x / TILE_SIZE).round() as i32 * TILE_SIZE as i32,
        (player_pos.y / TILE_SIZE).round() as i32 * TILE_SIZE as i32,
    );

    // === Dodaj nowy teren ===
    generate_area(
        &mut commands,
        &mut meshes,
        &asset_server,
        &mut texture_atlas_layouts,
        &mut terrain_map,
        center,
    );

    // === Usuń stary teren ===
    let radius = (TILE_SIZE as i32 * WORLD_SIZE) / 2;
    let mut to_remove = Vec::new();
    for &pos in terrain_map.generated.iter() {
        if (pos.x - center.x).abs() > radius || (pos.y - center.y).abs() > radius {
            to_remove.push(pos);
        }
    }

    for pos in to_remove {
        // usuń encje w tym kafelku
        for (entity, transform, handle) in query_phys.iter() {
            if !(transform.translation.x as i32 == pos.x && transform.translation.y as i32 == pos.y) {
                continue;
            }
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
            commands.entity(entity).despawn_recursive();
        }

        for (entity, transform) in query_non_phys.iter() {
            if !(transform.translation.x as i32 == pos.x && transform.translation.y as i32 == pos.y) {
                continue;
            }
            commands.entity(entity).despawn_recursive();
        }

        terrain_map.generated.remove(&pos);
    }
}

fn generate_area(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    asset_server: &Res<AssetServer>,
    texture_atlas_layouts: &mut ResMut<Assets<TextureAtlasLayout>>,
    terrain_map: &mut ResMut<TerrainMap>,
    center: IVec2,
) {
    let terrain_noise = Fbm::<Perlin>::new(921925);
    let path_noise = Fbm::<Perlin>::new(5342756);
    let biome_noise = Fbm::<Perlin>::new(2683467); // nowy noise dla biomów

    let world_size_x = WORLD_SIZE;
    let world_size_y = WORLD_SIZE;
    let tile_size = TILE_SIZE;
    let radius = world_radius as f32;
    let x_offset = (world_size_x as f32 * tile_size) / 2.0;
    let y_offset = (world_size_y as f32 * tile_size) / 2.0;

    // atlas wody
    let water_texture = asset_server.load("textures/water.png");
    let water_layout = TextureAtlasLayout::from_grid(UVec2::splat(32), 2, 2, None, None);
    let water_atlas = texture_atlas_layouts.add(water_layout);

    for gx in 0..world_size_x {
        for gy in 0..world_size_y {
            let dist2 = (gx * gx + gy * gy) as f32;
            if !(dist2 <= radius * radius) {
                continue;
            }
            let x = gx as f32 * tile_size - x_offset + center.x as f32;
            let y = gy as f32 * tile_size - y_offset + center.y as f32;
            
            let pos = IVec2::new(x as i32, y as i32);
            if terrain_map.generated.contains(&pos) {
                continue;
            }

            terrain_map.generated.insert(pos);
            // === wybór biomu ===
            let biome_val = biome_noise.get([(x / tile_size) as f64 / 128.0, (y / tile_size) as f64 / 128.0]);
            let biome = "normal";

            // === noise terenu w obrębie biomu ===
            let terrain_val = terrain_noise.get([(x / tile_size) as f64 / 15.0, (y / tile_size) as f64 / 15.0]);

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
                    Pending,
                    Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
                    Transform::from_xyz(x, y, -3.0 + -(y_offset/64.0 + y/64.0)+64.0),
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
                    Transform::from_xyz(x, y, -3.0 + -(y_offset/64.0 + y/64.0)+64.0),
                    children![(
                        Sprite::from_image(asset_server.load(texture_path)),
                        Transform::from_scale(Vec3::splat(tile_size / 32.0)),
                    )],
                ));
            }

            // === Ściany tylko na stone/evil_stone ===
            if texture_path == "textures/stone.png" || texture_path == "textures/evil_stone.png" {
                let wall_val = terrain_noise.get([(x / tile_size) as f64 / 6.0, (y / tile_size) as f64 / 6.0, 999.0]);
                if wall_val > 0.0 {
                    spawn_wall(commands, meshes, asset_server, x, y, tile_size, y_offset);
                }
            }
        }
    }

    // === barierka wokół świata ===
    /*for gx in 0..world_size_x {
        for gy in 0..world_size_y {
            if gx == 0 || gy == 0 || gx == world_size_x - 1 || gy == world_size_y - 1 {
                let x = gx as f32 * tile_size - x_offset;
                let y = gy as f32 * tile_size - y_offset;
                spawn_wall(commands, meshes, asset_server, x, y, tile_size, y_offset);
            }
        }
    }*/
}

fn animate_sprite(
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