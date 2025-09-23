use bevy::prelude::*;
use bevy_steamworks::*;
use flume::{Receiver, Sender};
use networked_events::register::{NetworkedEventRegister, NetworkedEventsPlugin};
use networked_movable::{NetworkedMovable, NetworkedMovablePlugin};
use networked_transform::{NetworkedTransform, NetworkedTransformPlugin, TransformUpdate};
use serde::{Deserialize, Serialize};
use steamworks::networking_types::NetConnectionEnd;

pub mod client;
pub mod networked_events;
mod networked_movable;
pub mod networked_transform;
pub mod prelude;
pub use client::SteamP2PClient;
use steamworks::networking_types::SendFlags;
use steamworks::SteamId;

use crate::client::{ChannelPacket, LobbyStatus};
pub struct SteamP2PPlugin;

impl Plugin for SteamP2PPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SteamworksPlugin::init_app(480).unwrap())
            .add_plugins((
                NetworkedEventsPlugin,
                NetworkedMovablePlugin,
                NetworkedTransformPlugin,
            ))
            .add_systems(PreStartup, steam_start)
            .add_systems(
                Update,
                (
                    handle_channels,
                    steam_events,
                    receive_messages,
                    handle_network_data,
                    handle_instantiate,
                    handle_queued_instantiations,
                    handle_joiner,
                ),
            )
            .add_event::<LobbyJoined>()
            .add_event::<NetworkPacket>()
            .add_event::<UnhandledInstantiation>()
            .add_event::<LobbyLeft>()
            .add_event::<OtherJoined>()
            .add_event::<NetworkedAction>()
            .add_event::<NetworkInstantiation>();
    }
}

#[derive(Event)]
pub struct LobbyJoined {
    pub lobby_id: LobbyId,
}

#[derive(Event)]
pub struct OtherJoined(pub SteamId);

#[derive(Event)]
pub struct LobbyLeft;

#[derive(Event)]
pub struct NetworkedAction {
    pub network_identity: NetworkIdentity,
    pub action_id: u8,
    pub action_data: Vec<u8>,
}

#[derive(Event)]
pub struct NetworkInstantiation(pub InstantiationData);

#[derive(Event)]
pub struct UnhandledInstantiation(pub InstantiationData);

#[derive(Event, Clone, Debug)]
pub struct NetworkPacket {
    pub data: NetworkData,
    pub sender: SteamId,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkId {
    pub owner: SteamId,
    pub index: u32,
}

#[derive(Component, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkIdentity {
    pub id: NetworkId,
    pub parent_id: Option<NetworkId>,
    pub instantiation_path: FilePath,
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
    OtherJoined(SteamId),
    Event(Vec<u8>, u8),
    NetworkedAction(NetworkIdentity, u8, Vec<u8>), //NetworkId of receiver, id of action, data of action
    Instantiate(InstantiationData), //NetworkId of created object, optional network id of parent, starting position
    TransformUpdate(NetworkIdentity, Option<Vec3>, Option<Quat>, Option<Vec3>), //NetworkId of receiver, new position
    Destroy(NetworkIdentity), //NetworkId of object to be destroyed
    NetworkMessage(String), //Message for arbitrary communication, to be avoided outside of development
    DebugMessage(String),   //Make the receiving client print the message
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstantiationData {
    pub network_identity: NetworkIdentity,
    pub starting_transform: Transform,
}

fn handle_joiner(
    client: ResMut<SteamP2PClient>,
    mut evs: EventReader<SteamworksEvent>,
    networked_query: Query<(&NetworkIdentity, Option<&Transform>)>,
) {
    for ev in evs.read().map(|SteamworksEvent::CallbackResult(a)| a) {
        let CallbackResult::LobbyChatUpdate(update) = ev else {
            return;
        };
        if update.member_state_change == bevy_steamworks::ChatMemberStateChange::Entered {
            println!("Somebody joined your lobby: {:?}", update.user_changed);
            if client.is_lobby_owner().unwrap() {
                for (networked, transform) in networked_query.iter() {
                    println!("Replicate: {:?}", networked);
                    client
                        .send_message(
                            &NetworkData::Instantiate(InstantiationData {
                                network_identity: networked.clone(),
                                starting_transform: *transform.unwrap_or(&Transform::default()),
                            }),
                            update.user_changed,
                            SendFlags::RELIABLE,
                        )
                        .expect("Couldn't send data to joiner");
                }
            }
        }
    }
}

fn handle_instantiate(
    mut client: ResMut<SteamP2PClient>,
    mut evs_network: EventReader<NetworkInstantiation>,
    mut evs_unhandled: EventWriter<UnhandledInstantiation>,
    networked_query: Query<&NetworkIdentity>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for NetworkInstantiation(data) in evs_network.read() {
        if let Some(parent_id) = &data.network_identity.parent_id {
            if !networked_query
                .iter()
                .any(|n| n.id.owner == data.network_identity.id.owner && n.id == *parent_id)
            {
                client.add_to_instantiation_queue(data.clone());
                continue;
            }
        }
        //TODO: Add scene support once it comes out
        if data.network_identity.instantiation_path == "InstantiationExample" {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
                data.starting_transform,
                data.network_identity.clone(),
                NetworkedTransform::default(),
                NetworkedMovable { speed: 10. },
            ));
        } else {
            evs_unhandled.write(UnhandledInstantiation(data.clone()));
        }
    }
}

