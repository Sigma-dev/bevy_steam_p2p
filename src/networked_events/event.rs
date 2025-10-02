use bevy::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

pub trait NetworkedEvent: Event + Serialize + DeserializeOwned + Clone {}
impl<T: Event + Serialize + DeserializeOwned + Clone> NetworkedEvent for T {}

#[derive(Event)]
pub struct Networked<T>
where
    T: NetworkedEvent,
{
    pub event: T,
    pub emit_locally: bool,
}

impl<T> Networked<T>
where
    T: NetworkedEvent,
{
    pub fn new(event: T) -> Self {
        Networked {
            event,
            emit_locally: true,
        }
    }

    pub fn new_only_others(event: T) -> Self {
        Networked {
            event,
            emit_locally: false,
        }
    }
}
