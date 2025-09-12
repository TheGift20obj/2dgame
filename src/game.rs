use bevy::prelude::*;
use crate::physics_resources::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, layer_checker);
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