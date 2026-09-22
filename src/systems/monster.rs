use bevy::prelude::*;
use rapier2d::prelude::*;

use crate::resourses::physics_resources::*;
use crate::systems::items::spawn_world_item;
use crate::systems::loader::Monster2AnimationLayouts;
use crate::systems::monster_ai::difficulty::{
    ActiveDifficulty, MonsterSenseConfig, population_config, sense_config,
};
use crate::systems::monster_ai::hivemind::{
    MonsterHiveMind, PackAlert, PackComms, PlayerEscapeModel, flank_side,
};
use crate::systems::monster_ai::pathfinding::{self, PathClaims};
use crate::systems::monster_ai::perception::{MonsterPerception, PlayerNoise};
use crate::systems::monster_ai::state::{self, MonsterState};
use crate::systems::physics::remove_rigid_body;
use crate::systems::terrain::{self, TerrainMap};
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::ecs::system::SystemParam;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct MonsterPlugin;

/// Which monster identity a spawned monster is — see `docs/monster2.md`.
/// `Monster1` is the original monster (no marker component beyond the
/// generic `Monster`); `Monster2` additionally gets the `Monster2` marker
/// component so future gameplay code can target it specifically. Adding a
/// Monster 3 later means adding one more variant here plus one more arm in
/// each of this enum's methods — nothing else needs a parallel type.
///
/// `Serialize`/`Deserialize` because a monster's kind is persisted in
/// `save::MonsterSaveData` — otherwise leaving with a Monster 2 nearby and
/// rejoining would restore it as a Monster 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MonsterKind {
    #[default]
    Monster1,
    Monster2,
}

impl MonsterKind {
    /// The `AtlasHandles` key (see `loader::init`) this kind's walk/idle
    /// animation is registered under.
    fn walk_animation_key(self) -> &'static str {
        match self {
            MonsterKind::Monster1 => "walk",
            MonsterKind::Monster2 => "walk2",
        }
    }
}

/// Monster 2's test-only stats — deliberately kept separate from
/// `MonsterCombatConfig` (Monster 1's tuned combat numbers) so a future
/// developer has one obvious, small place to find and change Monster 2's
/// placeholder HP/XP instead of hunting through Monster 1's config. See
/// `docs/monster2.md`.
#[derive(Resource)]
pub struct Monster2Config {
    /// HP a freshly spawned Monster 2 starts with.
    pub test_hp: f32,
    /// Random per-kill XP range granted for killing a Monster 2 — same
    /// mechanism as `MonsterCombatConfig::kill_xp_min`/`kill_xp_max`, just a
    /// separate range so Monster 2's reward can be tuned independently.
    pub test_xp_min: u32,
    pub test_xp_max: u32,
    /// Chance (0.0..=1.0) that a given spawn slot in `spawn_monsters_system`
    /// spawns a Monster 2 instead of a Monster 1 — the two kinds currently
    /// share one population budget (see `population_config`) rather than
    /// each getting their own cap, which is the simplest way to fold a
    /// second monster into the existing spawner without a parallel system.
    pub spawn_chance: f32,
}

#[derive(Resource)]
struct MonsterSpawnTimer(Timer);

/// Mirrors `terrain::TerrainResetPending`: set true while a Leave-triggered
/// despawn of all monsters is in progress, cleared once none remain
/// (including any still `Pending` and mid-flight through `loader::inspect`).
#[derive(Resource, Default)]
struct MonsterResetPending(bool);

/// Keeps the already large monster AI system below Bevy's system-parameter
/// limit while grouping the resources needed at the moment a monster dies:
/// loot materialization, plus Monster 2's test XP range (kill-time reward
/// data, same as the loot config) so `monster_ai` doesn't need a 17th
/// top-level parameter just for it.
#[derive(SystemParam)]
struct MonsterLootAssets<'w> {
    item_config: Res<'w, ItemConfig>,
    asset_server: Res<'w, AssetServer>,
    monster2: Res<'w, Monster2Config>,
    monster2_layouts: Res<'w, Monster2AnimationLayouts>,
}

#[derive(SystemParam)]
struct MonsterSenseInputs<'w> {
    difficulty: Res<'w, ActiveDifficulty>,
    noise: Res<'w, PlayerNoise>,
}

//use bevy_2d_screen_space_lightmaps::lightmap_plugin::lightmap_plugin::*;
use bevy::camera::visibility::RenderLayers;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_firefly::prelude::*;

#[derive(Resource)]
struct MonsterConfig {
    min_spawn_distance: f32,   // w tileach
    max_despawn_distance: f32, // w tileach
    tile_size: f32,            // piksele
    /// Half-extent of the monster's physical collider (matches the
    /// `Rectangle::new(40.0, 42.5)` spawn mesh), used to check that a
    /// candidate spawn point's whole body — not just its center point — is
    /// clear of walls/water.
    collider_half_extent: Vec2,
    /// Extra clearance beyond the collider itself required around a spawn
    /// point, so the monster doesn't spawn clipping/touching a wall's edge.
    spawn_clearance_margin: f32,
    /// How many candidate positions to try before giving up on spawning a
    /// given monster this tick (it'll be retried next tick).
    max_spawn_attempts: u32,
}

/// Melee combat numbers, centralized and difficulty-independent (difficulty
/// changes senses/memory/intelligence, never HP/damage — see
/// `monster_ai::difficulty`). `pub` so `lifecycle::handle_play_requested` can
/// hold a `Res<MonsterCombatConfig>` to pass through to `spawn_monster_at`
/// when restoring a loaded save's monsters — its fields stay private, it's
/// never read outside this module.
#[derive(Resource)]
pub struct MonsterCombatConfig {
    attack_range: f32,
    attack_cooldown_seconds: f32,
    attack_damage: f32,
    /// Movement speed while chasing/investigating (not attacking).
    move_speed: f32,
    /// Max A* nodes explored per path request — a hard cap so a single bad
    /// request can't spike a frame; exceeding it is treated as "no path".
    max_path_nodes: usize,
    /// How close to a waypoint counts as "arrived".
    waypoint_arrive_radius: f32,
    /// Below this much real-world movement per stuck-check window, while
    /// actively trying to travel somewhere, counts as "not making progress"
    /// (see `MonsterPerception::stuck_timer`) — e.g. a collider physically
    /// wedged on a wall corner the grid path didn't know to avoid.
    stuck_distance_threshold: f32,
    /// How far ahead the monster checks for walls/water before committing to
    /// a random wander direction, and before an unstick nudge — both are raw
    /// movement, not pathfinding, so they need their own terrain check.
    wander_lookahead: f32,
    /// Movement speed multiplier applied while a monster is outside the
    /// player's light (and line of sight to it) — see `PLAYER_LIGHT_RANGE`.
    dark_speed_multiplier: f32,
    /// Distance at which two monsters start gently pushing apart, so they
    /// don't overlap/crowd each other — see `resolve_monster_collisions`.
    separation_radius: f32,
    /// Tighter distance at which a lower-priority monster heading toward
    /// another one slows/stops instead of pushing through it — e.g. two
    /// monsters both funneling into the same one-tile-wide gap. Deliberately
    /// close to `separation_radius` so yielding is decided before the two
    /// are already deep inside a narrow corridor together.
    yield_radius: f32,
    /// How strongly the separation push (not the yield slowdown) is applied,
    /// in the same units as `move_speed`.
    separation_strength: f32,
    /// While strongly yielding, the monster also backs away a little (not
    /// just stopping in place) so it actually clears a one-tile corridor
    /// instead of standing in the doorway — same units as `move_speed`.
    yield_backoff_speed: f32,
    /// Soft A* cost added for routing through a tile another monster's
    /// current path already occupies — see `pathfinding::PathClaims`. Not a
    /// hard block; large enough to make a real detour preferable when one
    /// exists, small enough that a genuine choke point is still findable.
    path_claim_penalty: i64,
    /// How close a freshly picked investigate/search point can be to another
    /// monster's currently staked-out investigate target before it's nudged
    /// aside — see `deconflict_investigate_point`.
    investigate_claim_radius: f32,
    /// Lower bound of the random per-kill XP grant — see
    /// `MonsterKilledEvent::xp_reward`.
    kill_xp_min: u32,
    /// Upper bound (inclusive) of the random per-kill XP grant.
    kill_xp_max: u32,
}

