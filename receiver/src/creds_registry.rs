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
        self.creds.insert(req.creds.username.clone(), req);
    }

    pub fn remove(&mut self, username: &str) {
        self.creds.remove(username);
    }

    pub fn get(&self, username: &str) -> Option<&protocol::ForwardCreds> {
        self.creds.get(username)
    }

    pub fn take(&mut self, username: &str) -> Option<protocol::ForwardCreds> {
        self.creds.remove(username)
    }
}
