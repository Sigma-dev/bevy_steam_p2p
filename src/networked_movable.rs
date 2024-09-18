use bevy::*;
use prelude::*;

use crate::{ client::SteamP2PClient, NetworkIdentity };

#[derive(Component)]
pub struct NetworkedMovable {
    pub speed: f32
}

pub struct NetworkedMovablePlugin;

impl Plugin for NetworkedMovablePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_networked_movable);
    }
}

fn handle_networked_movable(
    mut transform_query: Query<(&mut Transform, Option<&NetworkIdentity>, &NetworkedMovable)>,
    keys: Res<ButtonInput<KeyCode>>,
    client: Option<Res<SteamP2PClient>>,
    time: Res<Time>,
) {
    for (mut movable_transform, network_identity, movable) in transform_query.iter_mut() {
        let mut vec = Vec3::ZERO;
        if let Some(identity) = network_identity {
            if let Some(ref cli) = client {
                if identity.owner_id != cli.id {
                    continue;
                }
            }
        }
        if keys.pressed(KeyCode::KeyW) {
            vec.z += 1.0
        }
        if keys.pressed(KeyCode::KeyS) {
            vec.z -= 1.0
        }
        if keys.pressed(KeyCode::KeyD) {
            vec.x -= 1.0
        }
        if keys.pressed(KeyCode::KeyA) {
            vec.x += 1.0
        }
        if keys.pressed(KeyCode::KeyQ) {
            vec.y += 1.0
        }
        if keys.pressed(KeyCode::KeyE) {
            vec.y -= 1.0
        }
        movable_transform.translation += vec * time.delta_seconds() * movable.speed;
    }
}