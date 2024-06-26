use anyhow::Result;
use clap::Parser;
use librespot::{
    connect::spirc::Spirc,
    core::{
        config::{ConnectConfig, SessionConfig},
        session::Session,
    },
    discovery::Credentials,
    playback::{
        config::{AudioFormat, PlayerConfig, VolumeCtrl},
        mixer::{self, MixerConfig},
        player::Player as SpotifyPlayer,
    },
};
use sha1::{Digest, Sha1};

use common::util;

#[derive(Debug, Parser)]
pub struct Options {
    #[clap(short, long, default_value = "danube")]
    device_name: String,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let opts = Options::parse();
    util::setup_logging()?;
    let _ = util::load_env(".env");

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

    // TODO: or bitrate 360? configurable?
    let player_config = PlayerConfig::default();

    tracing::debug!("connecting to spotify...");

    let (session, _reusable_creds) =
        Session::connect(session_config.clone(), creds, None, false).await?;

    let mixer = (mixer)(mixer_config);
    let player_config = player_config.clone();
    let connect_config = connect_config.clone();

    let soft_volume = mixer.get_soft_volume();

    let backend = librespot::playback::audio_backend::find(None).unwrap();

    let (player, _) = SpotifyPlayer::new(player_config, session.clone(), soft_volume, move || {
        (backend)(None, format)
    });

    let (spirc, spirc_task) = Spirc::new(connect_config, session.clone(), player, mixer);

    tracing::debug!("connected!");

    tokio::pin!(spirc_task);

    tokio::select! {
        _ = &mut spirc_task => {
            tracing::debug!("spirc task finished");
        }
        _ = util::ctrl_c_and_pipe() => {
            // what happens is songbird sends SIGKILL(9) to the last child -- gstreamer. presumably then its stdin is closed, which means our stdout is closed -> SIGPIPE
            // actually what happens is the Player fails to write to stoud and then calls std::process::exit(1) :(
            // TODO: what can we do about that
            tracing::debug!("received ctrl-c or pipe");
        },
    };

    tracing::debug!("exiting");
    // shutdown spirc gracefully
    spirc.shutdown();
    // wait for the task to finish
    tokio::time::timeout(std::time::Duration::from_secs(5), spirc_task).await?;
    tracing::debug!("spirc task finished");

    Ok(())
}

fn device_id(name: &str) -> String {
    hex::encode(Sha1::digest(name.as_bytes()))
}
