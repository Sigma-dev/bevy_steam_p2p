use bevy::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

pub trait NetworkedEvent: Event + Serialize + DeserializeOwned + Copy {}
impl<T: Event + Serialize + DeserializeOwned + Copy> NetworkedEvent for T {}

#[derive(Event)]
pub struct Networked<T>
where
    T: NetworkedEvent,
{
    pub event: T,
}

impl<T> Networked<T>
where
    T: NetworkedEvent,
{
    pub fn new(event: T) -> Self {
        Networked { event }
    }
}
