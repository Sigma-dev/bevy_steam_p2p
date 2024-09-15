use std::{path::Path, time::Duration};

use bevy::*;
use prelude::*;
use bevy_steamworks::*;
use flume::{Receiver, Sender};
use networked_movable::{ NetworkedMovable, NetworkedMovablePlugin};
use ::serde::{Deserialize, Serialize};
use steamworks::{networking_types::{ NetConnectionEnd, NetworkingIdentity, SendFlags}, LobbyChatUpdate};
mod networked_movable;
pub struct SteamP2PPlugin;

impl Plugin for SteamP2PPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_plugins(SteamworksPlugin::init_app(480).unwrap())
        .add_plugins(NetworkedMovablePlugin)
        .add_systems(Startup, steam_start)
        .add_systems(Update, (steam_system, steam_events, receive_messages))
        .add_systems(FixedUpdate, handle_networked_transform)
        .add_event::<PositionUpdate>();
    }
}

#[derive(Resource)]
pub struct SteamP2PClient {
    pub id: SteamId,
    pub lobby_status: LobbyStatus,
    steam_client: bevy_steamworks::Client,
    channel: LobbyIdCallbackChannel,
    instantiation_id: u32,
}

impl SteamP2PClient {
    pub fn create_lobby(&self) {
        let tx = self.channel.tx.clone();
        if self.lobby_status != LobbyStatus::OutOfLobby { return; };
        self.steam_client.matchmaking().create_lobby(LobbyType::Public, 2, 
            move |res| {
                if let Ok(lobby_id) = res {
                    match tx.send(lobby_id) {
                        Ok(_) => {}
                        Err(_) => {
                        }
                    }
                }
            });
    }
    pub fn join_lobby(&self, lobby_id: LobbyId) {
        let tx = self.channel.tx.clone();
        self.steam_client.matchmaking().join_lobby(lobby_id, 
            move |res| {
                if let Ok(lobby_id) = res {
                    match tx.send(lobby_id) {
                        Ok(_) => {}
                        Err(_) => {
                        }
                    }
                }
            });
    }
    pub fn leave_lobby(&mut self) {
        let LobbyStatus::InLobby(lobby) = self.lobby_status else {return; };
        println!("Leave");
        self.steam_client.matchmaking().leave_lobby(lobby);
        self.lobby_status = LobbyStatus::OutOfLobby;
    }
    pub fn send_message_all(&self, data: NetworkData, only_others: bool) -> Result<(), String> {
        let lobby_id = self.get_lobby_id()?;
        for player in self.steam_client.matchmaking().lobby_members(lobby_id) {
            if only_others && player == self.id {
                continue;
            }
            self.send_message(&data, player);
        }
        return Ok(()); 
    }
    pub fn send_to_owner(&self, data: &NetworkData) -> Result<(), String> {
        let lobby_id = self.get_lobby_id()?;
        let owner = self.get_lobby_owner()?;
        self.send_message(data, owner);
        Ok(())
    }
    pub fn send_message(&self, data: &NetworkData, target: SteamId) -> Result<(), String> {
        if !self.is_in_lobby() { return Err("Not in a lobby".to_string()); };
       // println!("Sending to: {}", target.raw());
        let serialize_data = rmp_serde::to_vec(&data);
        let serialized = serialize_data.map_err(|err| err.to_string())?;
        let data_arr = serialized.as_slice();
        let network_identity = NetworkingIdentity::new_steam_id(target);
        let res = self.steam_client.networking_messages().send_message_to_user(network_identity, SendFlags::RELIABLE, data_arr, 0);
        match res {
            Ok(_) => return Ok(()),
            Err(err) => return Err(format!("Message error: {}", err.to_string())),
        }
    }
    pub fn is_in_lobby(&self) -> bool {
        return self.lobby_status != LobbyStatus::OutOfLobby;
    }
    pub fn is_lobby_owner(&self) ->  Result<bool, String> {
        let owner = self.get_lobby_owner()?;
        return Ok(owner == self.id);
    }
    pub fn get_lobby_id(&self) -> Result<LobbyId, String> {
        match self.lobby_status {
            LobbyStatus::InLobby(lobby_id) => return Ok(lobby_id),
            LobbyStatus::OutOfLobby => return Err("Out of lobby".to_owned()),
        }
    }
    pub fn get_lobby_owner(&self) -> Result<SteamId, String> {
        let lobby_id = self.get_lobby_id()?;
        let owner = self.steam_client.matchmaking().lobby_owner(lobby_id);
        return Ok(owner);
    }
    pub fn instantiate(
        &mut self,
        path: FilePath,
        pos: Vec3,
    ) -> Result<(), String> {
        let instantiation_id = self.get_new_instantiation_id();
        self.send_message_all(NetworkData::Instantiate(NetworkIdentity { id: instantiation_id, owner_id: self.id }, path, pos), false)
    }
    pub fn get_new_instantiation_id(&mut self) -> u32 {
        let id = self.instantiation_id;
        self.instantiation_id += 1;
        return id;
    }
}

