use crate::resourses::physics_resources::*;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

pub struct WorldItemsPlugin;

const PICKUP_RANGE: f32 = 96.0;
const DROP_DISTANCE: f32 = 72.0;
const WORLD_ITEM_MAX_SIZE: f32 = 48.0;

/// Removed once the texture is available and its display scale is known.
#[derive(Component)]
struct WorldItemSpritePending;

impl Plugin for WorldItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldItemsState::default()).add_systems(
            Update,
            (
                reset_world_items_on_leave,
                pickup_nearest_item,
                drop_selected_item,
                scale_world_item_sprites,
            )
                .chain()
                .in_set(crate::systems::lifecycle::AppSet::Gameplay),
        );
    }
}

/// Icons have source-dependent resolutions (the apple is 1024×1024 while the
/// sword is 32×32). Scale each world sprite by the loaded image's longest
/// edge, so both occupy at most `WORLD_ITEM_MAX_SIZE` world units.
fn scale_world_item_sprites(
    images: Res<Assets<Image>>,
    mut pending_items: Query<(Entity, &Sprite, &mut Transform), With<WorldItemSpritePending>>,
    mut commands: Commands,
) {
    for (entity, sprite, mut transform) in &mut pending_items {
        let Some(image) = images.get(&sprite.image) else {
            continue;
        };
        let size = image.size();
        let longest_edge = size.x.max(size.y) as f32;
        if longest_edge > 0.0 {
            transform.scale = Vec3::splat(WORLD_ITEM_MAX_SIZE / longest_edge);
        }
        commands.entity(entity).remove::<WorldItemSpritePending>();
    }
}

fn reset_world_items_on_leave(
    game_status: Res<GameStatus>,
    mut state: ResMut<WorldItemsState>,
    items: Query<Entity, With<WorldItem>>,
    mut commands: Commands,
) {
    if game_status.0 || !state.spawned_for_session {
        return;
    }

    for entity in items.iter() {
        commands.entity(entity).despawn();
    }
    state.spawned_for_session = false;
}

fn pickup_nearest_item(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player: Query<(&Transform, &mut PlayerData), (With<Player>, Without<Pending>)>,
    items: Query<(Entity, &Transform, &WorldItem)>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Ok((player_transform, mut player_data)) = player.single_mut() else {
        return;
    };
    let player_position = player_transform.translation.xy();
    let nearest = items
        .iter()
        .filter_map(|(entity, transform, world_item)| {
            let distance = player_position.distance(transform.translation.xy());
            (distance <= PICKUP_RANGE).then_some((entity, distance, world_item.item.clone()))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1));

    if let Some((entity, _, item)) = nearest {
        if player_data.inventory.try_add_item(item) {
            commands.entity(entity).despawn();
        }
    }
}

fn drop_selected_item(
    keyboard: Res<ButtonInput<KeyCode>>,
    inventory_state: Res<InventoryState>,
    mut player: Query<
        (&Transform, &FacingDirection, &mut PlayerData),
        (With<Player>, Without<Pending>),
    >,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    if !keyboard.just_pressed(KeyCode::KeyQ) {
        return;
    }
    let Ok((transform, facing, mut player_data)) = player.single_mut() else {
        return;
    };
    // BLOKADA MIECZA: podstawowa broń nie może zostać przypadkowo wyrzucona.
    if player_data
        .inventory
        .get_item(inventory_state.selected as u32)
        .is_some_and(|item| item.id == "sword_basic")
    {
        return;
    }
    let drop_entire_stack =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let Some(item) = (if drop_entire_stack {
        player_data
            .inventory
            .remove_item(inventory_state.selected as u32)
    } else {
        player_data
            .inventory
            .remove_one(inventory_state.selected as u32)
    }) else {
        return;
    };
    let position = transform.translation.xy() + facing.0.normalize_or_zero() * DROP_DISTANCE;
    spawn_world_item(&mut commands, &asset_server, item, position);
}

pub fn spawn_world_item(
    commands: &mut Commands,
    asset_server: &AssetServer,
    item: Item,
    position: Vec2,
) {
    commands.spawn((
        WorldItem { item: item.clone() },
        Sprite::from_image(asset_server.load(&item.path)),
        Transform::from_xyz(position.x, position.y, 32.0),
        WorldItemSpritePending,
        RenderLayers::from_layers(CAMERA_LAYER_ENTITY),
    ));
}
