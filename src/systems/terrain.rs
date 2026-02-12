use bevy::prelude::*;
use crate::resourses::physics_resources::*;

use noise::{NoiseFn, Fbm, Perlin};
use std::collections::HashSet;

use rapier2d::prelude::ImpulseJointSet;
use rapier2d::prelude::MultibodyJointSet;

use bevy_2d_screen_space_lightmaps::lightmap_plugin::lightmap_plugin::*;
use bevy::camera::visibility::RenderLayers;

use std::collections::HashMap;
use bevy_firefly::prelude::*;

#[derive(Component, Clone)]
pub struct GapOccluder;

const TR_LOCAL: Vec2 = Vec2::new(-TILE_SIZE / 9.0, TILE_SIZE / 1.75);
const HALF_TILE: Vec2 = Vec2::new(TILE_SIZE / 1.125, TILE_SIZE / 1.125);

#[derive(Component, Clone)]
pub struct OccluderMeta {
    /// lokalny transform taki jaki miał occluder na starcie (base)
    pub base_local: Transform,
    /// base half extents (x = half_width, y = half_height)
    pub base_half: Vec2,
}

#[derive(Resource, Default)]
struct TerrainMap {
    generated: HashSet<IVec2>,
    linked: HashSet<IVec2, Vec<Entity>>,
    wall_map: HashMap<IVec2, Entity>,
    pub gap_occluders: HashMap<(Entity, Entity), Entity>,
}

impl TerrainMap {
    /// Tworzy canonical key (A,B) niezależnie od kolejności
    fn canonical_pair(a: Entity, b: Entity) -> (Entity, Entity) {
        if a.index() < b.index() { (a,b) } else { (b,a) }
    }

    /// Dodaje gap occluder między dwoma ścianami
    pub fn add_gap_occluder(
        &mut self,
        a: Entity,
        b: Entity,
        gap_entity: Entity,
    ) {
        let key = Self::canonical_pair(a, b);
        self.gap_occluders.insert(key, gap_entity);
    }

    /// Pobiera wszystkie gapy powiązane z wybraną ścianą
    pub fn get_gaps_for_wall(&self, wall: Entity) -> Vec<Entity> {
        self.gap_occluders
            .iter()
            .filter_map(|(&(a,b), &gap)| if a==wall || b==wall { Some(gap) } else { None })
            .collect()
    }

    /// Usuwa wszystkie gapy powiązane z wybraną ścianą
    pub fn remove_gaps_for_wall(
        &mut self,
        wall: Entity,
        commands: &mut Commands
    ) {
        let keys_to_remove: Vec<(Entity,Entity)> = self.gap_occluders
            .iter()
            .filter(|&(&k,_)| k.0 == wall || k.1 == wall)
            .map(|(&k,_)| k)
            .collect();

        for key in keys_to_remove {
            if let Some(gap_ent) = self.gap_occluders.remove(&key) {
                commands.entity(gap_ent).despawn();
            }
        }
    }

    /// Znajduje sąsiednie ściany w czterech kierunkach (w gridzie)
    pub fn find_adjacent_walls(
        &self,
        wall_pos: IVec2,
    ) -> Vec<IVec2> {
        let mut neighbors = Vec::new();
        let dirs = [IVec2::new((TILE_SIZE as i32),0), IVec2::new((-TILE_SIZE as i32),0), IVec2::new(0,(TILE_SIZE as i32)), IVec2::new(0,(-TILE_SIZE as i32))];
        for dir in dirs {
            let npos = wall_pos + dir;
            if self.generated.contains(&npos) {
                neighbors.push(npos);
            }
        }
        neighbors
    }
}