impl Plugin for MonsterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MonsterSpawnTimer(Timer::from_seconds(
            1.0,
            TimerMode::Repeating,
        )))
        .insert_resource(MonsterConfig {
            min_spawn_distance: 20.0,   // spawn 20 kratek
            max_despawn_distance: 30.0, // despawn 30 kratek
            tile_size: 64.0,
            collider_half_extent: Vec2::new(20.0, 21.25),
            spawn_clearance_margin: 8.0,
            max_spawn_attempts: 20,
        })
        .insert_resource(MonsterCombatConfig {
            attack_range: 1.25 * TILE_SIZE,
            attack_cooldown_seconds: 2.0,
            attack_damage: 20.0,
            move_speed: 80.0,
            max_path_nodes: 800,
            waypoint_arrive_radius: 16.0,
            stuck_distance_threshold: 10.0,
            wander_lookahead: 1.5 * TILE_SIZE,
            dark_speed_multiplier: 2.5,
            separation_radius: 0.95 * TILE_SIZE,
            yield_radius: 0.8 * TILE_SIZE,
            separation_strength: 110.0,
            yield_backoff_speed: 35.0,
            path_claim_penalty: (4.0 * TILE_SIZE) as i64,
            investigate_claim_radius: 2.0 * TILE_SIZE,
            kill_xp_min: 15,
            kill_xp_max: 30,
        })
        // Monster 2's test-only HP/XP/spawn-mix numbers — see
        // `Monster2Config`'s doc comment. All 4 values are placeholders,
        // deliberately easy to find here and change later.
        .insert_resource(Monster2Config {
            test_hp: 60.0,
            test_xp_min: 15,
            test_xp_max: 30,
            spawn_chance: 0.25,
        })
        .insert_resource(MonsterHiveMind::default())
        .insert_resource(PlayerEscapeModel::default())
        .add_message::<MonsterKilledEvent>()
        .add_systems(
            Update,
            spawn_monsters_system
                .run_if(|status: Res<GameStatus>, status2: Res<ResumeStatus>| {
                    status.0 && !status2.0
                })
                .in_set(crate::systems::lifecycle::AppSet::Gameplay),
        )
        .add_systems(
            Update,
            (monster_ai, animate_monster_sprite)
                .run_if(|status: Res<GameStatus>, status2: Res<ResumeStatus>| {
                    status.0 && !status2.0
                })
                .in_set(crate::systems::lifecycle::AppSet::Gameplay),
        )
        .insert_resource(MonsterResetPending::default())
        .add_systems(
            Update,
            handle_world_reset.in_set(crate::systems::lifecycle::AppSet::Lifecycle),
        );
    }
}

/// Despawns every monster in reaction to Leave, including proper Rapier
/// physics teardown. Runs ungated (not behind the GameStatus run_if) since
/// it must keep working across frames while the session is already ending.
fn handle_world_reset(
    mut events: MessageReader<LeaveRequested>,
    mut commands: Commands,
    mut reset_pending: ResMut<MonsterResetPending>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
    mut island_manager: ResMut<ResIslandManager>,
    phys_query: Query<(Entity, &RigidBodyHandleComponent), (With<Monster>, Without<Pending>)>,
    non_phys_query: Query<
        Entity,
        (
            With<Monster>,
            Without<RigidBodyHandleComponent>,
            Without<Pending>,
        ),
    >,
    any_monster_query: Query<Entity, With<Monster>>,
) {
    let mut requested = false;
    for _ in events.read() {
        requested = true;
    }
    if requested {
        reset_pending.0 = true;
    }
    if !reset_pending.0 {
        return;
    }

    for (entity, handle) in &phys_query {
        remove_rigid_body(
            &mut rigid_bodies,
            &mut colliders,
            &mut island_manager,
            handle.0,
        );
        commands.entity(entity).despawn();
    }
    for entity in &non_phys_query {
        commands.entity(entity).despawn();
    }

    // Anything still `Pending` is mid-flight through `loader::inspect`;
    // leave it for a later frame (same race avoidance as terrain's reset).
    if any_monster_query.iter().next().is_some() {
        return;
    }
    reset_pending.0 = false;
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
    combat_config: Res<MonsterCombatConfig>,
    monster2_config: Res<Monster2Config>,
    atlas_handles: Res<AtlasHandles>,
    difficulty: Res<ActiveDifficulty>,
    terrain_map: Res<TerrainMap>,
    //mut images: ResMut<Assets<Image>>,
    //menu_root_query: Query<Entity, (With<HealthBar>, Without<DebugAI>)>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let max_monsters = population_config(difficulty.0).max_monsters;
    let current_count = existing_monsters.iter().count();
    if current_count >= max_monsters {
        return;
    }

    let player_transform = if let Ok(t) = player_query.single() {
        t
    } else {
        return;
    };

    // Both kinds' textures are loaded every tick spawning happens — cheap:
    // `asset_server.load` just returns/reuses a cached handle, it doesn't
    // decode anything itself. Which one actually gets used per spawn slot
    // is decided below by `monster2_config.spawn_chance`.
    let (texture, texture_atlas_layout) =
        load_monster_texture(&asset_server, &mut texture_atlas_layouts);
    let (texture2, texture_atlas_layout2) =
        load_monster2_texture(&asset_server, &mut texture_atlas_layouts);

    let spawn_distance = config.min_spawn_distance * config.tile_size;
    let to_spawn = max_monsters - current_count;

    // Half-extent a candidate spawn point's surrounding area must be clear
    // of walls/water in, for the monster's whole collider (not just its
    // center point) — see `terrain::is_area_clear`.
    let spawn_half_extent =
        config.collider_half_extent + Vec2::splat(config.spawn_clearance_margin);

    for _ in 0..to_spawn {
        let mut candidate = None;
        for _ in 0..config.max_spawn_attempts {
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            let distance = spawn_distance + rand::random::<f32>() * (0.3 * config.tile_size); // od 20 do 30 kratek
            let pos = Vec2::new(
                player_transform.translation.x + distance * angle.cos(),
                player_transform.translation.y + distance * angle.sin(),
            );
            // Wolne od ścian/wody — sprawdzone na realnym terenie jeśli jest
            // załadowany, w przeciwnym razie na tej samej mapie szumu, której
            // używa generowanie (bez wymuszania generacji terenu). The world
            // itself has no fixed edge to bounds-check against — terrain
            // generation is a sliding window centered on the player (see
            // terrain::update_terrain), so a fixed origin-centered box here
            // would (and did) stop spawning entirely once the player walked
            // far enough from world-space (0,0).
            if !terrain::is_area_clear(&terrain_map, pos, spawn_half_extent) {
                continue;
            }
            candidate = Some(pos);
            break;
        }
        let Some(pos) = candidate else {
            // No valid spot found this attempt budget (crowded area, map
            // edge, or a very wall-dense patch) — try again next spawn tick
            // rather than forcing a bad spawn.
            continue;
        };
        let kind = if rand::random::<f32>() < monster2_config.spawn_chance {
            MonsterKind::Monster2
        } else {
            MonsterKind::Monster1
        };
        let (kind_texture, kind_layout, health) = match kind {
            MonsterKind::Monster1 => (texture.clone(), texture_atlas_layout.clone(), 100.0),
            MonsterKind::Monster2 => (
                texture2.clone(),
                texture_atlas_layout2.clone(),
                monster2_config.test_hp,
            ),
        };
        spawn_monster_at(
            &mut commands,
            &mut meshes,
            kind_texture,
            kind_layout,
            &atlas_handles,
            &combat_config,
            kind,
            sense_config(difficulty.0).reaction_time,
            pos,
            health,
        );
        /*if spawned == false {
            for root in menu_root_query.iter() {
                if spawned == true {
                    break;
                }
                let child = commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(120.0),   // 1/5 szerokości
                        height: Val::Percent(1200.0),  // 1/5 wysokości
                        right: Val::Px(-1500.0),        // prawy róg
                        bottom: Val::Px(-450.0),
                        ..default()
                    },
                    ImageNode::new(image_handle.clone()),
                )).id();
                commands.entity(root).add_children(&[child]);
                commands.entity(root).insert(DebugAI);
                println!("Spawning monster at ({}, {})", pos.x, pos.y);
                spawned = true;
            }
        }*/
    }
}