#[derive(PartialEq)]
pub enum LobbyStatus {
    InLobby(LobbyId),
    OutOfLobby
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

#[derive(Serialize, Deserialize, Debug)]
pub struct FilePath(pub u32);

#[derive(Event)]
struct PositionUpdate {
    network_identity: NetworkIdentity, 
    new_position: Vec3
}

#[derive(Serialize, Deserialize, Debug)]
pub enum NetworkData {
    Handshake,
    SendObjectData(NetworkIdentity, i8, Vec<u8>), //NetworkId of receiver, id of action, data of action
    Instantiate(NetworkIdentity, FilePath, Vec3), //NetworkId of created object, filepath of prefab, starting position
    PositionUpdate(NetworkIdentity, Vec3), //NetworkId of receiver, new position
    Destroy(NetworkIdentity), //NetworkId of object to be destroyed
}

pub struct LobbyIdCallbackChannel {
    pub tx: Sender<LobbyId>,
    pub rx: Receiver<LobbyId>
}

fn lobby_joined(client: &mut ResMut<SteamP2PClient>, info: &LobbyChatUpdate) {
    println!("Somebody joined your lobby: {:?}", info.user_changed);
}

fn instantiate(
    network_id: NetworkIdentity,
    path: FilePath,
    pos: Vec3,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    println!("Instantiation");
    if path.0 == 0 {
        commands.spawn((
            PbrBundle {
            mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            material: materials.add(Color::srgb_u8(124, 144, 255)),
            transform: Transform::from_translation(pos),
            ..default()
            },
            network_id.clone(),
            NetworkedTransform{synced: true, target: pos},
            NetworkedMovable { speed: 10. }
        ));
    }
}

fn handle_networked_transform(
    client: Res<SteamP2PClient>,
    mut networked_transform_query: Query<(&mut Transform, &NetworkIdentity, &mut NetworkedTransform)>,
    mut ev_reader: EventReader<PositionUpdate>,
    time: Res<Time>
) {
    let mut updates = Vec::new();
    for ev in ev_reader.read() {
        updates.push(ev)
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
        client.send_message_all(NetworkData::PositionUpdate(*network_identity, transform.translation), true);
    }
}

fn receive_messages(
    mut client: ResMut<SteamP2PClient>, 
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ev_pos_update: EventWriter<PositionUpdate>,
) {
    let messages: Vec<steamworks::networking_types::NetworkingMessage<steamworks::ClientManager>> = client.steam_client.networking_messages().receive_messages_on_channel(0, 1);
    if (messages.len() > 0) {
       // println!("Received {} messages", messages.len())
    }
    for message in messages {
        let sender = message.identity_peer().steam_id().unwrap();
        let serialized_data = message.data();
        let data_try: Result<NetworkData, _> = rmp_serde::from_slice(serialized_data);
        match data_try {
            Ok(data) => match data {
                NetworkData::SendObjectData(id, action_id, action_data) => println!("Action"),
                NetworkData::Instantiate(id, prefab_path, pos) => instantiate(id, prefab_path, pos, &mut commands, &mut meshes, &mut materials),
                NetworkData::PositionUpdate(id, pos) => {ev_pos_update.send(PositionUpdate { network_identity: id, new_position: pos }); },
                NetworkData::Destroy(id) => println!("Destroyed"),
                NetworkData::Handshake => {
                    println!("Received handshake");
                },
            },
            Err(err) => println!("{}", err.to_string())
        }
        drop(message); //not sure about usefullness, mentionned in steam docs as release
    }
}

fn steam_system(
    mut client: ResMut<SteamP2PClient>,
) { 
    let rx = client.channel.rx.clone();

    if let Ok(lobby_id) = rx.try_recv() {
        client.lobby_status = LobbyStatus::InLobby(lobby_id);
        println!("Joined Lobby: {}", lobby_id.raw());
    }
}

fn steam_start(
    steam_client: Res<Client>,
    mut commands: Commands,
) {
    println!("Connected: {}", steam_client.user().steam_id().raw());
    steam_client.networking_utils().init_relay_network_access();
    steam_client.networking_messages().session_request_callback(
        |res| {
            println!("Accepted");
            match res.accept() {
                true => println!("Succesfully accepted"),
                false => println!("Failed to accept"),
            }
        }
    );
    steam_client.networking_messages().session_failed_callback(
        |res| {
            println!("Session Failed: {:?}", res.end_reason().unwrap_or(NetConnectionEnd::Other(-42)));
        }
    );
    let (tx, rx) = flume::unbounded();

    commands.insert_resource(SteamP2PClient {
        id: steam_client.user().steam_id(),
        lobby_status: LobbyStatus::OutOfLobby,
        steam_client: steam_client.clone(),
        channel: LobbyIdCallbackChannel { tx, rx },
        instantiation_id: 0,
    });
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
                println!("Chat Update");
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