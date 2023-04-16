
use std::sync::Arc;
use std::{sync::RwLock};
use tokio::sync::mpsc;

use tonic::{Request, Response, Status};

use crate::pb::{self, spotify_remote_server::SpotifyRemote};
use crate::stream_registry::StreamRegistry;

pub struct Server {
    registry: Arc<RwLock<StreamRegistry>>,
}

impl Server {
    pub fn new(registry: Arc<RwLock<StreamRegistry>>) -> Self {
        Self { registry }
    }
}

#[tonic::async_trait]
impl SpotifyRemote for Server {
    async fn send_audio(
        &self,
        req: Request<tonic::Streaming<pb::AudioChunk>>,
    ) -> Result<Response<pb::SendAudioResponse>, Status> {
        let mut id = None;
        let mut tx = None;
        let mut stream = req.into_inner();
        while let Some(chunk) = stream.message().await? {
            tracing::trace!("Got chunk: {:?}", chunk);
            if id.is_none() {
                // first msg
                tracing::info!(?chunk.id, "stream started");

                id = Some(chunk.id.clone());
                let (tx_, rx) = mpsc::channel(1024);
                tx = Some(tx_);

                {
                    let mut registry = self.registry.write().unwrap();
                    registry.insert(chunk.id.clone(), rx);
                }
            }

            tx.as_ref().unwrap().send(chunk.data).await.unwrap();
        }

        tx.take(); // close channel
        {
            let mut registry = self.registry.write().unwrap();
            registry.remove(id.as_ref().unwrap());
        }
        tracing::info!(?id, "Finished sending audio");

        Ok(Response::new(pb::SendAudioResponse {}))
    }
}
