use bevy::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

#[derive(Event)]
pub struct Networked<T>
where
    T: Event + Serialize + DeserializeOwned + Copy,
{
    pub event: T,
}

impl<T> Networked<T>
where
    T: Event + Serialize + DeserializeOwned + Copy,
{
    pub fn new(event: T) -> Self {
        Networked { event }
    }
}
