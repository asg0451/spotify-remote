use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serenity::prelude::TypeMapKey;

#[derive(Debug, Default)]
pub struct CredsRegistry {
    creds: HashMap<String, protocol::ForwardCreds>,
}

impl TypeMapKey for CredsRegistry {
    type Value = Arc<RwLock<CredsRegistry>>;
}

impl CredsRegistry {
    pub fn insert(&mut self, req: protocol::ForwardCreds) {
        self.creds.insert(req.key.clone(), req);
    }

    pub fn remove(&mut self, key: &str) {
        self.creds.remove(key);
    }

    pub fn get(&self, key: &str) -> Option<&protocol::ForwardCreds> {
        self.creds.get(key)
    }

    pub fn take(&mut self, key: &str) -> Option<protocol::ForwardCreds> {
        self.creds.remove(key)
    }
}
