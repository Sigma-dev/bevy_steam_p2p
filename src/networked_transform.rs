use bevy::*;
use prelude::*;
use steamworks::networking_types::SendFlags;

use crate::{ client::SteamP2PClient, NetworkData, NetworkIdentity };

#[derive(Component)]
pub struct NetworkedTransform {
    pub target_position: Vec3,
    pub target_rotation: Quat,
    pub target_scale: Vec3,
    pub sync_position: bool,
    pub sync_rotation: bool,
    pub sync_scale: bool
}

impl Default for NetworkedTransform {
    fn default() -> Self {
        Self { target_position: Vec3::ZERO, target_rotation: Quat::default(), target_scale: Vec3::ZERO, sync_position: true, sync_rotation: true, sync_scale: true }
    }
}

impl NetworkedTransform {
    pub fn new(sync_position: bool, sync_rotation: bool, sync_scale: bool) -> Self {
        NetworkedTransform {
            sync_position,
            sync_rotation,
            sync_scale,
            ..default()
        }
    }
}

#[derive(Event, Debug)]
pub (crate) struct TransformUpdate {
    pub network_identity: NetworkIdentity, 
    pub position: Option<Vec3>,
    pub rotation: Option<Quat>,
    pub scale: Option<Vec3>,
}

pub struct NetworkedTransformPlugin;

impl Plugin for NetworkedTransformPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_systems(FixedUpdate, handle_networked_transform)
        .add_observer(on_add)
        .add_event::<TransformUpdate>();
    }
}

fn handle_networked_transform(
    client: Res<SteamP2PClient>,
    mut evs_update: EventReader<TransformUpdate>,
    mut networked_transform_query: Query<(&mut Transform, &NetworkIdentity, &mut NetworkedTransform)>,
    time: Res<Time>
) {
    let mut updates = Vec::new();
    
    for ev in evs_update.read() {
        updates.push(ev);
    }

    for (mut transform, network_identity, mut networked_transform) in networked_transform_query.iter_mut() {
        for update in &updates {
            if update.network_identity == *network_identity {
                if let Some(position) = update.position {
                    networked_transform.target_position = position;
                }
                if let Some(rotation) = update.rotation {
                    networked_transform.target_rotation = rotation;
                }
                if let Some(scale) = update.scale {
                    networked_transform.target_scale = scale;
                }
            }
        }
        if client.id != network_identity.owner_id {
            if networked_transform.sync_position {
                transform.translation = transform.translation.lerp(networked_transform.target_position, 10. * time.delta_secs());
            }
            if networked_transform.sync_rotation {
                transform.rotation = transform.rotation.lerp(networked_transform.target_rotation, 10. * time.delta_secs());
            }
            if networked_transform.sync_scale {
                transform.scale = transform.scale.lerp(networked_transform.target_scale, 10. * time.delta_secs());
            }
        } else {
            let data = NetworkData::TransformUpdate(
                network_identity.clone(),
                networked_transform.sync_position.then_some(transform.translation),
                networked_transform.sync_rotation.then_some(transform.rotation),
                networked_transform.sync_scale.then_some(transform.scale),
            );
            client.send_message_others(data, SendFlags::UNRELIABLE).expect("Couldn't send networked transform data");
        }
    }
}

fn on_add(
    trigger: Trigger<OnAdd, NetworkedTransform>,
    mut transform_query: Query<(&Transform, &mut NetworkedTransform)>
) {
    let Ok((transform, mut networked_transform)) = transform_query.get_mut(trigger.entity()) else { return; };
    networked_transform.target_position = transform.translation;
    networked_transform.target_rotation = transform.rotation;
    networked_transform.target_scale = transform.scale;
}