/// Loads the monster spritesheet + its atlas layout — split out of
/// `spawn_monsters_system` so `lifecycle::handle_play_requested` can load it
/// once too, when restoring a loaded save's monsters via `spawn_monster_at`.
pub fn load_monster_texture(
    asset_server: &AssetServer,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
) -> (Handle<Image>, Handle<TextureAtlasLayout>) {
    let texture = asset_server.load("textures/monster_combined.png");
    let layout = TextureAtlasLayout::from_grid(bevy::prelude::UVec2::splat(64), 2, 5, None, None);
    (texture, texture_atlas_layouts.add(layout))
}

/// Loads Monster 2's spritesheet + its atlas layout — mirrors
/// `load_monster_texture`. The layout only covers `monster2_combined.png`'s
/// top (walk) section: an 80x80, 4-column x 2-row grid exactly matching the
/// original `monster2.png`'s dimensions (320x160) placed at the top of the
/// combined image, giving the 8 frames registered as the `"walk2"`
/// `AnimationIndices` in `loader::init`. The attack/jump sections below it
/// use different cell sizes (see `docs/monster2.md`) and have no layout
/// defined yet — add a second `TextureAtlasLayout::from_grid` with an
/// `offset` when wiring those up.
pub fn load_monster2_texture(
    asset_server: &AssetServer,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
) -> (Handle<Image>, Handle<TextureAtlasLayout>) {
    let texture = asset_server.load("textures/monster2_combined.png");
    let layout = TextureAtlasLayout::from_grid(bevy::prelude::UVec2::new(80, 80), 4, 2, None, None);
    (texture, texture_atlas_layouts.add(layout))
}

/// Spawns one monster of the given `kind` at `pos` with the given
/// `health` — the exact bundle `spawn_monsters_system` used to build
/// inline, parameterized so `lifecycle::handle_play_requested` can reuse it
/// verbatim to restore a loaded save's monsters (position + HP + kind)
/// instead of duplicating the bundle. A freshly-spawned wandering monster
/// and a save-restored one only ever differ in where they start and how
/// much health they have; two different kinds only ever differ in their
/// sprite/animation and the `Monster2` marker — everything else (physics,
/// movement, collision, death handling) is identical, which is exactly why
/// `kind` is just one more parameter here rather than a second function.
#[allow(clippy::too_many_arguments)]
pub fn spawn_monster_at(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    texture: Handle<Image>,
    texture_atlas_layout: Handle<TextureAtlasLayout>,
    atlas_handles: &AtlasHandles,
    combat_config: &MonsterCombatConfig,
    kind: MonsterKind,
    reaction_time: f32,
    pos: Vec2,
    health: f32,
) {
    let monster_animation_indices = atlas_handles
        .0
        .get(kind.walk_animation_key())
        .unwrap()
        .clone();
    let mut entity_commands = commands.spawn((
        Monster,
        MonsterAI {
            random_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
            random_dir: Vec2::ZERO,
            action_cooldown: Timer::from_seconds(
                combat_config.attack_cooldown_seconds,
                TimerMode::Once,
            ),
            health,
            last_health: health,
            stun_cooldown: Timer::from_seconds(0.375, TimerMode::Once),
        },
        MonsterPerception::new(reaction_time),
        RenderLayers::from_layers(CAMERA_LAYER_EFFECT),
        Pending,
        Mesh2d(meshes.add(Rectangle::new(40.0, 42.5))),
        Transform::from_xyz(pos.x, pos.y, -32.0),
        children![
            (
                Sprite::from_atlas_image(
                    texture.clone(),
                    bevy::prelude::TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: monster_animation_indices.first,
                    },
                ),
                YSort { z: 0.375 },
                Transform::from_xyz(0.0, 37.5, 64.0).with_scale(Vec3::splat(2.0)),
                RenderLayers::from_layers(CAMERA_LAYER_MONSTER),
                monster_animation_indices,
                AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                MonsterSprite,
                AttackStatus(false),
                FinishStatus(false),
            ),
            (
                Transform::from_xyz(0.0, 15.0, 0.0),
                PointLight2d {
                    range: 375.0,
                    intensity: 0.075,
                    color: Color::srgba(1.0, 0.5, 0.0, 1.0),
                    ..default()
                },
                YSort { z: 0.0 },
            )
        ],
    ));
    if kind == MonsterKind::Monster2 {
        entity_commands.insert((
            Monster2,
            Monster2Leap {
                cooldown: Timer::from_seconds(4.5, TimerMode::Once),
            },
        ));
    }
}

fn create_ai_texture(images: &mut Assets<Image>, width: u32, height: u32) -> Handle<Image> {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        data: vec![0; (width * height * 4) as usize].into(),
        ..default()
    };

    images.add(image)
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

type MonsterSpriteQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut AnimationIndices,
        &'static mut AttackStatus,
        &'static mut FinishStatus,
        &'static mut Sprite,
    ),
    With<MonsterSprite>,
