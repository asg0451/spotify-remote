use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct CredsRegistry {
    creds: HashMap<String, protocol::ForwardCreds>,
}

impl CredsRegistry {
    pub fn insert(&mut self, req: protocol::ForwardCreds) {
        self.creds.insert(req.key.clone(), req);
    }

    pub fn take(&mut self, key: &str) -> Option<protocol::ForwardCreds> {
        self.creds.remove(key)
    }
}
