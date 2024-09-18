use bevy::*;
use prelude::*;
use bevy_steamworks::*;

use crate::*;

#[derive(Resource)]
pub struct SteamP2PClient {
    pub id: SteamId,
    pub lobby_status: LobbyStatus,
    pub steam_client: bevy_steamworks::Client,
    pub(crate)  lobby_channel: LobbyIdCallbackChannel,
    pub(crate) packet_channel: NetworkPacketChannel,
    instantiation_id: u32,
}

impl SteamP2PClient {
    pub fn new(steam_client: Client) -> SteamP2PClient {
        let steam_id = steam_client.user().steam_id();
        let (tx, rx) = flume::unbounded();
        let (tx2, rx2) = flume::unbounded();

        SteamP2PClient {
            id: steam_id,
            lobby_status: LobbyStatus::OutOfLobby,
            steam_client: steam_client.clone(),
            lobby_channel: LobbyIdCallbackChannel { tx, rx },
            packet_channel: NetworkPacketChannel { tx: tx2, rx: rx2 },
            instantiation_id: 0,
        }
    }
    pub fn create_lobby(&self) {
        let tx = self.lobby_channel.tx.clone();
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
        let tx = self.lobby_channel.tx.clone();
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
    pub fn send_message_all(&self, data: NetworkData, flags: SendFlags) -> Result<(), String> {
        self.packet_channel.tx.send(NetworkPacket { data: data.clone(), sender: self.id }).map_err(|e| println!("{e:?}"));
        return self.send_message_others(data, flags)
    }
    pub fn send_message_others(&self, data: NetworkData, flags: SendFlags) -> Result<(), String> {
        let lobby_id = self.get_lobby_id()?;
        for player in self.steam_client.matchmaking().lobby_members(lobby_id) {
            if player == self.id {
                continue;
            }
            self.send_message(&data, player, flags).map_err(|e| println!("Message error: {e}"));
        }
        return Ok(()); 
    }
    pub fn send_to_owner(&self, data: &NetworkData, flags: SendFlags) -> Result<(), String> {
        let lobby_id = self.get_lobby_id()?;
        let owner = self.get_lobby_owner()?;
        return self.send_message(data, owner, flags);
    }
    pub fn send_message(&self, data: &NetworkData, target: SteamId, flags: SendFlags) -> Result<(), String> {
        if !self.is_in_lobby() { return Err("Not in a lobby".to_string()); };
        let serialize_data = rmp_serde::to_vec(&data);
        let serialized = serialize_data.map_err(|err| err.to_string())?;
        let data_arr = serialized.as_slice();
        let network_identity = NetworkingIdentity::new_steam_id(target);
        let res = self.steam_client.networking_messages().send_message_to_user(network_identity, flags, data_arr, 0);
        return res.map_err(|e| e.to_string());
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
        self.send_message_all(NetworkData::Instantiate(NetworkIdentity { id: instantiation_id, owner_id: self.id }, path, pos), SendFlags::RELIABLE)
    }
    pub fn get_new_instantiation_id(&mut self) -> u32 {
        let id = self.instantiation_id;
        self.instantiation_id += 1;
        return id;
    }
}

pub struct LobbyIdCallbackChannel {
    pub tx: Sender<LobbyId>,
    pub rx: Receiver<LobbyId>
}

pub struct NetworkPacketChannel {
    pub tx: Sender<NetworkPacket>,
    pub rx: Receiver<NetworkPacket>
}

#[derive(PartialEq)]
pub enum LobbyStatus {
    InLobby(LobbyId),
    OutOfLobby
}