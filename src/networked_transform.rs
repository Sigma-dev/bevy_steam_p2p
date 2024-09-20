use bevy::*;
use math::VectorSpace;
use prelude::*;
use steamworks::networking_types::SendFlags;

use crate::{ client::SteamP2PClient, NetworkData, NetworkIdentity };

#[derive(Component)]
pub struct NetworkedTransform {
    pub synced: bool,
    pub target: Vec3,
}

impl Default for NetworkedTransform {
    fn default() -> Self {
        Self { synced: true, target: Vec3::ZERO }
    }
}

#[derive(Event)]
pub (crate) struct PositionUpdate {
    pub network_identity: NetworkIdentity, 
    pub new_position: Vec3
}

pub struct NetworkedTransformPlugin;

impl Plugin for NetworkedTransformPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_systems(FixedUpdate, handle_networked_transform)
        .add_event::<PositionUpdate>();
    }
}

fn handle_networked_transform(
    client: Res<SteamP2PClient>,
    mut evs_update: EventReader<PositionUpdate>,
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
                networked_transform.target = update.new_position;
            }
        }
        if !networked_transform.synced { continue; };
        if client.id != network_identity.owner_id { 
            transform.translation = transform.translation.lerp(networked_transform.target, 10. * time.delta_seconds());
            continue; 
        };
        client.send_message_others(NetworkData::PositionUpdate(*network_identity, transform.translation), SendFlags::UNRELIABLE);
    }
}

fn on_add(
    trigger: Trigger<OnAdd, NetworkedTransform>,
    mut transform_query: Query<(&Transform, &mut NetworkedTransform)>
) {
    println!("On Add");
    let Ok((transform, mut networked_transform)) = transform_query.get_mut(trigger.entity()) else { return; };
    networked_transform.target = transform.translation;
}