pub fn add_gap_occluders_for_tile(
    commands: &mut Commands,
    terrain_map: &mut TerrainMap,
    wall_pos: IVec2,
    tile_size: f32,
) {
    // znajdź sąsiednie ściany
    let neighbors = terrain_map.find_adjacent_walls(wall_pos);

    if neighbors.is_empty() {
        //println!("Wall at {:?} has no neighbors, skipping gap occluder", wall_pos);
        return; // brak sąsiadów → brak gapów
    }
    //println!("Wall at {:?} has neighbors at: {:?}", wall_pos, neighbors);

    let wall_entity = match terrain_map.wall_map.get(&wall_pos) {
        Some(&e) => e,
        None => return,
    };

    for &neighbor_pos in neighbors.iter() {
        let neighbor_entity = match terrain_map.wall_map.get(&neighbor_pos) {
            Some(&e) => e,
            None => continue,
        };

        // canonical key
        let key = TerrainMap::canonical_pair(wall_entity, neighbor_entity);

        // jeśli gap już istnieje, pomijamy
        if terrain_map.gap_occluders.contains_key(&key) {
            continue;
        }

        // --- WYLICZENIE GAP TRANSFORM ---
        // gap będzie w połowie dystansu między wall_entity i neighbor_entity
        let center_x = (wall_pos.x as f32 + neighbor_pos.x as f32) * 0.5;
        let center_y = (wall_pos.y as f32 + neighbor_pos.y as f32) * 0.5;

        // długość gapu = dystans między krawędziami, przyjmujemy tile_size dla prostoty
        let half_x = if wall_pos.x != neighbor_pos.x { tile_size * 0.5 } else { HALF_TILE.x };
        let half_y = if wall_pos.y != neighbor_pos.y { tile_size * 0.5 } else { HALF_TILE.y };

        // wybieramy parenta dla gapu (np wall_entity)
        let parent_entity = wall_entity;

        // transform lokalny względem parenta
        let parent_pos = Vec3::new(wall_pos.x as f32, wall_pos.y as f32, 0.0);
        let local_x = center_x - parent_pos.x + TR_LOCAL.x;
        let local_y = center_y - parent_pos.y + TR_LOCAL.y;

        let gap_transform = Transform {
            translation: Vec3::new(local_x, local_y, 0.0),
            rotation: Default::default(),
            scale: Vec3::ONE,
        };

        // spawn gap occludera
        let gap_entity = commands.spawn((
            GapOccluder,
            gap_transform,
            Occluder2d::rectangle(half_x, half_y),
            YSort { z: 0.8 },
        )).id();
        //println!("Spawning gap between {:?} and {:?} at local ({}, {})", wall_pos, neighbor_pos, local_x, local_y);

        commands.entity(parent_entity).add_children(&[gap_entity]);

        // zapisz do terrain_map
        terrain_map.add_gap_occluder(wall_entity, neighbor_entity, gap_entity);
    }
}

pub fn remove_gap_occluders_for_wall(
    terrain_map: &mut TerrainMap,
    wall_entity: Entity,
    commands: &mut Commands,
) {
    terrain_map.remove_gaps_for_wall(wall_entity, commands);
}

pub struct TerrainGenerationPlugin;

impl Plugin for TerrainGenerationPlugin {
    fn build(&self, app: &mut App) {
         app.insert_resource(TerrainMap::default())
            .add_systems(Startup, init_terrain)
            .add_systems(Update, (update_terrain, animate_sprite, y_sort_relative));
    }
}

fn y_sort_relative(
    cam_q: Query<&GlobalTransform, (With<Camera>, With<PlayerCamera>)>,
    mut q: Query<(&GlobalTransform, &mut Transform, &YSort)>,
) {
    let cam_y = match cam_q.single() {
        Ok(cam) => cam.translation().y,
        Err(_) => return,
    };

    for (global, mut tf, ysort) in q.iter_mut() {
        let relative_y = global.translation().y - cam_y;

        // sigmoid w MAŁYM zakresie
        let depth = 1.0 / (1.0 + (-0.05 * relative_y).exp());

        // mniejsze Y = wyżej rysowane
        tf.translation.z = ysort.z - depth;
    }
}

fn y_sort(
    mut q: Query<(&mut Transform, &YSort)>,
) {
    for (mut tf, ysort) in q.iter_mut() {
        tf.translation.z = ysort.z-(1.0f32 / (1.0f32 + (2.0f32.powf(-0.01*tf.translation.y))));
    }
}

