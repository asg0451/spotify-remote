

use anyhow::Result;
use futures_util::StreamExt;
use librespot::{
    discovery::Credentials,
};
use sha1::{Digest, Sha1};
use tonic::transport::Channel;

use crate::pb::spotify_remote_client::SpotifyRemoteClient;

#[derive(Debug)]
pub struct Transmitter {
    grpc_client: SpotifyRemoteClient<Channel>,
    device_name: String,
}

impl Transmitter {
    pub async fn new(receiver_addr: String, device_name: String) -> Result<Self> {
        let grpc_client = SpotifyRemoteClient::connect(receiver_addr).await?;
        Ok(Self {
            grpc_client,
            device_name,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        // pretend to be a spotify receiver to grab credentials

        let device_id = device_id(&self.device_name);

        let mut discovery = librespot::discovery::Discovery::builder(device_id)
            .name(self.device_name.clone())
            .launch()?;

        tracing::debug!("Starting discovery loop");

        loop {
            tracing::debug!("discovery loop");
            tokio::select! {
                credentials = discovery.next() => {
                    tracing::debug!("discovery next");
                    match credentials {
                        Some(credentials) => {
                            tracing::debug!(?credentials, "got creds");
                            self.forward_creds(self.device_name.clone(), credentials).await?;
                            tracing::debug!("forwarded");
                        },
                        None => {
                            anyhow::bail!("Discovery stopped unexpectedly");
                        }
                    }
                },
                 _ = tokio::signal::ctrl_c() => {
                    break;
                },
                else => break,
            }
        }
        tracing::info!("Gracefully shutting down");

        Ok(())
    }

    async fn forward_creds(&mut self, device_name: String, creds: Credentials) -> Result<()> {
        let _resp = self
            .grpc_client
            .forward_creds(crate::pb::ForwardCredsRequest {
                device_name,
                username: creds.username.clone(),
                creds_json: serde_json::to_string(&creds)?,
            })
            .await?;
        Ok(())
    }
}

fn device_id(name: &str) -> String {
    hex::encode(Sha1::digest(name.as_bytes()))
}
