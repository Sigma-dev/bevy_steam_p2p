use std::{any::TypeId, marker::PhantomData};

use bevy::{ecs::event, prelude::*, utils::hashbrown::HashMap};
use rmp_serde::from_slice;
use serde::{de::DeserializeOwned, Serialize};
use steamworks::networking_types::SendFlags;

use crate::{NetworkData, SteamP2PClient};

use super::event::Networked;

pub trait NetworkedEvents {
    fn add_networked_event<T: Event + Serialize + DeserializeOwned + Copy>(&mut self) -> &mut Self;
}

impl<'de> NetworkedEvents for App {
    fn add_networked_event<T: Event + Serialize + DeserializeOwned + Copy>(&mut self) -> &mut Self {
        self.add_event::<T>();
        self.add_event::<Networked<T>>();
        self.add_systems(PostUpdate, networked_event_system::<T>);
        let mut register = self
            .world_mut()
            .get_resource_or_insert_with::<NetworkedEventRegister>(NetworkedEventRegister::new);
        register.register::<T>();
        self
    }
}

fn networked_event_system<T: Event + Serialize + DeserializeOwned + Copy>(
    mut client: ResMut<SteamP2PClient>,
    mut networked_event_r: EventReader<Networked<T>>,
    mut event_w: EventWriter<T>,
    networked_event_register: Res<NetworkedEventRegister>,
) {
    for ev in networked_event_r.read() {
        event_w.send(ev.event);
        client.send_message_others(
            NetworkData::NetworkedEvent(
                rmp_serde::to_vec(&ev.event).unwrap(),
                *networked_event_register
                    .indexes
                    .get(&TypeId::of::<T>())
                    .unwrap(),
            ),
            (SendFlags::RELIABLE),
        );
    }
}

pub struct NetworkedEventReader<T: Event + Serialize + DeserializeOwned + Copy> {
    _marker: PhantomData<T>,
}

impl<T: Event + Serialize + DeserializeOwned + Copy> NetworkedEventReader<T> {
    fn new() -> NetworkedEventReader<T> {
        NetworkedEventReader {
            _marker: PhantomData,
        }
    }

    fn deserialize(data: &[u8]) -> T {
        from_slice(data).unwrap()
    }
}

#[derive(Resource)]
pub struct NetworkedEventRegister {
    pub readers: Vec<fn(&[u8], &mut Commands) -> ()>,
    pub indexes: HashMap<TypeId, u8>,
    pub counter: u8,
}

impl NetworkedEventRegister {
    pub fn new() -> NetworkedEventRegister {
        NetworkedEventRegister {
            readers: Vec::new(),
            indexes: HashMap::new(),
            counter: 0,
        }
    }

    pub fn register<T: Event + Serialize + DeserializeOwned + Copy>(&mut self) {
        self.indexes.insert(TypeId::of::<T>(), self.counter);
        self.counter += 1;
        self.readers.push(|buffer: &[u8], commands: &mut Commands| {
            let unserialized = from_slice::<T>(buffer).unwrap();
            commands.send_event(unserialized);
        });
    }
}