>;

fn monster_ai(
    time: Res<Time>,
    mut player_query: Query<(&Transform, &mut PlayerData), (With<Player>, Without<Pending>)>,
    mut query: Query<
        (
            &mut MonsterAI,
            &mut MonsterPerception,
            &mut RigidBodyHandleComponent,
            &mut Transform,
            Entity,
            &Children,
            Option<&Monster2>,
            Option<&mut Monster2Leap>,
        ),
        (With<Monster>, Without<Player>, Without<Pending>),
    >,
    mut child_query: MonsterSpriteQuery,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
    mut island_manager: ResMut<ResIslandManager>,
    mut commands: Commands,
    mut killed: MessageWriter<MonsterKilledEvent>,
    config: Res<MonsterConfig>,
    combat_config: Res<MonsterCombatConfig>,
    atlas_handles: Res<AtlasHandles>,
    terrain_map: Res<TerrainMap>,
    sense_inputs: MonsterSenseInputs,
    loot_assets: MonsterLootAssets,
    mut pack: PackComms,
) {
    let (player_transform, mut player_data_some): (Transform, Option<Mut<PlayerData>>) =
        if let Ok((t, d)) = player_query.single_mut() {
            (t.clone(), Some(d)) // <- klonujemy Transform, żeby mieć wartość
        } else {
            (Transform::default(), None)
        };

    let despawn_distance = config.max_despawn_distance * TILE_SIZE;
    let now = time.elapsed_secs();
    let sense_cfg = sense_config(sense_inputs.difficulty.0);
    let player_pos = player_transform.translation.xy();
    let has_player = player_data_some.is_some();
    // Alerts broadcast by packmates during the *previous* pass — consulted
    // this pass, while this pass's own fresh sightings accumulate fresh into
    // `hive_mind.alerts` (now empty) for the *next* pass to consult. Keeps
    // pack telepathy one frame stale instead of depending on iteration order
    // between monsters processed earlier/later in the same query.
    let alerts_snapshot = std::mem::take(&mut pack.hive_mind.alerts);
    // Snapshot of the pack's learned escape-direction estimate for this pass
    // (see `PlayerEscapeModel`) — only meaningfully populated on Hard
    // (`share_full_memory`); zero elsewhere, which naturally makes every use
    // of it below a no-op on Easy/Normal.
    let escape_bias = if sense_cfg.share_full_memory {
        pack.escape_model.avg_flee_dir
    } else {
        Vec2::ZERO
    };

    // A snapshot of every monster's position (from `Transform`, synced last
    // physics step — a frame stale, which is fine for this) taken before the
    // mutable loop below, so each monster can check its distance to every
    // *other* monster without needing two overlapping mutable borrows of the
    // same query. The same pass also collects what each monster currently
    // has staked out — its investigate target (so a monster about to pick a
    // new one can avoid a spot someone else already planned) and every tile
    // it currently occupies *or* is walking through (so pathfinding can
    // softly steer a packmate around a monster that's standing somewhere —
    // e.g. fighting the player in a doorway — not just one mid-transit,
    // instead of independently computing a route straight through it).
    let mut monster_snapshot: Vec<(Entity, Vec2)> = Vec::new();
    let mut investigate_claims: Vec<(Entity, Vec2)> = Vec::new();
    let mut path_claims: HashMap<IVec2, Entity> = HashMap::new();
    for (_, perception, _, transform, entity, _, _, _) in query.iter() {
        let pos = transform.translation.xy();
        monster_snapshot.push((entity, pos));
        if let Some(target) = perception.investigate_target {
            investigate_claims.push((entity, target));
        }
        path_claims
            .entry(pathfinding::world_to_tile(pos))
            .or_insert(entity);
        for &waypoint in &perception.path {
            path_claims
                .entry(pathfinding::world_to_tile(waypoint))
                .or_insert(entity);
        }
    }

    for (
        mut ai,
        mut perception,
        rb_handle,
        mut rb_transform,
        entity,
        children,
        monster2,
        mut leap,
    ) in &mut query
    {
        let is_monster2 = monster2.is_some();
        if let Some(rigid_body) = rigid_bodies.0.get_mut(rb_handle.0) {
            let monster_pos = Vec2::new(
                rigid_body.position().translation.x,
                rigid_body.position().translation.y,
            );
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
                    colliders.0.remove(
                        collider_handle,
                        &mut island_manager.0,
                        &mut rigid_bodies.0,
                        true,
                    );
                }
                rigid_bodies.0.remove(
                    rb_handle.0,
                    &mut island_manager.0,
                    &mut colliders.0,
                    &mut ImpulseJointSet::new(),
                    &mut MultibodyJointSet::new(),
                    true, // usuwa powiązane collidery
                );
                commands.entity(entity).despawn();
                continue;
            }
            if ai.health <= 0.0 {
                // Monster 2 uses its own separate, easy-to-find test XP
                // range (`Monster2Config`) instead of Monster 1's tuned
                // `MonsterCombatConfig` range — see `docs/monster2.md`. The
                // death handling below (physics teardown, despawn) and the
                // XP grant itself (`MonsterKilledEvent` -> `quests::progress
                // ::track_kills` -> `PlayerLevel::add_xp`) are entirely
                // shared, unmodified for both kinds.
                let (xp_min, xp_max) = if is_monster2 {
                    (
                        loot_assets.monster2.test_xp_min,
                        loot_assets.monster2.test_xp_max,
                    )
                } else {
                    (combat_config.kill_xp_min, combat_config.kill_xp_max)
                };
                let xp_reward = rand::thread_rng().gen_range(xp_min..=xp_max);
                killed.write(MonsterKilledEvent { xp_reward });
                // Loot is Monster 1-specific flavor (its apple drop) — not
                // part of Monster 2's basic test scope, see
                // `docs/monster2.md` for where to add Monster 2 loot later.
                if !is_monster2 && let Some(apple) = loot_assets.item_config.items.get("apple_red")
                {
                    let apple_count = rand::thread_rng().gen_range(2..=4);
                    for index in 0..apple_count {
                        let mut dropped_apple = apple.clone();
                        dropped_apple.amount = 1;
                        let angle = index as f32 / apple_count as f32 * std::f32::consts::TAU;
                        let position = monster_pos + Vec2::from_angle(angle) * 18.0;
                        spawn_world_item(
                            &mut commands,
                            &loot_assets.asset_server,
                            dropped_apple,
                            position,
                        );
                    }
                }
                let mut colliders_clone = Vec::new();
                if let Some(rb) = rigid_bodies.0.get(rb_handle.0) {
                    for collider_handle in rb.colliders() {
                        colliders_clone.push(collider_handle.clone());
                    }
                }

                for collider_handle in colliders_clone {
                    colliders.0.remove(
                        collider_handle,
                        &mut island_manager.0,
                        &mut rigid_bodies.0,
                        true,
                    );
                }
                rigid_bodies.0.remove(
                    rb_handle.0,
                    &mut island_manager.0,
                    &mut colliders.0,
                    &mut ImpulseJointSet::new(),
                    &mut MultibodyJointSet::new(),
                    true, // usuwa powiązane collidery
                );
                commands.entity(entity).despawn();
                continue;
            }

            // --- Perception: re-evaluate senses/state at a throttled interval
            // (not every frame — see MonsterSenseConfig::reaction_time). ---
            // Monster 2 deliberately never runs `state::evaluate` — that's
            // the one function that can ever move a monster out of `Idle`
            // (into Chase/Investigate/Attack), so skipping it here is the
            // entire "no player detection/aggro/chasing/attacking" behavior
            // for Monster 2 — see `docs/monster2.md`'s AI section. It falls
            // into the same branch as "no player exists yet" below, which
            // already does exactly what a passive wanderer needs: force
            // `Idle` and let the `MonsterState::Idle` match arm's existing
            // wander/pathfinding-avoidance code (unchanged, shared with
            // Monster 1) drive its movement.
            perception.sense_timer.tick(time.delta());
            if has_player {
                if perception.sense_timer.just_finished() {
                    state::evaluate(
                        &mut perception,
                        &terrain_map,
                        monster_pos,
                        player_pos,
                        &sense_inputs.noise,
                        &sense_cfg,
                        combat_config.attack_range,
                        now,
                        entity,
                        &monster_snapshot,
                    );

                    // Darkness: outside the player's own light (or blocked
                    // from it), a monster moves faster — reusing the same
                    // range+LOS check vision already uses, just without a
                    // facing cone (light radiates in every direction).
                    perception.in_darkness = !(distance <= PLAYER_LIGHT_RANGE
                        && pathfinding::line_of_sight(&terrain_map, monster_pos, player_pos));

                    // Pack sharing (off on Easy, minimal on Normal, full on
                    // Hard — see MonsterSenseConfig doc comments): a monster
                    // that currently trusts its own info about the player —
                    // live this tick, or (Hard only) still-standing memory —
                    // broadcasts it for packmates within `communication_range`
                    // to react to next pass. A monster with no better info of
                    // its own investigates the *nearest* broadcast, offset to
                    // one side so it approaches from a different angle than
                    // whoever it heard it from. A monster that's *also*
                    // currently engaged doesn't drop its own info for someone
                    // else's — instead it and its engaged packmates settle
                    // who's the "primary" (direct) pursuer and offset
                    // everyone else's live Chase target too, via
                    // `pack_flank_offset` — so a pack that all has eyes on
                    // the player still spreads out instead of funneling
                    // through the same gap as one blob. The offset scales
                    // with distance to the target, so it's subtle up close
                    // but genuinely routes around a big obstacle (a lake, a
                    // mountain, a wall cluster) from far away.
                    perception.pack_flank_offset = Vec2::ZERO;
                    if sense_cfg.telepathy_enabled {
                        let fresh_sighting = perception.last_seen_time == now;
                        let memory_age = now - perception.last_seen_time;
                        let trusts_own_memory = perception.last_seen_pos.is_some()
                            && memory_age <= sense_cfg.memory_duration;
                        let sharing_own_info =
                            fresh_sighting || (sense_cfg.share_full_memory && trusts_own_memory);

                        if sharing_own_info {
                            let known_pos = perception.last_seen_pos.unwrap();
                            pack.hive_mind.alerts.push(PackAlert {
                                source: entity,
                                source_pos: monster_pos,
                                player_pos: known_pos,
                            });
                            // Basic shared "learning": a fresh, personally
                            // confirmed sighting with real movement teaches
                            // the whole pack a little about which way the
                            // player tends to run (Hard only — the model is
                            // never touched on Easy/Normal).
                            if sense_cfg.share_full_memory && fresh_sighting {
                                pack.escape_model.observe(perception.last_seen_velocity);
                            }

                            // Among packmates also currently sharing live info
                            // nearby, the lowest entity index leads the direct
                            // pursuit; everyone else flanks.
                            let am_primary = alerts_snapshot.iter().all(|a| {
                                a.source == entity
                                    || monster_pos.distance(a.source_pos)
                                        > sense_cfg.communication_range
                                    || a.source.index().index() > entity.index().index()
                            });
                            if !am_primary {
                                let side = flank_side(entity);
                                perception.pack_flank_offset = flank_offset(
                                    monster_pos,
                                    known_pos,
                                    side,
                                    sense_cfg.flank_offset_fraction,
                                    sense_cfg.flank_offset_max,
                                );
                            }
                        } else if matches!(
                            perception.state,
                            MonsterState::Idle | MonsterState::Investigate
                        ) {
                            if let Some(alert) = alerts_snapshot
                                .iter()
                                .filter(|a| {
                                    a.source != entity
                                        && monster_pos.distance(a.source_pos)
                                            <= sense_cfg.communication_range
                                })
                                .min_by(|a, b| {
                                    monster_pos
                                        .distance(a.source_pos)
                                        .total_cmp(&monster_pos.distance(b.source_pos))
                                })
                            {
                                let side = flank_side(entity);
                                let flanked = alert.player_pos
                                    + flank_offset(
                                        monster_pos,
                                        alert.player_pos,
                                        side,
                                        sense_cfg.flank_offset_fraction,
                                        sense_cfg.flank_offset_max,
                                    );
                                // Investigate claims are atomic: don't also
                                // head for a spot another monster already
                                // has staked out as of last pass.
                                let target = deconflict_investigate_point(
                                    flanked,
                                    entity,
                                    &investigate_claims,
                                    combat_config.investigate_claim_radius,
                                );

                                // Only reset the investigate walk on genuinely
                                // new information — otherwise a packmate
                                // still sharing every tick would keep
                                // resetting progress before a single leg ever
                                // completes.
                                let is_new_info = perception.state == MonsterState::Idle
                                    || !perception
                                        .investigate_target
                                        .is_some_and(|t| t.distance(target) <= TILE_SIZE);
                                if is_new_info {
                                    perception.investigate_target = Some(target);
                                    perception.investigate_started = now;
                                    perception.investigate_leg = 0;
                                    perception.investigate_subtarget = None;
                                }
                                perception.last_heard_pos = Some(target);
                                perception.last_heard_time = now;
                                perception.state = MonsterState::Investigate;
                            }
                        }
                    }
                }
            } else {
                perception.state = MonsterState::Idle;
                perception.in_darkness = false;
                perception.pack_flank_offset = Vec2::ZERO;
            }

            let move_speed = combat_config.move_speed
                * if perception.in_darkness {
                    combat_config.dark_speed_multiplier
                } else {
                    1.0
                };

            ai.action_cooldown.tick(time.delta());

            let mut velocity = Vec2::ZERO;

            match perception.state {
                MonsterState::Attack => {
                    // The whole point of the CHASE/ATTACK split: stop issuing
                    // movement toward the player and let the animation
                    // lifecycle below own the hit, instead of shoving into them.
                    perception.facing = (player_pos - monster_pos).normalize_or_zero();

                    if let Ok((mut child_indices, mut attack, mut finish, mut sprite)) =
                        child_query.get_mut(children[0])
                    {
                        if !attack.0 && !finish.0 && ai.action_cooldown.is_finished() {
                            attack.0 = true;
                            ai.action_cooldown.reset();
                            let animation_indices = atlas_handles
                                .0
                                .get(if is_monster2 { "attack2" } else { "attack" })
                                .unwrap()
                                .clone();
                            if let Some(atlas) = &mut sprite.texture_atlas {
                                atlas.index = animation_indices.first;
                                if is_monster2 {
                                    atlas.layout = loot_assets.monster2_layouts.attack.clone();
                                }
                            }
                            *child_indices = animation_indices;
                        }
                        if finish.0 {
                            finish.0 = false;
                            ai.action_cooldown.reset();
                            if is_monster2 {
                                if let Some(atlas) = &mut sprite.texture_atlas {
                                    atlas.layout = loot_assets.monster2_layouts.walk.clone();
                                    atlas.index = atlas_handles.0.get("walk2").unwrap().first;
                                }
                                *child_indices = atlas_handles.0.get("walk2").unwrap().clone();
                            }
                            // Hit frame: only land the hit if the player is
                            // still actually in range right now, not just
                            // when the swing started.
                            if let Some(ref mut player_data) = player_data_some {
                                if monster_pos.distance(player_pos) <= combat_config.attack_range {
                                    player_data.damage(combat_config.attack_damage);
                                    player_data.can_heal.reset();
                                }
                            }
                        }
                    }
                }
                MonsterState::Idle => {
                    clear_attack_visuals(&mut child_query, children);
                    perception.path.clear();
                    perception.path_index = 0;

                    ai.random_timer.tick(time.delta());
                    if ai.random_timer.just_finished() {
                        ai.random_dir = pick_wander_direction(
                            &terrain_map,
                            monster_pos,
                            config.collider_half_extent,
                            combat_config.wander_lookahead,
                        );
                    }
                    velocity = ai.random_dir * move_speed;
                    if ai.random_dir.length_squared() > 0.0001 {
                        perception.facing = ai.random_dir;
                    } else {
                        // Every wander direction was blocked within
                        // lookahead range (e.g. boxed into a corner) — keep
                        // slowly turning to scan around instead of freezing
                        // facing one fixed way until the next wander retry
                        // (up to `random_timer`'s full 2s), which could
                        // otherwise leave the player standing right behind
                        // it undetected the whole time.
                        const IDLE_SCAN_RADIANS_PER_SEC: f32 = 1.2;
                        let rotation =
                            Vec2::from_angle(IDLE_SCAN_RADIANS_PER_SEC * time.delta_secs());
                        perception.facing = rotation.rotate(perception.facing);
                    }
                }
                MonsterState::Chase | MonsterState::Investigate => {
                    clear_attack_visuals(&mut child_query, children);

                    // Monster 2 occasionally uses its jump sheet to close a
                    // medium-sized gap. The cooldown and distance window keep
                    // it readable and prevent a permanent dash state.
                    if is_monster2 {
                        if let Some(leap) = leap.as_deref_mut() {
                            leap.cooldown.tick(time.delta());
                            let can_leap = perception.state == MonsterState::Chase
                                && distance > 1.75 * TILE_SIZE
                                && distance < 5.5 * TILE_SIZE
                                && leap.cooldown.is_finished();
                            if can_leap {
                                let direction = (player_pos - monster_pos).normalize_or_zero();
                                velocity = direction * combat_config.move_speed * 5.0;
                                perception.facing = direction;
                                leap.cooldown.reset();
                                if let Ok((mut child_indices, mut attack, _, mut sprite)) =
                                    child_query.get_mut(children[0])
                                {
                                    let jump = atlas_handles.0.get("jump2").unwrap().clone();
                                    *child_indices = jump.clone();
                                    attack.0 = true;
                                    if let Some(atlas) = &mut sprite.texture_atlas {
                                        atlas.layout = loot_assets.monster2_layouts.jump.clone();
                                        atlas.index = jump.first;
                                    }
                                }
                            }
                        }
                    }

                    if velocity == Vec2::ZERO
                        && let Some(target) = nav_target_for(
                            &mut perception,
                            &sense_cfg,
                            monster_pos,
                            combat_config.waypoint_arrive_radius,
                            escape_bias,
                            entity,
                            &investigate_claims,
                            combat_config.investigate_claim_radius,
                            now,
                        )
                    {
                        velocity = seek_along_path(
                            &mut perception,
                            &terrain_map,
                            monster_pos,
                            target,
                            &time,
                            &config,
                            &combat_config,
                            move_speed,
                            entity,
                            &monster_snapshot,
                            &path_claims,
                        );
                        if velocity.length_squared() > 0.0001 {
                            perception.facing = velocity.normalize();
                        }
                    }
                }
            }

            // Monster-vs-monster collision resolution: pathfinding only
            // reasons about walls, so two monsters can still independently
            // compute paths that send them through the same tight gap at the
            // same time. Not while attacking (that velocity is already zero
            // and shouldn't be perturbed mid-swing).
            if perception.state != MonsterState::Attack {
                velocity = resolve_monster_collisions(
                    entity,
                    monster_pos,
                    velocity,
                    &monster_snapshot,
                    &combat_config,
                    &config,
                    &terrain_map,
                );
            }

            let move_dir = if velocity.length_squared() > 0.0001 {
                velocity.normalize()
            } else {
                perception.facing
            };

            if ai.health < ai.last_health {
                // obrażenia, cofamy się
                let knockback = -move_dir * combat_config.move_speed * 3.14 / 2.0;
                ai.last_health = (ai.health * 2.0 + ai.last_health) / 3.0;
                rigid_body.set_linvel(vector![knockback.x, knockback.y], true);
            } else {
                ai.last_health = ai.health;
                if ai.stun_cooldown.just_finished() {
                    rigid_body.set_linvel(vector![velocity.x, velocity.y], true);
                } else {
                    ai.stun_cooldown.tick(time.delta());
                }
            }
            if move_dir.x < 0.0 {
                if rb_transform.scale.x < 0.0 {
                    rb_transform.scale.x *= -1.0;
                }
            } else {
                if rb_transform.scale.x > 0.0 {
                    rb_transform.scale.x *= -1.0;
                }
            }
        }
    }
}

