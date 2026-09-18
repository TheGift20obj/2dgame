use crate::resourses::physics_resources::*;
use crate::systems::monster::{MonsterCombatConfig, load_monster_texture, spawn_monster_at};
use crate::systems::monster_ai::difficulty::{ActiveDifficulty, sense_config};
use crate::systems::monster_ai::hivemind::PlayerEscapeModel;
use crate::systems::physics::remove_rigid_body;
use crate::systems::player;
use crate::systems::save::{self, ActiveSlot, MonsterSaveData, PendingRespawn};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// Bundles a handful of small, unrelated session-flag resources into one
/// system parameter. `handle_leave_requested` otherwise needs one parameter
/// per resource/query, which pushes it past Bevy's 16-parameter limit on a
/// plain system function.
#[derive(SystemParam)]
struct LeaveFlags<'w> {
    game_status: ResMut<'w, GameStatus>,
    resume_status: ResMut<'w, ResumeStatus>,
    active_slot: ResMut<'w, ActiveSlot>,
    active_difficulty: Res<'w, ActiveDifficulty>,
    score: Res<'w, Score>,
    escape_model: Res<'w, PlayerEscapeModel>,
}

/// Bundles the handful of small session-flag resources `handle_play_requested`
/// sets on every Play (not the asset-loading params `player::init`/monster
/// spawning also need — those stay as individual params since they're not
/// unique to this function). Same purpose as `LeaveFlags`: keeps the plain
/// system function under Bevy's 16-parameter cap.
#[derive(SystemParam)]
struct PlaySessionFlags<'w> {
    game_status: ResMut<'w, GameStatus>,
    active_slot: ResMut<'w, ActiveSlot>,
    active_difficulty: ResMut<'w, ActiveDifficulty>,
    score: ResMut<'w, Score>,
    escape_model: ResMut<'w, PlayerEscapeModel>,
}

/// Deterministic ordering for the three concerns that used to race each
/// other across frames: UI turns clicks into intent events, the lifecycle
/// reacts to that intent (and to player events), and gameplay systems react
/// to the resulting state. Chaining them means a click's consequences (new
/// GameStatus, new/despawned entities queued) are always settled before
/// gameplay-gated systems evaluate that same tick, instead of the relative
/// order being left to the scheduler.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppSet {
    UiIntent,
    Lifecycle,
    Gameplay,
}

/// Translates UI intent (PlayRequested, LeaveRequested, RespawnRequested)
/// and player events (PlayerDied) into game lifecycle changes: spawning/
/// despawning the player, loading/writing saves, and flipping GameStatus/
/// ResumeStatus. menu_ui and player only talk to this plugin, through
/// events — never directly to each other.
pub struct GameLifecyclePlugin;

