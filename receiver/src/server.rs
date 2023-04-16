use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::Result;
use axum::http::StatusCode;
use protocol::ForwardCreds;

use crate::creds_registry::CredsRegistry;
use crate::util;

pub struct Server {
    registry: Arc<RwLock<CredsRegistry>>,
}

impl Server {
    pub fn new(registry: Arc<RwLock<CredsRegistry>>) -> Self {
        Self { registry }
    }

    pub async fn run(self, port: u16) -> Result<()> {
        use axum::routing::post;
        use axum::Json;
        use axum::Router;

        let app = Router::new().route(
            "/api/forward_creds",
            post(|Json(payload): Json<ForwardCreds>| async move {
                tracing::debug!(?payload.key, ?payload.creds.username, ?payload.device_name, "got forwarded creds");
                let mut reg = self.registry.write().unwrap();
                reg.insert(payload);
                StatusCode::OK
            }),
        );

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        tracing::debug!("listening on {}", addr);
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .with_graceful_shutdown(util::ctrl_c())
            .await?;

        Ok(())
    }
}
