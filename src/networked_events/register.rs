use std::{any::TypeId, marker::PhantomData};

use bevy::{prelude::*, utils::hashbrown::HashMap};
use rmp_serde::from_slice;
use steamworks::networking_types::SendFlags;

use crate::{NetworkData, SteamP2PClient};

use super::event::{Networked, NetworkedEvent};

pub struct NetworkedEventsPlugin;

impl Plugin for NetworkedEventsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NetworkedEventRegister::new());
    }
}

pub trait NetworkedEvents {
    fn add_networked_event<T: NetworkedEvent>(&mut self) -> &mut Self;
}

impl<'de> NetworkedEvents for App {
    fn add_networked_event<T: NetworkedEvent>(&mut self) -> &mut Self {
        self.add_event::<T>();
        self.add_event::<Networked<T>>();
        self.add_systems(PostUpdate, networked_event_system::<T>);
        let mut register = self
            .world_mut()
            .get_resource_mut::<NetworkedEventRegister>()
            .unwrap();
        register.register::<T>();
        self
    }
}

fn networked_event_system<T: NetworkedEvent>(
    client: Res<SteamP2PClient>,
    mut networked_event_r: EventReader<Networked<T>>,
    mut event_w: EventWriter<T>,
    networked_event_register: Res<NetworkedEventRegister>,
) {
    for ev in networked_event_r.read() {
        if ev.emit_locally {
            event_w.send(ev.event);
        }
        let _ = client.send_message_others(
            NetworkData::Event(
                rmp_serde::to_vec(&ev.event).unwrap(),
                *networked_event_register
                    .indexes
                    .get(&TypeId::of::<T>())
                    .unwrap(),
            ),
            SendFlags::RELIABLE,
        );
    }
}

pub struct NetworkedEventReader<T: NetworkedEvent> {
    _marker: PhantomData<T>,
}

impl<T: NetworkedEvent> NetworkedEventReader<T> {
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

    pub fn register<T: NetworkedEvent>(&mut self) {
        self.indexes.insert(TypeId::of::<T>(), self.counter);
        self.counter += 1;
        self.readers.push(|buffer: &[u8], commands: &mut Commands| {
            let unserialized = from_slice::<T>(buffer).unwrap();
            commands.send_event(unserialized);
        });
    }
}
