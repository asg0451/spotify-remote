use anyhow::{anyhow, Context, Result};
use clap::Parser;
use librespot::{
    connect::spirc::Spirc,
    core::{
        config::{ConnectConfig, SessionConfig},
        session::Session,
        spotify_id::SpotifyId,
    },
    discovery::Credentials,
    metadata::Metadata,
    playback::{
        config::{AudioFormat, PlayerConfig, VolumeCtrl},
        mixer::{self, MixerConfig},
        player::Player as SpotifyPlayer,
    },
};
use protocol::TrackInfo;
use sha1::{Digest, Sha1};

use common::util;

#[derive(Debug, Parser)]
pub struct Options {
    #[clap(short, long, default_value = "danube")]
    device_name: String,
    #[clap(short, long, default_value = "localhost:9090")]
    player_updates_addr: String,
    #[clap(short, long, default_value = "blah")]
    player_updates_token: String,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let opts = Options::parse();
    util::setup_logging()?;
    let _ = util::load_env(".env");

    let evts_client = reqwest::Client::new();

    // read creds as json from stdin
    let creds: Credentials = serde_json::from_reader(std::io::stdin())?;

    let format = AudioFormat::default();
    let mixer = mixer::find(None).unwrap();
    let mixer_config = MixerConfig {
        volume_ctrl: VolumeCtrl::Log(VolumeCtrl::DEFAULT_DB_RANGE),
        ..Default::default()
    };

    let connect_config = ConnectConfig {
        name: opts.device_name.clone(),
        initial_volume: None,
        has_volume_ctrl: true,
        autoplay: true,
        device_type: Default::default(),
    };

    let session_config = SessionConfig {
        device_id: device_id(&connect_config.name),
        ..Default::default()
    };

    // TODO: or bitrate 360
    let player_config = PlayerConfig::default();

    tracing::debug!("connecting to spotify...");

    let (session, _reusable_creds) =
        Session::connect(session_config.clone(), creds, None, false).await?;

    let mixer_config = mixer_config.clone();
    let mixer = (mixer)(mixer_config);
    let player_config = player_config.clone();
    let connect_config = connect_config.clone();

    let soft_volume = mixer.get_soft_volume();
    let format = format;

    let backend = librespot::playback::audio_backend::find(None).unwrap();

    let (player, events) =
        SpotifyPlayer::new(player_config, session.clone(), soft_volume, move || {
            (backend)(None, format)
        });

    let (spirc, spirc_task) = Spirc::new(connect_config, session.clone(), player, mixer);

    let updates_task = tokio::spawn(async move {
        let mut events = events;
        while let Some(event) = events.recv().await {
            tracing::debug!("event: {:?}", event);
            match event {
                librespot::playback::player::PlayerEvent::Stopped { .. } => {
                    send_player_event(&evts_client, &opts, protocol::PlayerEvent::Stopped).await?;
                }
                librespot::playback::player::PlayerEvent::Playing { track_id, .. } => {
                    send_player_event(
                        &evts_client,
                        &opts,
                        protocol::PlayerEvent::Playing(get_track_info(&session, track_id).await?),
                    )
                    .await?;
                }
                librespot::playback::player::PlayerEvent::Paused { track_id, .. } => {
                    send_player_event(
                        &evts_client,
                        &opts,
                        protocol::PlayerEvent::Paused(get_track_info(&session, track_id).await?),
                    )
                    .await?;
                }
                _ => (),
            };
        }
        Ok::<(), anyhow::Error>(())
    });

    tracing::debug!("connected!");

    tokio::select! {
        _ = spirc_task => {
            tracing::debug!("spirc task finished");
        }
        _ = updates_task => {
            tracing::debug!("updates task finished");
        }
        _ = util::ctrl_c() => {
            // TODO: send player events shutdown?
            // TODO: cancel token? +wait?
            tracing::debug!("ctrl-c received");
        },
    };

    // Shutdown spirc gracefully if necessary
    spirc.shutdown();

    Ok(())
}

fn device_id(name: &str) -> String {
    hex::encode(Sha1::digest(name.as_bytes()))
}

// TODO: clean this up
async fn send_player_event(
    client: &reqwest::Client,
    opts: &Options,
    pe: protocol::PlayerEvent,
) -> Result<()> {
    let endpoint = format!("{}/api/player_events", &opts.player_updates_addr);
    let token = opts.player_updates_token.clone();
    client
        .clone()
        .post(&endpoint)
        .header("Authorization", format!("Bearer {}", &token))
        .query(&[("token", &token)])
        .json(&pe)
        .send()
        .await
        .context("sending player event")?;
    Ok(())
}

// TODO: cache?
// have to do this here since this is the only place we have a session
async fn get_track_info(session: &Session, track_id: SpotifyId) -> Result<TrackInfo> {
    use librespot::metadata::{Album, Artist, Track};
    let track = Track::get(session, track_id)
        .await
        .map_err(|_| anyhow!("error getting track"))?;

    let artists = track.artists.into_iter().map(|id| Artist::get(session, id));

    let artists = futures::future::join_all(artists)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| anyhow!("error getting artists"))?;

    let album = Album::get(session, track.album)
        .await
        .map_err(|_| anyhow!("error getting album"))?;

    Ok(TrackInfo {
        id: track_id,
        name: track.name,
        artists: artists
            .into_iter()
            .map(|artist| artist.name)
            .collect::<Vec<_>>(),
        album: album.name,
        duration: track.duration as u32,
    })
}