impl Plugin for GameLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ActiveSlot::default())
            .insert_resource(ActiveDifficulty::default())
            .insert_resource(PendingRespawn::default())
            .insert_resource(Score::default())
            .add_message::<PlayRequested>()
            .add_message::<PlayerDied>()
            .add_message::<LeaveRequested>()
            .add_message::<RespawnRequested>()
            .configure_sets(
                Update,
                (AppSet::UiIntent, AppSet::Lifecycle, AppSet::Gameplay).chain(),
            )
            .add_systems(
                Update,
                (
                    handle_play_requested,
                    handle_player_died,
                    handle_leave_requested,
                    handle_respawn_requested,
                )
                    .in_set(AppSet::Lifecycle),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_play_requested(
    mut events: MessageReader<PlayRequested>,
    existing_player: Query<Entity, With<Player>>,
    menu_root_query: Query<Entity, With<MenuRoot>>,
    camera_query: Query<Entity, With<MenuCamera>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    images: Res<Assets<Image>>,
    config: Res<ItemConfig>,
    atlas_handles: Res<AtlasHandles>,
    combat_config: Res<MonsterCombatConfig>,
    mut flags: PlaySessionFlags,
) {
    // Drain every event this tick, but only ever act once: a player already
    // existing (or more than one request queued before the first is
    // processed) must never result in more than one spawned player.
    let mut requested: Option<PlayRequested> = None;
    for event in events.read() {
        if requested.is_none() {
            requested = Some(PlayRequested {
                slot: event.slot,
                difficulty: event.difficulty,
            });
        }
    }
    let Some(PlayRequested {
        slot,
        difficulty: requested_difficulty,
    }) = requested
    else {
        return;
    };
    if !existing_player.is_empty() {
        return;
    }

    // Despawn the slot-select screen and its camera in the same batch as
    // spawning the player, so both take effect on the same flush — no frame
    // with neither the old UI/camera nor the new one.
    for root in menu_root_query.iter() {
        commands.entity(root).despawn();
    }
    for cam in camera_query.iter() {
        commands.entity(cam).despawn();
    }

    let player_id = player::init(
        &mut commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &mut texture_atlas_layouts,
        &images,
        &config,
        &atlas_handles,
    );

    let points = if let Some(save) = save::read_save(slot) {
        // An existing save always keeps its own difficulty, regardless of
        // what the message carried.
        flags.active_difficulty.0 = save.difficulty;
        commands.entity(player_id).insert((
            Transform::from_xyz(save.position.0, save.position.1, -32.0),
            save::player_data_from_save(&save),
        ));

        // Restore monsters (position + HP) and the pack's learned escape
        // direction exactly as they were when the player left — otherwise
        // leaving and rejoining would be a free reset of the current threat.
        flags.escape_model.restore(
            Vec2::new(save.pack_escape_dir.0, save.pack_escape_dir.1),
            save.pack_escape_samples,
        );
        if !save.monsters.is_empty() {
            let (texture, texture_atlas_layout) =
                load_monster_texture(&asset_server, &mut texture_atlas_layouts);
            let reaction_time = sense_config(save.difficulty).reaction_time;
            for monster in &save.monsters {
                spawn_monster_at(
                    &mut commands,
                    &mut meshes,
                    texture.clone(),
                    texture_atlas_layout.clone(),
                    &atlas_handles,
                    &combat_config,
                    reaction_time,
                    Vec2::new(monster.position.0, monster.position.1),
                    monster.health,
                );
            }
        }

        save.points
    } else {
        // Fresh slot: write its initial save now, so it's no longer "empty"
        // the next time the slot-select screen is shown.
        let difficulty = requested_difficulty.unwrap_or_default();
        flags.active_difficulty.0 = difficulty;
        let mut inventory = Inventory::new();
        inventory.init(&config);
        save::write_save(
            slot,
            &save::SaveData {
                health: 100.0,
                max_health: 100.0,
                satamina: 360.0,
                min_satamina: 25.0,
                max_satamina: 360.0,
                position: (0.0, 0.0),
                inventory: inventory.items,
                points: 0,
                difficulty,
                monsters: Vec::new(),
                pack_escape_dir: (0.0, 0.0),
                pack_escape_samples: 0,
            },
        );
        0
    };

    flags.score.0 = points;
    crate::systems::player_game_ui::spawn_health_bar(&mut commands, &asset_server, points);
    crate::systems::player_game_ui::spawn_inventory_bar(&mut commands, &asset_server);

    flags.active_slot.0 = Some(slot);
    flags.game_status.0 = true;
}

fn handle_player_died(
    mut events: MessageReader<PlayerDied>,
    mut commands: Commands,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
    mut island_manager: ResMut<ResIslandManager>,
    player_query: Query<(Option<&RigidBodyHandleComponent>, &PlayerData), With<Player>>,
    player_ui_query: Query<Entity, With<PlayerUIs>>,
    asset_server: Res<AssetServer>,
    mut resume_status: ResMut<ResumeStatus>,
    mut pending_respawn: ResMut<PendingRespawn>,
) {
    // Drain every event this tick, but only ever act once: the player entity
    // may be reported dead more than once before its despawn command is
    // actually applied, and despawning it twice would warn about an
    // already-despawned entity.
    let mut died: Option<Entity> = None;
    for event in events.read() {
        if died.is_none() {
            died = Some(event.0);
        }
    }
    let Some(entity) = died else { return };

    // Capture what needs to survive death (inventory, points) before the
    // entity and its components are gone.
    if let Ok((handle, player_data)) = player_query.get(entity) {
        pending_respawn.inventory = Some(player_data.inventory.items.clone());
        if let Some(handle) = handle {
            remove_rigid_body(
                &mut rigid_bodies,
                &mut colliders,
                &mut island_manager,
                handle.0,
            );
        }
    }

    commands.entity(entity).despawn();
    for ui_entity in player_ui_query {
        commands.entity(ui_entity).despawn();
    }

    // Freeze gameplay (reusing the existing pause gate) while the death
    // screen is up, instead of fully ending the session.
    resume_status.0 = true;
    crate::systems::menu_ui::setup_death_screen(&mut commands, &asset_server);
    // The player's own cameras just went with it — without a camera here,
    // nothing (not even this UI) would render.
    commands.spawn((Camera2d, MenuCamera));
}

fn handle_respawn_requested(
    mut events: MessageReader<RespawnRequested>,
    existing_player: Query<Entity, With<Player>>,
    menu_root_query: Query<Entity, With<MenuRoot>>,
    camera_query: Query<Entity, With<MenuCamera>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    images: Res<Assets<Image>>,
    config: Res<ItemConfig>,
    atlas_handles: Res<AtlasHandles>,
    mut resume_status: ResMut<ResumeStatus>,
    mut pending_respawn: ResMut<PendingRespawn>,
    score: Res<Score>,
) {
    let mut requested = false;
    for _ in events.read() {
        requested = true;
    }
    if !requested || !existing_player.is_empty() {
        return;
    }

    // Despawn the death screen and its camera in the same batch as spawning
    // the new player, so both take effect on the same flush.
    for root in menu_root_query.iter() {
        commands.entity(root).despawn();
    }
    for cam in camera_query.iter() {
        commands.entity(cam).despawn();
    }

    let player_id = player::init(
        &mut commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &mut texture_atlas_layouts,
        &images,
        &config,
        &atlas_handles,
    );

    if let Some(items) = pending_respawn.inventory.take() {
        commands
            .entity(player_id)
            .insert(PlayerData::respawn_with_inventory(Inventory {
                items,
                capacity: 16,
            }));
    }

    crate::systems::player_game_ui::spawn_health_bar(&mut commands, &asset_server, score.0);
    crate::systems::player_game_ui::spawn_inventory_bar(&mut commands, &asset_server);

    resume_status.0 = false;
}

fn handle_leave_requested(
    mut events: MessageReader<LeaveRequested>,
    menu_root_query: Query<Entity, With<MenuRoot>>,
    camera_query: Query<Entity, With<MenuCamera>>,
    mut commands: Commands,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
    mut island_manager: ResMut<ResIslandManager>,
    player_query: Query<
        (
            Entity,
            &Transform,
            Option<&RigidBodyHandleComponent>,
            &PlayerData,
        ),
        With<Player>,
    >,
    player_ui_query: Query<Entity, With<PlayerUIs>>,
    monster_query: Query<(&Transform, &MonsterAI), With<Monster>>,
    asset_server: Res<AssetServer>,
    mut pending_respawn: ResMut<PendingRespawn>,
    menu_assets: Res<crate::systems::menu_ui::MenuAssets>,
    mut flags: LeaveFlags,
) {
    let mut requested = false;
    for _ in events.read() {
        requested = true;
    }
    if !requested {
        return;
    }

    // Captured once, used by both branches below — monsters (and the pack's
    // learned escape direction) persist through the death screen too, so
    // leaving from there must save them just as much as leaving from Pause.
    // Without this, leaving and rejoining would be a free reset of whatever
    // monsters were currently alive/hurt/chasing.
    let monsters: Vec<MonsterSaveData> = monster_query
        .iter()
        .map(|(transform, ai)| MonsterSaveData {
            position: (transform.translation.x, transform.translation.y),
            health: ai.health,
        })
        .collect();
    let pack_escape_dir = (
        flags.escape_model.avg_flee_dir.x,
        flags.escape_model.avg_flee_dir.y,
    );
    let pack_escape_samples = flags.escape_model.samples();

    if let Some(slot) = flags.active_slot.0 {
        if let Ok((entity, transform, handle, player_data)) = player_query.single() {
            // Leaving while alive (from Pause): persist the exact current state.
            save::write_save(
                slot,
                &save::capture_save_data(
                    transform,
                    player_data,
                    flags.score.0,
                    flags.active_difficulty.0,
                    monsters,
                    pack_escape_dir,
                    pack_escape_samples,
                ),
            );
            if let Some(handle) = handle {
                remove_rigid_body(
                    &mut rigid_bodies,
                    &mut colliders,
                    &mut island_manager,
                    handle.0,
                );
            }
            commands.entity(entity).despawn();
        } else if let Some(items) = pending_respawn.inventory.take() {
            // Leaving from the death screen: the player is already despawned;
            // persist a fresh state that keeps only the preserved inventory.
            save::write_save(
                slot,
                &save::SaveData {
                    health: 100.0,
                    max_health: 100.0,
                    satamina: 360.0,
                    min_satamina: 25.0,
                    max_satamina: 360.0,
                    position: (0.0, 0.0),
                    inventory: items,
                    points: flags.score.0,
                    difficulty: flags.active_difficulty.0,
                    monsters,
                    pack_escape_dir,
                    pack_escape_samples,
                },
            );
        }
    }

    for ui_entity in player_ui_query {
        commands.entity(ui_entity).despawn();
    }
    // Despawn whatever screen (Pause or the death screen) and camera sent us
    // here in the same batch as spawning the main menu's own, so both take
    // effect on the same flush.
    for root in menu_root_query.iter() {
        commands.entity(root).despawn();
    }
    for cam in camera_query.iter() {
        commands.entity(cam).despawn();
    }

    flags.active_slot.0 = None;
    flags.game_status.0 = false;
    flags.resume_status.0 = false;
    crate::systems::menu_ui::setup_main_menu(
        &mut commands,
        &asset_server,
        menu_assets.background.clone(),
    );
    commands.spawn((Camera2d, MenuCamera));
}
