use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct CredsRegistry {
    creds: HashMap<String, protocol::ForwardCreds>,
}

impl CredsRegistry {
    // will NOT overwrite a key. false if key already exists and the insert failed
    pub fn insert(&mut self, req: protocol::ForwardCreds) -> bool {
        let key = req.key.clone();
        if self.creds.contains_key(&key) {
            return false;
        }
        self.creds.insert(key, req);
        true
    }

    pub fn take(&mut self, key: &str) -> Option<protocol::ForwardCreds> {
        self.creds.remove(key)
    }
}
