use crate::resourses::physics_resources::*;
use crate::systems::physics::remove_rigid_body;
use crate::systems::player;
use crate::systems::save::{self, ActiveSlot, PendingRespawn};
use bevy::prelude::*;

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
            .insert_resource(PendingRespawn::default())
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
    mut game_status: ResMut<GameStatus>,
    mut active_slot: ResMut<ActiveSlot>,
) {
    // Drain every event this tick, but only ever act once: a player already
    // existing (or more than one request queued before the first is
    // processed) must never result in more than one spawned player.
    let mut requested_slot: Option<u8> = None;
    for event in events.read() {
        if requested_slot.is_none() {
            requested_slot = Some(event.0);
        }
    }
    let Some(slot) = requested_slot else {
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
        commands.entity(player_id).insert((
            Transform::from_xyz(save.position.0, save.position.1, -32.0),
            save::player_data_from_save(&save),
        ));
        save.points
    } else {
        // Fresh slot: write its initial save now, so it's no longer "empty"
        // the next time the slot-select screen is shown.
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
            },
        );
        0
    };

    crate::systems::player_game_ui::spawn_health_bar(&mut commands, &asset_server, points);
    crate::systems::player_game_ui::spawn_inventory_bar(&mut commands, &asset_server);

    active_slot.0 = Some(slot);
    game_status.0 = true;
}

fn handle_player_died(
    mut events: MessageReader<PlayerDied>,
    mut commands: Commands,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
    mut island_manager: ResMut<ResIslandManager>,
    player_query: Query<(Option<&RigidBodyHandleComponent>, &PlayerData), With<Player>>,
    player_ui_query: Query<Entity, With<PlayerUIs>>,
    points_query: Query<&PointText>,
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
    pending_respawn.points = points_query.iter().next().map(|p| p.0).unwrap_or(0);

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

    crate::systems::player_game_ui::spawn_health_bar(
        &mut commands,
        &asset_server,
        pending_respawn.points,
    );
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
    points_query: Query<&PointText>,
    asset_server: Res<AssetServer>,
    mut game_status: ResMut<GameStatus>,
    mut resume_status: ResMut<ResumeStatus>,
    mut active_slot: ResMut<ActiveSlot>,
    mut pending_respawn: ResMut<PendingRespawn>,
    menu_assets: Res<crate::systems::menu_ui::MenuAssets>,
) {
    let mut requested = false;
    for _ in events.read() {
        requested = true;
    }
    if !requested {
        return;
    }

    if let Some(slot) = active_slot.0 {
        if let Ok((entity, transform, handle, player_data)) = player_query.single() {
            // Leaving while alive (from Pause): persist the exact current state.
            let points = points_query.iter().next().map(|p| p.0).unwrap_or(0);
            save::write_save(
                slot,
                &save::capture_save_data(transform, player_data, points),
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
                    points: pending_respawn.points,
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

    active_slot.0 = None;
    game_status.0 = false;
    resume_status.0 = false;
    crate::systems::menu_ui::setup_main_menu(
        &mut commands,
        &asset_server,
        menu_assets.background.clone(),
    );
    commands.spawn((Camera2d, MenuCamera));
}