// === GENERACJA STARTOWA ===
fn init_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut terrain_map: ResMut<TerrainMap>,
    query_non_phys: Query<(Entity, &Transform, Option<&Fog>, &Children), (Or<(With<Floor>, With<Wall>)>, Without<RigidBodyHandleComponent>)>,
    mut sprite_query: Query<&mut Sprite, With<WaterSprite>>,
) {
    let center = IVec2::ZERO;
    generate_area(
        &mut commands,
        &mut meshes,
        &asset_server,
        &mut texture_atlas_layouts,
        &mut terrain_map,
        center,
        &query_non_phys,
        &mut sprite_query,
    );
    generate_halo(
        &mut commands,
        &mut meshes,
        &asset_server,
        &mut texture_atlas_layouts,
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
    mut sprite_query: Query<&mut Sprite, With<WaterSprite>>,
    // do usuwania fizyki
    mut colliders: ResMut<ResColliderSet>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut island_manager: ResMut<ResIslandManager>,
    query_phys: Query<(Entity, &Transform, &RigidBodyHandleComponent), Or<(With<Floor>, With<Wall>)>>,
    query_non_phys: Query<(Entity, &Transform, Option<&Fog>, &Children), (Or<(With<Floor>, With<Wall>)>, Without<RigidBodyHandleComponent>)>,
    mut halo_query: Query<&mut Transform, (With<FogHalo>, Without<Floor>, Without<Wall>, Without<Player>)>,
) {
    let player_transform = if let Ok(d) = player_q.single() {
        d
    } else {
        return;
    };
    let mut halo_transform = if let Ok(mut d) = halo_query.single_mut() {
        d
    } else {
        return;
    };
    let player_pos = player_transform.translation.truncate();
    let center = IVec2::new(
        (player_pos.x / TILE_SIZE).round() as i32 * TILE_SIZE as i32,
        (player_pos.y / TILE_SIZE).round() as i32 * TILE_SIZE as i32,
    );
    halo_transform.translation.x = center.x as f32;
    halo_transform.translation.y = center.y as f32;
    halo_transform.translation.z = player_transform.translation.z+3.14;
    // === Dodaj nowy teren ===
    generate_area(
        &mut commands,
        &mut meshes,
        &asset_server,
        &mut texture_atlas_layouts,
        &mut terrain_map,
        center,
        &query_non_phys,
        &mut sprite_query,
    );

    // === Usuń stary teren ===
    let radius = (TILE_SIZE as i32 * WORLD_SIZE) / 2;
    let mut to_remove = Vec::new();
    for &pos in terrain_map.generated.iter() {
        let dx = pos.x - center.x;
        let dy = pos.y - center.y;
        let r = (dx * dx + dy * dy);
        if r > (radius*radius)/25 {
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
            terrain_map.remove_gaps_for_wall(entity, &mut commands);
            terrain_map.wall_map.retain(|_, &mut e| e != entity);
            commands.entity(entity).despawn();
        }

        for (entity, transform, _, _) in query_non_phys.iter() {
            if !(transform.translation.x as i32 == pos.x && transform.translation.y as i32 == pos.y) {
                continue;
            }
            commands.entity(entity).despawn();
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
    query_non_phys: &Query<(Entity, &Transform, Option<&Fog>, &Children), (Or<(With<Floor>, With<Wall>)>, Without<RigidBodyHandleComponent>)>,
    sprite_query: &mut Query<&mut Sprite, With<WaterSprite>>,
) {
    let terrain_noise = Fbm::<Perlin>::new(921925);
    let path_noise = Fbm::<Perlin>::new(5342756);
    let biome_noise = Fbm::<Perlin>::new(2683467); // nowy noise dla biomów

    let world_size_x = WORLD_SIZE/3;
    let world_size_y = WORLD_SIZE/3;
    let tile_size = TILE_SIZE;
    let radius = WORLD_SIZE as f32;
    let x_offset = (world_size_x as f32 * tile_size) / 2.0;
    let y_offset = (world_size_y as f32 * tile_size) / 2.0;
    let g_offset = (radius * tile_size) / 2.0;

    // atlas wody
    let water_texture = asset_server.load("textures/water.png");
    let water_layout = TextureAtlasLayout::from_grid(UVec2::splat(32), 2, 2, None, None);
    let water_atlas = texture_atlas_layouts.add(water_layout);

    let fog_texture = asset_server.load("textures/fog_black.png");
    let fog_layout = TextureAtlasLayout::from_grid(UVec2::splat(32), 2, 2, None, None);
    let fog_atlas = texture_atlas_layouts.add(fog_layout);

    for gx in 0..world_size_x {
        for gy in 0..world_size_y {
            let dist2 = ((gx as f32 - x_offset/tile_size) * (gx as f32 - x_offset/tile_size) + (gy as f32 - y_offset/tile_size) * (gy as f32 - y_offset/tile_size)) as f32;
            if !(dist2*dist2 <= radius * radius) {
                continue;
            }
            let x = gx as f32 * tile_size - x_offset + center.x as f32;
            let y = gy as f32 * tile_size - y_offset + center.y as f32;
            
            let pos = IVec2::new(x as i32, y as i32);
            let t = ((dist2*dist2) / (radius-16.0).powi(2)).clamp(0.0, 1.0);
            let k = 0.25; // <1 → szybszy wzrost przezroczystości
            let transp = t.powf(k);
            if terrain_map.generated.contains(&pos) {
                if dist2*dist2 < 0.0 {
                    for (entity, transform, fog, _) in query_non_phys.iter() {
                        if fog.is_some() {
                            if !(transform.translation.x as i32 == pos.x && transform.translation.y as i32 == pos.y) {
                                continue;
                            }
                            commands.entity(entity).despawn();
                        }
                    }
                } else {
                    let mut exits = false;
                    for (entity, transform, fog, children) in query_non_phys.iter() {
                        if fog.is_some() {
                            if (transform.translation.x as i32 == pos.x && transform.translation.y as i32 == pos.y) {
                                exits = true;
                                for child in children.iter() {
                                    if let Ok(mut sprite) = sprite_query.get_mut(child) {
                                        sprite.color = Color::srgba(0.25, 0.25, 0.25, transp);
                                    }
                                }
                            }
                        }
                    }
                    if !exits {
                        commands.spawn((
                            Floor, Fog,
                            Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
                            //Transform::from_xyz(x, y, 3.14 + -(g_offset/64.0 + y/64.0)+64.0),
                            Transform::from_xyz(x, y, 100.0),
                            children![(
                                {
                                    let mut s = Sprite::from_atlas_image(
                                        fog_texture.clone(),
                                        TextureAtlas { layout: fog_atlas.clone(), index: 0 },
                                    );
                                    s.color = Color::srgba(0.25, 0.25, 0.25, transp); // odcień szarości + alfa
                                    s
                                },
                                Transform {
                                    scale: Vec3::new(tile_size / 32.0, tile_size / 32.0, 0.0),
                                    ..Default::default()
                                },
                                Visibility::Inherited,
                                RenderLayers::from_layers(CAMERA_LAYER_EFFECT),
                                AnimationIndices { first: 0, last: 3 },
                                AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                                WaterSprite,
                            )],
                        ));
                    }
                }
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
                let entity =commands.spawn((
                    Floor,
                    Pending,
                    Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
                    //Transform::from_xyz(x, y, -3.0 + -(g_offset/64.0 + y/64.0)+64.0),
                    Transform::from_xyz(x, y, -64.0),
                    children![(
                        Sprite::from_atlas_image(
                            water_texture.clone(),
                            TextureAtlas {
                                layout: water_atlas.clone(),
                                index: 0,
                            },
                        ),
                        Transform {
                            scale: Vec3::new(tile_size / 32.0, tile_size / 32.0, 1.0),
                            ..Default::default()
                        },
                        YSort { z: 0.0 },
                        AnimationIndices { first: 0, last: 3 },
                        AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                        WaterSprite,
                        RenderLayers::from_layers(CAMERA_LAYER_FLOOR)
                    )],
                ));
            } else {
                commands.spawn((
                    Floor,
                    Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
                    //Transform::from_xyz(x, y, -3.0 + -(g_offset/64.0 + y/64.0)+64.0),
                    Transform::from_xyz(x, y, -64.0),
                    children![(
                        Sprite::from_image(asset_server.load(texture_path)),
                        YSort { z: 0.0 },
                        Transform {
                            scale: Vec3::new(tile_size / 32.0, tile_size / 32.0, 1.0),
                            ..Default::default()
                        },
                        RenderLayers::from_layers(CAMERA_LAYER_FLOOR)
                    )],
                ));
            }

            // === Ściany tylko na stone/evil_stone ===
            if texture_path == "textures/stone.png" || texture_path == "textures/evil_stone.png" {
                let wall_val = terrain_noise.get([(x / tile_size) as f64 / 6.0, (y / tile_size) as f64 / 6.0, 999.0]);
                if wall_val > 0.0 {
                    let wall_entity = spawn_wall(commands, meshes, asset_server, x, y, tile_size, g_offset);
                    terrain_map.wall_map.insert(pos, wall_entity);
                    add_gap_occluders_for_tile(commands, terrain_map, pos, tile_size);
                }
            }

            if dist2*dist2 >= 0.0 {
                let mut exits = false;
                for (entity, transform, fog, children) in query_non_phys.iter() {
                    if fog.is_some() {
                        if (transform.translation.x as i32 == pos.x && transform.translation.y as i32 == pos.y) {
                            exits = true;
                            for child in children.iter() {
                                if let Ok(mut sprite) = sprite_query.get_mut(child) {
                                    sprite.color = Color::srgba(0.25, 0.25, 0.25, transp);
                                }
                            }
                        }
                    }
                }
                if !exits {
                    commands.spawn((
                        Floor, Fog,
                        Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
                        //Transform::from_xyz(x, y, 3.14 + -(g_offset/64.0 + y/64.0)+64.0),
                        Transform::from_xyz(x, y, 0.0),
                        children![(
                            {
                                let mut s = Sprite::from_atlas_image(
                                    fog_texture.clone(),
                                    TextureAtlas { layout: fog_atlas.clone(), index: 0 },
                                );
                                s.color = Color::srgba(0.25, 0.25, 0.25, transp); // odcień szarości + alfa
                                s
                            },
                            Transform {
                                scale: Vec3::new(tile_size / 32.0, tile_size / 32.0, 1.0),
                                ..Default::default()
                            },
                            AnimationIndices { first: 0, last: 3 },
                            AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                            WaterSprite,
                            RenderLayers::from_layers(CAMERA_LAYER_EFFECT),
                        )],
                    ));
                }
            } else {
                for (entity, transform, fog, _) in query_non_phys.iter() {
                    if fog.is_some() {
                        if !(transform.translation.x as i32 == pos.x && transform.translation.y as i32 == pos.y) {
                            continue;
                        }
                        commands.entity(entity).despawn();
                    }
                }
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
) -> Entity {
    //let half = Vec2::splat(tile_size);
    let child_local = Transform::from_xyz(TR_LOCAL.x, TR_LOCAL.y, 0.0);
    return commands.spawn((
        Wall,
        Pending,
        Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
        Transform::from_xyz(x, y, -32.0),
        children![(
            child_local,
            Occluder2d::rectangle(HALF_TILE.x, HALF_TILE.y),
            //OccluderMeta { base_local: child_local, base_half: half },
            YSort { z: 0.8 },
        ),(
            Occluder2d::rectangle(tile_size, tile_size),
            //OccluderMeta { base_local: child_local, base_half: half },
            YSort { z: -8.0 },
        ),
        (
            RenderLayers::from_layers(CAMERA_LAYER_WALL),
            YSort { z: 0.3 },
            Sprite::from_image(asset_server.load("textures/main_wall.png")),
            Transform::from_xyz(0.0, 0.0, 0.0)
                .with_scale(Vec3::new(tile_size / 32.0, tile_size / 32.0, 1.0)),
        ),(
            RenderLayers::from_layers(CAMERA_LAYER_WALL),
            YSort { z: 0.31 },
            Sprite::from_image(asset_server.load("textures/side_wall.png")),
            Transform::from_xyz(-tile_size, 0.0, 0.0)
                .with_scale(Vec3::new(tile_size / 32.0, tile_size / 32.0, 1.0)),
        ),(
            RenderLayers::from_layers(CAMERA_LAYER_WALL),
            YSort { z: 0.49 },
            Sprite::from_image(asset_server.load("textures/up_wall.png")),
            Transform::from_xyz(0.0, tile_size, 0.0)
                .with_scale(Vec3::new(tile_size / 32.0, tile_size / 32.0, 1.0)),
        ),(
            RenderLayers::from_layers(CAMERA_LAYER_WALL),
            YSort { z: 0.49 },
            Sprite::from_image(asset_server.load("textures/corner_wall.png")),
            Transform::from_xyz(-tile_size, tile_size, 0.0)
                .with_scale(Vec3::new(tile_size / 32.0, tile_size / 32.0, 1.0)),
        )],
    )).id();
}

fn generate_halo(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    asset_server: &Res<AssetServer>,
    texture_atlas_layouts: &mut ResMut<Assets<TextureAtlasLayout>>,
    center: IVec2,
) {
    let world_size_x = WORLD_SIZE/3;
    let world_size_y = WORLD_SIZE/3;
    let tile_size = TILE_SIZE;
    let radius = WORLD_SIZE as f32;
    let x_offset = (world_size_x as f32 * tile_size) / 2.0;
    let y_offset = (world_size_y as f32 * tile_size) / 2.0;
    let g_offset = (radius * tile_size) / 2.0;

    // atlas wody
    let water_texture = asset_server.load("textures/fog_black.png");
    let water_layout = TextureAtlasLayout::from_grid(UVec2::splat(32), 2, 2, None, None);
    let water_atlas = texture_atlas_layouts.add(water_layout);

    commands.spawn((FogHalo, Transform::from_xyz(0.0, 0.0, 3.14), InheritedVisibility::default())).with_children(|parent| {
        for gx in 0..world_size_x {
            for gy in 0..world_size_y {
                let dist2 = ((gx as f32 - x_offset/tile_size) * (gx as f32 - x_offset/tile_size) + (gy as f32 - y_offset/tile_size) * (gy as f32 - y_offset/tile_size)) as f32;
                if !(dist2*dist2 <= radius * radius) {
                    let x = gx as f32 * tile_size - x_offset + center.x as f32;
                    let y = gy as f32 * tile_size - y_offset + center.y as f32;

                    parent.spawn((
                        Mesh2d(meshes.add(Rectangle::new(tile_size, tile_size))),
                        //Transform::from_xyz(x, y, -(g_offset/64.0 + y/64.0)+64.0),
                        Transform::from_xyz(x, y, -32.0),
                         //RenderLayers::from_layers(CAMERA_LAYER_SPRITE),
                        children![(
                            {
                                let mut s = Sprite::from_atlas_image(
                                    water_texture.clone(),
                                    TextureAtlas { layout: water_atlas.clone(), index: 0 },
                                );
                                s.color = Color::srgba(0.25, 0.25, 0.25, 1.0); // odcień szarości + alfa
                                s
                            },
                                RenderLayers::from_layers(CAMERA_LAYER_EFFECT),
                            Transform {
                                scale: Vec3::new(tile_size / 32.0, tile_size / 32.0, 1.0),
                                ..Default::default()
                            },
                            AnimationIndices { first: 0, last: 3 },
                            AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                            WaterSprite,
                        )],
                    ));
                }
            }
        }
    });
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

/// marker że encja jest "split" stworzonym przez system (Y-part)
#[derive(Component)]
pub struct OccluderSplit {
    pub owner: Entity,
}

/// marker, że encja to Y-part (rozciągnięcie tylko w Y)
#[derive(Component)]
pub struct OccluderPartY;