use std::{path::Path, time::Duration};

use bevy::*;
use prelude::*;
use bevy_steamworks::*;
use flume::{Receiver, Sender};
use networked_movable::{ NetworkedMovable, NetworkedMovablePlugin};
use ::serde::{Deserialize, Serialize};
use steamworks::{networking_types::{ NetConnectionEnd, NetworkingIdentity }, LobbyChatUpdate};
mod networked_movable;
pub mod client;
pub use client::*;
pub use steamworks::networking_types::SendFlags;

pub struct SteamP2PPlugin;

impl Plugin for SteamP2PPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins(SteamworksPlugin::init_app(480).unwrap())
        .add_plugins(NetworkedMovablePlugin)
        .add_systems(PreStartup, steam_start)
        .add_systems(Update, (handle_channels, steam_events, receive_messages, handle_network_data, handle_instantiate))
        .add_systems(FixedUpdate, (handle_networked_transform))
        .add_event::<PositionUpdate>()
        .add_event::<LobbyJoined>()
        .add_event::<NetworkPacket>();
    }
}

#[derive(Event)]
pub struct LobbyJoined {
    lobby_id: LobbyId
}

#[derive(Event, Clone)]
pub struct NetworkPacket {
    data: NetworkData,
    sender: SteamId
}



#[derive(Component, Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub struct NetworkIdentity {
    pub id: u32,
    pub owner_id: SteamId
}

#[derive(PartialEq)]
enum NetworkSync {
    Disabled,
    Enabled(f32),
}

