use bevy::prelude::*;
use crate::physics_resources::*;

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
            println!("Gracz zjadł {}, +{} HP", ev.item_id, item.value);
        }
    }
}

fn functional_eventer(
    mut events: EventReader<FunctionalEvent>,
    config: Res<ItemConfig>,
) {
    for ev in events.read() {
        if let Some(item) = config.items.get(&ev.item_id) {
            println!("Gracz użył {}, dmg: {}", ev.item_id, item.value);
        }
    }
}