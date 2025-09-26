use bevy::prelude::*;
use crate::physics_resources::*;
use crate::monster::MonsterAI;
use bevy::window::{PrimaryWindow, Window};

pub struct EventerPlugin;

impl Plugin for EventerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<ConsumeEvent>()
            .add_event::<FunctionalEvent>()
            .add_systems(Update, (food_eventer, functional_eventer));
    }
}

fn food_eventer(
    mut events: EventReader<ConsumeEvent>,
    config: Res<ItemConfig>,
    mut query: Query<&mut PlayerData, With<Player>>,
    mut slots: Query<(&InventorySlot, &Children), With<InventorySlot>>,
    mut text_q: Query<&mut Text>,
) {
    for ev in events.read() {
        let Ok(mut pdata) = query.single_mut() else {
            return;
        };
        // usuń 1 sztukę itemu z inventory
        pdata.inventory.remove_one(ev.slot);
        for (slot, child) in slots {
            if slot.0 as u32 == ev.slot {
                if let Some(item) = pdata.inventory.get_item(ev.slot) {
                    if let Ok(mut text) = text_q.get_mut(child[0]) {
                        *text = Text::new(format!("{}", item.amount));
                    }
                }
                break;
            }
        }

        // efekt spożycia
        if let Some(item) = config.items.get(&ev.item_id) {
            pdata.health = (pdata.health + item.value).min(pdata.max_health);
            //println!("Gracz zjadł {}, +{} HP", ev.item_id, item.value);
        }
    }
}

fn functional_eventer(
    mut events: EventReader<FunctionalEvent>,
    config: Res<ItemConfig>,
    mut query: Query<(&mut AnimationIndices, &mut AnimationTimer, &mut Sprite, &mut GlobalTransform, &mut AttackStatus), With<PlayerSprite>>,
    asset_server: Res<AssetServer>,
    atlas_handles: Res<AtlasHandles>,
    mut query_m: Query<(&mut MonsterAI, &Transform), (With<Monster>, Without<Player>, Without<Pending>)>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    for ev in events.read() {
        if let Some(item) = config.items.get(&ev.item_id) {
            for (mut indices, mut timer, mut sprite, mut transform, mut atack) in &mut query {
                if !atack.0 {
                    atack.0 = true;
                    let animation_indices = atlas_handles.0.get("attack").unwrap().clone();
                    if let Some(atlas) = &mut sprite.texture_atlas {
                        atlas.index = animation_indices.first;
                    }
                    *indices = animation_indices;
                    timer.reset();
                    let tile_size = 64.0;
                    let action_distance = 1.675 * tile_size;
                    for (mut ai, rb_transform) in &mut query_m {
                        let monster_pos = rb_transform.translation.xy();
                        let player_pos = transform.translation().xy();
                        let distance = monster_pos.distance(player_pos);
                        if distance < action_distance {
                            let window = match windows.get_single() {
                                Ok(w) => w,
                                Err(_) => return, // brak okna głównego
                            };

                            if let Some(cursor_pos) = window.cursor_position() {
                                let to_monster = (monster_pos - player_pos).normalize_or_zero();
                                let screen_center = Vec2::new(window.width() / 2.0, window.height() / 2.0);
                                let fixed_cursor_pos = Vec2::new(cursor_pos.x, window.height() - cursor_pos.y);
                                let cursor_dir = (fixed_cursor_pos - screen_center).normalize_or_zero();
                                let dot = cursor_dir.dot(to_monster).clamp(-1.0, 1.0);
                                let angle = dot.acos().to_degrees();
                                if angle < 75.0 {
                                    ai.health -= item.value;
                                } else if distance < 1.25 * tile_size {
                                    ai.health -= item.value;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}