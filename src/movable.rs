use bevy::*;
use prelude::*;

use crate::{NetworkClient, NetworkIdentity};

#[derive(Component)]
pub struct Movable {
    pub speed: f32
}

pub struct MovablePlugin;

impl Plugin for MovablePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, movable);
    }
}

fn movable(
    mut transform_query: Query<(&mut Transform, Option<&NetworkIdentity>, &Movable)>,
    keys: Res<ButtonInput<KeyCode>>,
    client: Option<Res<NetworkClient>>,
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