use librespot_core::{authentication::Credentials, spotify_id::SpotifyId};
use serde::{Deserialize, Serialize};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ForwardCreds {
    pub device_name: String,
    pub key: String,
    pub creds: Credentials,
}

// for receiver <-> player comms

// subset of librespot::playback::player::PlayerEvent
// as that doesnt have serde support and i dont wanna copy it and this is all we need anyway
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlayerEvent {
    Playing(TrackInfo),
    Paused(TrackInfo),
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    #[serde(
        serialize_with = "serialize_spotify_id",
        deserialize_with = "deserialize_spotify_id"
    )]
    pub id: SpotifyId,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration: u32,
}

fn serialize_spotify_id<S>(id: &SpotifyId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&id.to_base62().map_err(serde::ser::Error::custom)?)
}

fn deserialize_spotify_id<'de, D>(deserializer: D) -> Result<SpotifyId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    SpotifyId::from_base62(&s).map_err(|e| serde::de::Error::custom("invalid spotify id"))
}