#[derive(Component)]
pub struct NetworkedTransform {
    pub synced: bool,
    pub target: Vec3,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FilePath(pub u32);

#[derive(Event)]
struct PositionUpdate {
    network_identity: NetworkIdentity, 
    new_position: Vec3
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkData {
    Handshake,
    SendObjectData(NetworkIdentity, i8, Vec<u8>), //NetworkId of receiver, id of action, data of action
    Instantiate(NetworkIdentity, FilePath, Vec3), //NetworkId of created object, filepath of prefab, starting position
    PositionUpdate(NetworkIdentity, Vec3), //NetworkId of receiver, new position
    Destroy(NetworkIdentity), //NetworkId of object to be destroyed
    NetworkMessage(String), //Message for arbitrary communication, to be avoided outside of development
    DebugMessage(String), //Make the receiving client print the message
}

fn lobby_joined(client: &mut ResMut<SteamP2PClient>, info: &LobbyChatUpdate) {
    println!("Somebody joined your lobby: {:?}", info.user_changed);
}

fn handle_instantiate(
    mut evs_network: EventReader<NetworkPacket>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in evs_network.read() {
        let NetworkData::Instantiate(ref network_identity, ref path, ref pos) = ev.data else { continue; };
        println!("Instantiation");

        if (*path == FilePath(0)) {
            commands.spawn((
                PbrBundle {
                mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                material: materials.add(Color::srgb_u8(124, 144, 255)),
                transform: Transform::from_translation(*pos),
                ..default()
                },
                network_identity.clone(),
                NetworkedTransform{synced: true, target: *pos},
                NetworkedMovable { speed: 10. }
            ));
        }
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

fn handle_network_data(
    mut evs_network: EventReader<NetworkPacket>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ev_pos_update: EventWriter<PositionUpdate>,
) {
    for ev in evs_network.read() { 
        match ev.data.clone() {
            NetworkData::SendObjectData(id, action_id, action_data) => println!("Action"),
            NetworkData::Instantiate(id, prefab_path, pos) => {},// instantiate(id, prefab_path, pos, &mut commands, &mut meshes, &mut materials),
            NetworkData::PositionUpdate(id, pos) => {ev_pos_update.send(PositionUpdate { network_identity: id, new_position: pos }); },
            NetworkData::Destroy(id) => println!("Destroyed"),
            NetworkData::Handshake => {
                println!("Received handshake");
            },
            NetworkData::DebugMessage(message) => println!("Debug: {message}"),
            _ => {}
        }
    }
}

fn receive_messages(
    mut client: ResMut<SteamP2PClient>, 
    mut evs_network: EventWriter<NetworkPacket>
) {
    let messages: Vec<steamworks::networking_types::NetworkingMessage<steamworks::ClientManager>> = client.steam_client.networking_messages().receive_messages_on_channel(0, 2048);

    for message in messages {
        let sender = message.identity_peer().steam_id().unwrap();
        let serialized_data = message.data();
        let data_try: Result<NetworkData, _> = rmp_serde::from_slice(serialized_data);

        if let Ok(data) = data_try { 
            evs_network.send(NetworkPacket { sender, data });
        }
        drop(message); //not sure about usefullness, mentionned in steam docs as release
    }
}

fn handle_channels(
    mut client: ResMut<SteamP2PClient>,
    mut event_writer: EventWriter<LobbyJoined>,
    mut evs_network: EventWriter<NetworkPacket>
) { 
    if let Ok(lobby_id) = client.lobby_channel.rx.try_recv() {
        client.lobby_status = LobbyStatus::InLobby(lobby_id);
        event_writer.send(LobbyJoined { lobby_id });
        println!("Joined Lobby: {}", lobby_id.raw());
    }

    if let Ok(packet) = client.packet_channel.rx.try_recv() {
        evs_network.send(packet);
    }
}

fn steam_start(
    steam_client: Res<Client>,
    mut commands: Commands,
) {
    let steam_id = steam_client.user().steam_id();
    println!("Connected: {}", steam_id.raw());
    steam_client.networking_utils().init_relay_network_access();
    steam_client.networking_messages().session_request_callback(
        move |session_request| {
            if session_request.remote().steam_id() == Some(steam_id) {
                session_request.accept();
                return;
            }
            match session_request.accept() {
                true => println!("Succesfully accepted"),
                false => println!("Failed to accept"),
            }
        }
    );
    steam_client.networking_messages().session_failed_callback(
        move |res| {
            if let Some(id) = res.identity_remote() {
                if id.steam_id() == Some(steam_id) {
                    return;
                }
            }
            println!("Session Failed: {:?}", res.end_reason().unwrap_or(NetConnectionEnd::Other(-42)));
        }
    );
    commands.insert_resource(SteamP2PClient::new(steam_client.clone()));  
}

fn steam_events(
    mut evs: EventReader<SteamworksEvent>,
    mut client: ResMut<SteamP2PClient>,
    network_query: Query<(Entity, &NetworkIdentity)>,
    mut commands: Commands,
) {
    for ev in evs.read() {
        match ev {
            SteamworksEvent::GameLobbyJoinRequested(info) => {
                println!("Trying to join: {}", info.lobby_steam_id.raw());
                client.join_lobby(info.lobby_steam_id)
            },
            SteamworksEvent::LobbyChatUpdate(info) => {
                match info.member_state_change {
                    bevy_steamworks::ChatMemberStateChange::Entered => lobby_joined(&mut client, info),
                    bevy_steamworks::ChatMemberStateChange::Left | bevy_steamworks::ChatMemberStateChange::Disconnected => {
                        println!("Other left lobby");
                        client.lobby_status = LobbyStatus::OutOfLobby;
                        for (entity, networked) in network_query.iter() {
                            if (networked.owner_id == info.making_change) {
                                commands.entity(entity).despawn();
                            }
                        }
                    }
                    _ => println!("other")
                }
            },
            SteamworksEvent::SteamServersConnected(_) => println!("Connected to steam servers!"),
            SteamworksEvent::AuthSessionTicketResponse(_) => println!("Ticket response"),
            SteamworksEvent::DownloadItemResult(_) => println!("Download item result"),
            SteamworksEvent::P2PSessionConnectFail(_) => println!("P2P Fail"),
            SteamworksEvent::P2PSessionRequest(_) => println!("P2P Session request"),
            SteamworksEvent::PersonaStateChange(persona) => println!("Persona {}: {}", persona.steam_id.raw(), persona.flags.bits()),
            SteamworksEvent::SteamServerConnectFailure(_) => println!("Connection failed"),
            SteamworksEvent::SteamServersDisconnected(_) => println!("Disconnected"),
            SteamworksEvent::TicketForWebApiResponse(_) => println!("Ticket"),
            SteamworksEvent::UserAchievementStored(_) => println!("Achievement stored"),
            SteamworksEvent::UserStatsReceived(_) => println!("UserStatsReceived"),
            SteamworksEvent::UserStatsStored(_) => println!("User stats stored"),
            SteamworksEvent::ValidateAuthTicketResponse(_) => println!("Validate auth ticket"),
            SteamworksEvent::NetworkingMessagesSessionRequest(_) => println!("Message session request"),
            SteamworksEvent::RelayNetworkStatusCallback(_) => println!("Relay network status"),
        }
    }
}