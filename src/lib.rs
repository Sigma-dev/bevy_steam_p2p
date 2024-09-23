use std::{path::Path, time::Duration};

use bevy::*;
use math::VectorSpace;
use networked_transform::{NetworkedTransform, NetworkedTransformPlugin, PositionUpdate};
use prelude::*;
use bevy_steamworks::*;
use flume::{Receiver, Sender};
use networked_movable::{ NetworkedMovable, NetworkedMovablePlugin};
use ::serde::{Deserialize, Serialize};
use steamworks::{networking_types::{ NetConnectionEnd, NetworkingIdentity }, LobbyChatUpdate};
mod networked_movable;
pub mod client;
pub mod networked_transform;
pub use client::*;
pub use steamworks::networking_types::SendFlags;

pub struct SteamP2PPlugin;

impl Plugin for SteamP2PPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins(SteamworksPlugin::init_app(480).unwrap())
        .add_plugins((NetworkedMovablePlugin, NetworkedTransformPlugin))
        .add_systems(PreStartup, steam_start)
        .add_systems(Update, (handle_channels, steam_events, receive_messages, handle_network_data, handle_instantiate, handle_joiner))
        .add_event::<LobbyJoined>()
        .add_event::<NetworkPacket>()
        .add_event::<UnhandledInstantiation>();
    }
}

#[derive(Event)]
pub struct LobbyJoined {
    pub lobby_id: LobbyId
}

#[derive(Event)]
pub struct UnhandledInstantiation {
    pub network_identity: NetworkIdentity,
    pub position: Vec3
}

#[derive(Event, Clone)]
pub struct NetworkPacket {
    pub data: NetworkData,
    pub sender: SteamId
}

#[derive(Component, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkIdentity {
    pub id: u32,
    pub owner_id: SteamId,
    pub instantiation_path: FilePath
}

#[derive(PartialEq)]
enum NetworkSync {
    Disabled,
    Enabled(f32),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FilePath(pub String);

impl FilePath {
    pub fn new(path: &str) -> FilePath {
        FilePath(path.to_string())
    }
}

impl std::cmp::PartialEq<&str> for FilePath {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkData {
    Handshake,
    SendObjectData(NetworkIdentity, i8, Vec<u8>), //NetworkId of receiver, id of action, data of action
    Instantiate(NetworkIdentity, Vec3), //NetworkId of created object, filepath of prefab, starting position
    PositionUpdate(NetworkIdentity, Vec3), //NetworkId of receiver, new position
    Destroy(NetworkIdentity), //NetworkId of object to be destroyed
    NetworkMessage(String), //Message for arbitrary communication, to be avoided outside of development
    DebugMessage(String), //Make the receiving client print the message
}

fn handle_joiner(
    client: ResMut<SteamP2PClient>,
    mut evs: EventReader<SteamworksEvent>,
    networked_query: Query<(&NetworkIdentity, Option<&Transform>)>
) {
    for ev in evs.read() {
        let SteamworksEvent::LobbyChatUpdate(update) = ev else {return;};
        if update.member_state_change == bevy_steamworks::ChatMemberStateChange::Entered {
            println!("Somebody joined your lobby: {:?}", update.user_changed);
            if client.is_lobby_owner().unwrap() {
                for (networked, transform) in networked_query.iter() {
                    client.send_message(&NetworkData::Instantiate(networked.clone(), transform.map(|t| t.translation).unwrap_or(Vec3::ZERO)), update.user_changed, SendFlags::RELIABLE);
                }
            }
        }
        
    }
    
}

fn handle_instantiate(
    mut evs_network: EventReader<NetworkPacket>,
    mut evs_unhandled: EventWriter<UnhandledInstantiation>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in evs_network.read() {
        let NetworkData::Instantiate(ref network_identity, ref pos) = ev.data else { continue; };
        println!("Instantiation");

        if network_identity.instantiation_path == "InstantiationExample" {
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
        } else {
            evs_unhandled.send(UnhandledInstantiation { network_identity: network_identity.clone(), position: *pos });
        }
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
                    bevy_steamworks::ChatMemberStateChange::Left | bevy_steamworks::ChatMemberStateChange::Disconnected => {
                        println!("Other left lobby");
                        client.lobby_status = LobbyStatus::OutOfLobby;
                        for (entity, networked) in network_query.iter() {
                            if (networked.owner_id == info.making_change) {
                                commands.entity(entity).despawn();
                            }
                        }
                    }
                    _ => println!("")
                }
            },
            SteamworksEvent::SteamServersConnected(_) => println!("Connected to steam servers!"),
            SteamworksEvent::AuthSessionTicketResponse(_) => println!("Ticket response"),
            SteamworksEvent::DownloadItemResult(_) => println!("Download item result"),
            SteamworksEvent::P2PSessionConnectFail(_) => println!("P2P Fail"),
            SteamworksEvent::P2PSessionRequest(_) => println!("P2P Session request"),
            SteamworksEvent::PersonaStateChange(persona) => {},
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