fn handle_queued_instantiations(
    mut client: ResMut<SteamP2PClient>,
    mut evs_network: EventWriter<NetworkInstantiation>,
    networked_query: Query<&NetworkIdentity>,
) {
    client.get_instantiation_queue().retain(|queued| {
        if networked_query.iter().any(|n| {
            n.id.owner == queued.network_identity.id.owner
                && n.id == queued.network_identity.parent_id.clone().unwrap()
        }) {
            evs_network.write(NetworkInstantiation(queued.clone()));
            return false;
        }
        return true;
    });
}

fn handle_network_data(
    mut commands: Commands,
    mut evs_network: EventReader<NetworkPacket>,
    mut ev_pos_update: EventWriter<TransformUpdate>,
    mut ev_network_instantiation: EventWriter<NetworkInstantiation>,
    mut ev_networked_action: EventWriter<NetworkedAction>,
    register: Res<NetworkedEventRegister>,
    mut other_joined_w: EventWriter<OtherJoined>,
) {
    for ev in evs_network.read() {
        match ev.data.clone() {
            NetworkData::NetworkedAction(id, action_id, action_data) => {
                ev_networked_action.write(NetworkedAction {
                    network_identity: id,
                    action_id,
                    action_data,
                });
            }
            NetworkData::TransformUpdate(id, position, rotation, scale) => {
                ev_pos_update.write(TransformUpdate {
                    network_identity: id,
                    position,
                    rotation,
                    scale,
                });
            }
            NetworkData::Destroy(_) => println!("Destroyed"),
            NetworkData::OtherJoined(id) => {
                println!("Other joined: {:?}", id);
                other_joined_w.write(OtherJoined(id));
            }
            NetworkData::DebugMessage(message) => {
                println!("Debug message from {:?}: {}", ev.sender, message)
            }
            NetworkData::Instantiate(data) => {
                ev_network_instantiation.write(NetworkInstantiation(data));
            }
            NetworkData::Event(data, index) => {
                let reader = &register.readers[index as usize];
                reader(&data, &mut commands)
            }
            _ => {}
        }
    }
}

fn receive_messages(client: Res<SteamP2PClient>, mut evs_network: EventWriter<NetworkPacket>) {
    while client
        .steam_client
        .networking()
        .is_p2p_packet_available()
        .is_some()
    {
        let mut buf = [0; 4096];
        let Some((sender, _)) = client.steam_client.networking().read_p2p_packet(&mut buf) else {
            break;
        };
        let data_try: Result<NetworkData, _> = rmp_serde::from_slice(&buf);

        if let Ok(data) = data_try {
            evs_network.write(NetworkPacket { sender, data });
        }
    }
}

fn handle_channels(
    mut client: ResMut<SteamP2PClient>,
    mut evs_joined: EventWriter<LobbyJoined>,
    mut evs_network: EventWriter<NetworkPacket>,
    mut evs_left: EventWriter<LobbyLeft>,
    mut commands: Commands,
    networked_query: Query<Entity, With<NetworkIdentity>>,
) {
    if let Ok(channel_packet) = client.steam_bevy_channel.rx.try_recv() {
        match channel_packet {
            ChannelPacket::LobbyJoined(lobby_id) => {
                client.lobby_status = LobbyStatus::InLobby(lobby_id);
                evs_joined.write(LobbyJoined { lobby_id });
                client
                    .send_message_others(
                        NetworkData::OtherJoined(client.steam_client.user().steam_id()),
                        SendFlags::RELIABLE,
                    )
                    .expect("Couldn't send other joined message");
                println!("Joined Lobby: {}", lobby_id.raw());
            }
            ChannelPacket::LobbyLeft => {
                evs_left.write(LobbyLeft);
                for entity in networked_query.iter() {
                    commands.entity(entity).despawn();
                }
                println!("Left Lobby")
            }
            ChannelPacket::NetworkPacket(network_packet) => {
                evs_network.write(network_packet);
            }
        }
    }
}

