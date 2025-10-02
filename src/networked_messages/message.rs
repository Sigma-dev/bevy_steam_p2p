use bevy::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

pub trait NetworkedMessage: Message + Serialize + DeserializeOwned + Clone {}
impl<T: Message + Serialize + DeserializeOwned + Clone> NetworkedMessage for T {}

#[derive(Message)]
pub struct Networked<T>
where
    T: NetworkedMessage,
{
    pub message: T,
    pub emit_locally: bool,
}

impl<T> Networked<T>
where
    T: NetworkedMessage,
{
    pub fn new(message: T) -> Self {
        Networked {
            message,
            emit_locally: true,
        }
    }

    pub fn new_only_others(message: T) -> Self {
        Networked {
            message,
            emit_locally: false,
        }
    }
}
