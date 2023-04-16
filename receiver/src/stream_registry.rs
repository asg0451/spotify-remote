use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serenity::prelude::TypeMapKey;
use tokio::sync::mpsc::Receiver;

#[derive(Debug, Default)]
pub struct StreamRegistry {
    chans: HashMap<String, Receiver<Vec<u8>>>,
}

impl TypeMapKey for StreamRegistry {
    type Value = Arc<RwLock<StreamRegistry>>;
}

impl StreamRegistry {
    pub fn insert(&mut self, id: String, rx: Receiver<Vec<u8>>) {
        self.chans.insert(id, rx);
    }

    pub fn remove(&mut self, id: &str) {
        self.chans.remove(id);
    }

    pub fn get(&self, id: &str) -> Option<&Receiver<Vec<u8>>> {
        self.chans.get(id)
    }

    pub fn take(&mut self, id: &str) -> Option<Receiver<Vec<u8>>> {
        self.chans.remove(id)
    }
}