/// Resets the attack-animation marker components whenever the monster isn't
/// mid-swing, so leaving Attack state doesn't strand the sprite on a stale
/// "finish" flag from a previous encounter.
fn clear_attack_visuals(child_query: &mut MonsterSpriteQuery, children: &Children) {
    if let Ok((_, attack, mut finish, _)) = child_query.get_mut(children[0]) {
        if !attack.0 {
            finish.0 = false;
        }
    }
}

/// Picks the world-space point a Chase/Investigate monster should currently
/// be walking toward. Chase follows (optionally Hard-predicted) last-known
/// player info; Investigate walks to the point of interest and then samples
/// a few random nearby "search" points around it while its window lasts.
/// `escape_bias` is the pack's learned escape-direction estimate (see
/// `PlayerEscapeModel`) — zero on anything but Hard — used as a fallback
/// prediction direction and to lean search points toward historically
/// likely escape routes instead of pure chance. `investigate_claims` lists
/// every *other* monster's currently staked-out investigate point, so a
/// freshly picked search leg doesn't land on the same spot someone else is
/// already covering.
fn nav_target_for(
    perception: &mut MonsterPerception,
    cfg: &MonsterSenseConfig,
    monster_pos: Vec2,
    arrive_radius: f32,
    escape_bias: Vec2,
    entity: Entity,
    investigate_claims: &[(Entity, Vec2)],
    investigate_claim_radius: f32,
    now: f32,
) -> Option<Vec2> {
    const MAX_SEARCH_LEGS: u8 = 3;
    const SEARCH_BIAS_STRENGTH: f32 = 0.55;

    match perception.state {
        MonsterState::Chase => state::predicted_chase_target(perception, cfg, escape_bias, now)
            .map(|p| p + perception.pack_flank_offset),
        MonsterState::Investigate => {
            let anchor = perception.investigate_target?;
            let need_new_leg = match perception.investigate_subtarget {
                None => true,
                Some(sub) => monster_pos.distance(sub) <= arrive_radius,
            };
            if need_new_leg {
                if perception.investigate_leg == 0 {
                    perception.investigate_subtarget = Some(anchor);
                    perception.investigate_leg = 1;
                } else if perception.investigate_leg < MAX_SEARCH_LEGS {
                    let point = state::pick_search_point(
                        anchor,
                        cfg.investigate_radius,
                        escape_bias,
                        SEARCH_BIAS_STRENGTH,
                    );
                    perception.investigate_subtarget = Some(deconflict_investigate_point(
                        point,
                        entity,
                        investigate_claims,
                        investigate_claim_radius,
                    ));
                    perception.investigate_leg += 1;
                } else {
                    perception.investigate_subtarget = Some(anchor);
                }
            }
            perception.investigate_subtarget
        }
        _ => None,
    }
}

