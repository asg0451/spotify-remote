use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serenity::prelude::TypeMapKey;

use crate::pb;

#[derive(Debug, Default)]
pub struct CredsRegistry {
    creds: HashMap<String, pb::ForwardCredsRequest>,
}

impl TypeMapKey for CredsRegistry {
    type Value = Arc<RwLock<CredsRegistry>>;
}

impl CredsRegistry {
    pub fn insert(&mut self, req: pb::ForwardCredsRequest) {
        self.creds.insert(req.username.clone(), req);
    }

    pub fn remove(&mut self, username: &str) {
        self.creds.remove(username);
    }

    pub fn get(&self, username: &str) -> Option<&pb::ForwardCredsRequest> {
        self.creds.get(username)
    }

    pub fn take(&mut self, username: &str) -> Option<pb::ForwardCredsRequest> {
        self.creds.remove(username)
    }
}
