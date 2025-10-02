use std::{any::TypeId, marker::PhantomData};

use bevy::{platform::collections::HashMap, prelude::*};
use rmp_serde::from_slice;
use steamworks::networking_types::SendFlags;

use crate::{networked_messages::message::NetworkedMessage, NetworkData, SteamP2PClient};

use super::message::Networked;

pub struct NetworkedMessagesPlugin;

impl Plugin for NetworkedMessagesPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NetworkedMessageRegister::new());
    }
}

pub trait NetworkedMessages {
    fn add_networked_message<T: NetworkedMessage>(&mut self) -> &mut Self;
}

impl<'de> NetworkedMessages for App {
    fn add_networked_message<T: NetworkedMessage>(&mut self) -> &mut Self {
        self.add_message::<T>();
        self.add_message::<Networked<T>>();
        self.add_systems(PostUpdate, networked_message_system::<T>);
        let mut register = self
            .world_mut()
            .get_resource_mut::<NetworkedMessageRegister>()
            .unwrap();
        register.register::<T>();
        self
    }
}

fn networked_message_system<T: NetworkedMessage>(
    client: Res<SteamP2PClient>,
    mut networked_message_r: MessageReader<Networked<T>>,
    mut message_w: MessageWriter<T>,
    networked_message_register: Res<NetworkedMessageRegister>,
) {
    for ev in networked_message_r.read() {
        if ev.emit_locally {
            message_w.write(ev.message.clone());
        }
        let _ = client.send_message_others(
            NetworkData::Message(
                rmp_serde::to_vec(&ev.message).unwrap(),
                *networked_message_register
                    .indexes
                    .get(&TypeId::of::<T>())
                    .unwrap(),
            ),
            SendFlags::RELIABLE,
        );
    }
}

pub struct NetworkedMessageReader<T: NetworkedMessage> {
    _marker: PhantomData<T>,
}

#[derive(Resource)]
pub struct NetworkedMessageRegister {
    pub readers: Vec<fn(&[u8], &mut Commands) -> ()>,
    pub indexes: HashMap<TypeId, u8>,
    pub counter: u8,
}

impl NetworkedMessageRegister {
    pub fn new() -> NetworkedMessageRegister {
        NetworkedMessageRegister {
            readers: Vec::new(),
            indexes: HashMap::new(),
            counter: 0,
        }
    }

    pub fn register<T: NetworkedMessage>(&mut self) {
        self.indexes.insert(TypeId::of::<T>(), self.counter);
        self.counter += 1;
        self.readers.push(|buffer: &[u8], commands: &mut Commands| {
            let unserialized = from_slice::<T>(buffer).unwrap();
            commands.write_message(unserialized);
        });
    }
}
