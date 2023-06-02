use librespot_core::authentication::Credentials;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ForwardCreds {
    pub device_name: String,
    pub key: String,
    pub creds: Credentials,
}

// custom implementation to not show actual creds in logs
impl std::fmt::Debug for ForwardCreds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForwardCreds")
            .field("device_name", &self.device_name)
            .field("key", &self.key)
            .field("creds", &self.creds.username)
            .finish()
    }
}
