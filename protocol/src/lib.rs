use librespot_core::authentication::Credentials;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ForwardCreds {
    pub device_name: String,
    pub creds: Credentials,
}