fn steam_start(steam_client: Res<Client>, mut commands: Commands) {
    let steam_id = steam_client.user().steam_id();
    println!("Connected: {}", steam_id.raw());
    steam_client.networking_utils().init_relay_network_access();
    steam_client
        .networking_messages()
        .session_request_callback(move |session_request| {
            if session_request.remote().steam_id() == Some(steam_id) {
                session_request.accept();
                return;
            }
            session_request.accept();
        });
    steam_client
        .networking_messages()
        .session_failed_callback(move |res| {
            if let Some(id) = res.identity_remote() {
                if id.steam_id() == Some(steam_id) {
                    return;
                }
            }
            println!(
                "Session Failed: {:?}",
                res.end_reason().unwrap_or(NetConnectionEnd::Other(-42))
            );
        });
    commands.insert_resource(SteamP2PClient::new(steam_client.clone()));
}

fn steam_events(
    mut evs: EventReader<SteamworksEvent>,
    client: Res<SteamP2PClient>,
    network_query: Query<(Entity, &NetworkIdentity)>,
    mut commands: Commands,
) {
    for ev in evs.read().map(|SteamworksEvent::CallbackResult(a)| a) {
        match ev {
            CallbackResult::GameLobbyJoinRequested(info) => {
                println!("Trying to join: {}", info.lobby_steam_id.raw());
                client.join_lobby(info.lobby_steam_id)
            }
            CallbackResult::LobbyChatUpdate(info) => match info.member_state_change {
                ChatMemberStateChange::Entered => {
                    println!("Other joined lobby !!!");
                }
                ChatMemberStateChange::Left | ChatMemberStateChange::Disconnected => {
                    println!("Other left lobby");
                    for (entity, networked) in network_query.iter() {
                        if networked.id.owner == info.making_change {
                            commands.entity(entity).despawn();
                        }
                    }
                }
                _ => println!("Lobby chat update: {:?}", info),
            },
            CallbackResult::SteamServersConnected(_) => println!("Connected to steam servers!"),
            CallbackResult::AuthSessionTicketResponse(_) => println!("Ticket response"),
            CallbackResult::DownloadItemResult(_) => println!("Download item result"),
            CallbackResult::P2PSessionConnectFail(_) => println!("P2P Fail"),
            CallbackResult::P2PSessionRequest(request) => {
                client
                    .steam_client
                    .networking()
                    .accept_p2p_session(request.remote);
            }
            CallbackResult::PersonaStateChange(_) => {}
            CallbackResult::SteamServerConnectFailure(_) => println!("Connection failed"),
            CallbackResult::SteamServersDisconnected(_) => println!("Disconnected"),
            CallbackResult::TicketForWebApiResponse(_) => println!("Ticket"),
            CallbackResult::UserAchievementStored(_) => println!("Achievement stored"),
            CallbackResult::UserStatsReceived(_) => println!("UserStatsReceived"),
            CallbackResult::UserStatsStored(_) => println!("User stats stored"),
            CallbackResult::ValidateAuthTicketResponse(_) => println!("Validate auth ticket"),
            CallbackResult::LobbyChatMsg(_) => println!("Lobby chat message received"),
            CallbackResult::FloatingGamepadTextInputDismissed(_) => {
                println!("Floating gamepad text input dismissed")
            }
            CallbackResult::GameOverlayActivated(_) => println!("Game overlay activated"),
            CallbackResult::GamepadTextInputDismissed(_) => {
                println!("Gamepad text input dismissed")
            }
            CallbackResult::GameRichPresenceJoinRequested(_) => {
                println!("Game rich presence join requested")
            }
            CallbackResult::LobbyCreated(_) => println!("Lobby created"),
            CallbackResult::LobbyDataUpdate(_) => println!("Lobby data update"),
            CallbackResult::LobbyEnter(_) => println!("Lobby enter"),
            CallbackResult::MicroTxnAuthorizationResponse(_) => {
                println!("MicroTxn authorization response")
            }
            CallbackResult::NetConnectionStatusChanged(_) => {
                println!("Net connection status changed")
            }
            CallbackResult::NetworkingMessagesSessionFailed(_) => {
                println!("Networking messages session failed")
            }
            CallbackResult::NetworkingMessagesSessionRequest(_) => {
                println!("Networking messages session request")
            }
            CallbackResult::RelayNetworkStatusCallback(_) => println!("Relay network status"),
            CallbackResult::RemotePlayConnected(_) => println!("Remote play connected"),
            CallbackResult::RemotePlayDisconnected(_) => println!("Remote play disconnected"),
            CallbackResult::ScreenshotRequested(_) => println!("Screenshot requested"),
            CallbackResult::ScreenshotReady(_) => println!("Screenshot ready"),
            CallbackResult::UserAchievementIconFetched(_) => {
                println!("User achievement icon fetched")
            }
            CallbackResult::GSClientApprove(_) => println!("GS client approve"),
            CallbackResult::GSClientDeny(_) => println!("GS client deny"),
            CallbackResult::GSClientKick(_) => println!("GS client kick"),
            CallbackResult::GSClientGroupStatus(_) => println!("GS client group status"),
            CallbackResult::NewUrlLaunchParameters(_) => println!("New URL launch parameters"),
        }
    }
}