/// Moves a monster toward `target` by following (and, when needed,
/// recomputing) its A* path — never a straight line through walls. Repathing
/// is throttled by `perception.path_timer` and only attempted when the path
/// is stale (target moved meaningfully) or exhausted, per the "don't
/// recalculate every frame" requirement.
///
/// The grid path only reasons about whole tiles being walkable — it doesn't
/// know the monster's physical collider can still clip a wall's corner (e.g.
/// cutting a turn) and get physically wedged there even though the path
/// itself is fine. `perception.stuck_timer` watches for "commanded to move,
/// but not actually making progress" and reacts with a brief perpendicular
/// push plus a forced repath, instead of pushing into the same spot forever
/// — except while yielding to a nearby monster (see
/// `resolve_monster_collisions`), which looks identical (not moving) but
/// isn't actually stuck and would otherwise wrongly trigger a nudge+repath
/// the moment the other monster clears out of the way anyway.
fn seek_along_path(
    perception: &mut MonsterPerception,
    map: &TerrainMap,
    monster_pos: Vec2,
    target: Vec2,
    time: &Time,
    config: &MonsterConfig,
    combat_config: &MonsterCombatConfig,
    move_speed: f32,
    entity: Entity,
    monster_snapshot: &[(Entity, Vec2)],
    path_claims: &HashMap<IVec2, Entity>,
) -> Vec2 {
    let now = time.elapsed_secs();

    if now < perception.unstick_until {
        return perception.unstick_dir * move_speed;
    }

    perception.path_timer.tick(time.delta());

    let path_exhausted = perception.path_index >= perception.path.len();
    let arrived = monster_pos.distance(target) <= combat_config.waypoint_arrive_radius;
    let target_moved = perception.path_computed_for.distance(target) > TILE_SIZE * 1.5;

    // Ran out of waypoints without actually reaching the target (path was
    // short, or the target drifted since it was computed) -> get a fresh
    // path right away. Otherwise, only reconsider on the throttled timer.
    let should_repath = !perception.has_path_target
        || (path_exhausted && !arrived && perception.path_found)
        || (perception.path_timer.just_finished()
            && (target_moved || (path_exhausted && !perception.path_found)));

    if should_repath {
        let claims = PathClaims {
            claimed_by: path_claims,
            self_entity: entity,
            penalty: combat_config.path_claim_penalty,
        };
        match pathfinding::find_path(
            map,
            monster_pos,
            target,
            combat_config.max_path_nodes,
            Some(&claims),
        ) {
            Some(new_path) => {
                perception.path = new_path;
                perception.path_index = 0;
                perception.path_found = true;
            }
            None => {
                perception.path.clear();
                perception.path_index = 0;
                perception.path_found = false;
            }
        }
        perception.path_computed_for = target;
        perception.has_path_target = true;
    }

    if !perception.path_found {
        return Vec2::ZERO;
    }

    while perception.path_index < perception.path.len()
        && monster_pos.distance(perception.path[perception.path_index])
            <= combat_config.waypoint_arrive_radius
    {
        perception.path_index += 1;
    }

    let next_point = match perception.path.get(perception.path_index) {
        Some(&p) => p,
        None => target,
    };

    let to_point = next_point - monster_pos;
    let seeking = to_point.length() > combat_config.waypoint_arrive_radius;

    // Only sample progress while genuinely trying to travel somewhere — a
    // monster that legitimately arrived and is standing still must not be
    // flagged as stuck. Likewise, a monster currently yielding right of way
    // to a *higher-priority* nearby monster (see `resolve_monster_collisions`
    // — same lowest-index-wins convention) is stationary on purpose, not
    // stuck. Deliberately NOT suppressed for the monster *with* right of way:
    // if it's still not making progress (e.g. the yielder hasn't backed off
    // far enough yet), it needs its own stuck-recovery to keep firing —
    // otherwise both sides could end up silently waiting forever.
    let yielding_to_packmate = monster_snapshot.iter().any(|&(other, pos)| {
        other != entity
            && monster_pos.distance(pos) < combat_config.yield_radius
            && entity.index().index() > other.index().index()
    });

    perception.stuck_timer.tick(time.delta());
    if perception.stuck_timer.just_finished() {
        if seeking
            && !yielding_to_packmate
            && monster_pos.distance(perception.stuck_probe_pos)
                < combat_config.stuck_distance_threshold
        {
            perception.stuck_ticks = perception.stuck_ticks.saturating_add(1);
        } else {
            perception.stuck_ticks = 0;
        }
        perception.stuck_probe_pos = monster_pos;
    }

    if perception.stuck_ticks >= 2 {
        perception.stuck_ticks = 0;
        // Invalidate the path so the next call gets a fresh one computed
        // from wherever the nudge below actually ends up.
        perception.has_path_target = false;

        // Try both perpendicular sides, then straight back, and only commit
        // to a direction that's actually clear of walls/water a short
        // distance out — a raw nudge could otherwise shove the monster from
        // one wedge straight into another (or into water).
        let base = to_point.normalize_or_zero();
        let perp = if base.length_squared() > 0.0001 {
            base.perp()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let first_side = if rand::random::<bool>() { 1.0 } else { -1.0 };
        let candidates = [perp * first_side, perp * -first_side, -base];

        let mut nudge = Vec2::ZERO;
        for candidate in candidates {
            if candidate.length_squared() < 0.0001 {
                continue;
            }
            let dir = candidate.normalize();
            let probe = monster_pos + dir * combat_config.wander_lookahead;
            if terrain::is_area_clear(map, probe, config.collider_half_extent) {
                nudge = dir;
                break;
            }
        }

        perception.unstick_dir = nudge;
        perception.unstick_until = now + 0.25;
        return nudge * move_speed;
    }

    if !seeking {
        Vec2::ZERO
    } else {
        to_point.normalize_or_zero() * move_speed
    }
}

/// Adjusts an intended velocity so monsters don't crowd or shove through
/// each other: every other monster within `separation_radius` gently pushes
/// this one away (stronger the closer they are), and if this monster is
/// heading toward one that's even closer (within `yield_radius`), it slows
/// or stops instead of pushing through — the lower `Entity` index wins right
/// of way (same convention as the pack's flanking "primary"), so exactly one
/// side of any given pair yields rather than both dithering. A monster that's
/// strongly yielding also backs off a little instead of just stopping dead,
/// so it actually clears a one-tile corridor instead of standing in the
/// doorway blocking the one with right of way.
///
/// The separation/yield push itself has no notion of walls or water — purely
/// distance-based repulsion from another monster's position could otherwise
/// shove a monster straight into either near a tight spot (e.g. two monsters
/// crowded at a doorway next to a lake). So the result is checked against
/// terrain a short distance out before being used; if it would head into
/// something solid, the push/backoff is dropped and only the (already
/// terrain-safe, since it's whatever the caller already validated) yield-
/// scaled base velocity is kept.
#[allow(clippy::too_many_arguments)]
fn resolve_monster_collisions(
    entity: Entity,
    monster_pos: Vec2,
    velocity: Vec2,
    monster_snapshot: &[(Entity, Vec2)],
    combat_config: &MonsterCombatConfig,
    config: &MonsterConfig,
    map: &TerrainMap,
) -> Vec2 {
    let mut separation = Vec2::ZERO;
    let mut yield_factor = 1.0f32;

    for &(other_entity, other_pos) in monster_snapshot {
        if other_entity == entity {
            continue;
        }
        let to_other = other_pos - monster_pos;
        let dist = to_other.length();
        if dist < 0.0001 || dist >= combat_config.separation_radius {
            continue;
        }
        let away = -to_other / dist;
        let push_strength =
            (combat_config.separation_radius - dist) / combat_config.separation_radius;
        separation += away * push_strength;

        if dist < combat_config.yield_radius && velocity.length_squared() > 0.0001 {
            let heading_toward = velocity.normalize().dot(to_other / dist) > 0.4;
            let i_yield = entity.index().index() > other_entity.index().index();
            if heading_toward && i_yield {
                yield_factor =
                    yield_factor.min((dist / combat_config.yield_radius).clamp(0.0, 1.0));
            }
        }
    }

    let base = velocity * yield_factor;
    let mut result = base + separation * combat_config.separation_strength;
    if yield_factor < 0.7 && velocity.length_squared() > 0.0001 {
        result -= velocity.normalize() * combat_config.yield_backoff_speed;
    }

    if result.length_squared() > 0.0001 {
        let probe = monster_pos + result.normalize() * combat_config.wander_lookahead;
        if !terrain::is_area_clear(map, probe, config.collider_half_extent) {
            result = base;
        }
    }

    result
}

/// A sideways offset from a straight line between `from` and `to`, scaled by
/// the distance between them and `fraction`, capped at `max_offset` — a
/// fixed world-space nudge would either be pointless from far away or absurd
/// up close, so packmates flanking a target near the player barely diverge,
/// while ones reacting to a shared position on the other side of a big
/// obstacle (a lake, a mountain, a wall cluster) genuinely take a different
/// route to it — but the cap keeps that divergence to "different approach
/// angle on the same target", not the pack scattering apart.
fn flank_offset(from: Vec2, to: Vec2, side: f32, fraction: f32, max_offset: f32) -> Vec2 {
    let to_target = to - from;
    if to_target.length_squared() < 0.0001 {
        return Vec2::new(side, 0.0) * (fraction * TILE_SIZE).min(max_offset);
    }
    // `Vec2::perp` rotates 90° without changing length, so this is already
    // `distance(from, to) * fraction` in the perpendicular direction.
    let offset = to_target.perp() * side * fraction;
    offset.clamp_length_max(max_offset)
}

/// If `candidate` lands within `claim_radius` of another monster's currently
/// staked-out investigate target, nudges it a fixed distance to one side
/// (deterministic per `entity`, same left/right convention as flanking)
/// instead of two monsters investigating the identical spot — "investigate
/// claims are atomic": once a point is claimed, only one monster commits to
/// it this pass. Never fully discards the candidate — it's relocated, not
/// rejected, so callers always get a usable target back.
fn deconflict_investigate_point(
    candidate: Vec2,
    entity: Entity,
    claims: &[(Entity, Vec2)],
    claim_radius: f32,
) -> Vec2 {
    for &(other, claimed_pos) in claims {
        if other == entity {
            continue;
        }
        let dist = candidate.distance(claimed_pos);
        if dist < claim_radius {
            let side = flank_side(entity);
            let away = if dist > 0.0001 {
                (candidate - claimed_pos) / dist
            } else {
                Vec2::new(side, 0.0)
            };
            return candidate + away.perp() * side * claim_radius;
        }
    }
    candidate
}

/// Picks a random wander direction that's actually clear of walls/water a
/// short distance ahead (loaded terrain, or the same noise prediction used
/// for spawning if it isn't loaded yet) — idle wandering has no pathfinding
/// of its own, so without this check it could walk straight into either.
/// Falls back to standing still if nothing clear turns up in a few tries.
fn pick_wander_direction(
    map: &TerrainMap,
    monster_pos: Vec2,
    half_extent: Vec2,
    lookahead: f32,
) -> Vec2 {
    const ATTEMPTS: u32 = 6;
    for _ in 0..ATTEMPTS {
        let dir = Vec2::new(rand_dir(), rand_dir()).normalize_or_zero();
        if dir.length_squared() < 0.0001 {
            continue;
        }
        if terrain::is_area_clear(map, monster_pos + dir * lookahead, half_extent) {
            return dir;
        }
    }
    Vec2::ZERO
}

fn rand_dir() -> f32 {
    // losowa wartość między -1 a 1
    (rand::random::<f32>() - 0.5) * 2.0
}

fn animate_monster_sprite(
    time: Res<Time>,
    atlas_handles: Res<AtlasHandles>,
    mut query: Query<
        (
            &mut AnimationIndices,
            &mut AnimationTimer,
            &mut Sprite,
            &mut AttackStatus,
            &mut FinishStatus,
        ),
        With<MonsterSprite>,
    >,
) {
    for (mut indices, mut timer, mut sprite, mut attack, mut finish) in &mut query {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                let mut last = false;
                atlas.index = if atlas.index == indices.last {
                    last = true;
                    indices.first
                } else {
                    atlas.index + 1
                };
                if last {
                    if attack.0 {
                        attack.0 = false;
                        finish.0 = true;
                        let animation_indices = atlas_handles.0.get("walk").unwrap().clone();
                        if let Some(atlas) = &mut sprite.texture_atlas {
                            atlas.index = animation_indices.first;
                        }
                        *indices = animation_indices;
                        timer.reset();
                    }
                }
            }
        }
    }
}
