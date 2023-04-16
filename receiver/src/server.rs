use std::sync::Arc;
use std::sync::RwLock;

use tonic::{Request, Response, Status};

use crate::creds_registry::CredsRegistry;
use crate::pb::{self, spotify_remote_server::SpotifyRemote};

pub struct Server {
    registry: Arc<RwLock<CredsRegistry>>,
}

impl Server {
    pub fn new(registry: Arc<RwLock<CredsRegistry>>) -> Self {
        Self { registry }
    }
}

#[tonic::async_trait]
impl SpotifyRemote for Server {
    async fn forward_creds(
        &self,
        request: Request<pb::ForwardCredsRequest>,
    ) -> Result<Response<pb::ForwardCredsResponse>, Status> {
        let request = request.into_inner();
        tracing::info!(?request.username, "Forwarded creds");
        {
            let mut reg = self.registry.write().unwrap();
            reg.insert(request);
        }
        Ok(Response::new(pb::ForwardCredsResponse {}))
    }
}
