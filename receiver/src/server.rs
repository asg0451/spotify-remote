use std::net::SocketAddr;
use std::sync::Arc;

use std::sync::RwLock;

use anyhow::Result;
use axum::debug_handler;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use protocol::ForwardCreds;
use serde::Deserialize;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::creds_registry::CredsRegistry;
use crate::player_events_manager::PlayerEventWithToken;

#[derive(Debug, Clone)]
pub struct AppState {
    creds: Arc<RwLock<CredsRegistry>>,
    add_player_event: Sender<PlayerEventWithToken>,
}

#[derive(Debug, Deserialize)]
struct PeParams {
    token: String,
}

impl AppState {
    pub fn new(
        creds: Arc<RwLock<CredsRegistry>>,
        add_player_event: Sender<PlayerEventWithToken>,
    ) -> Self {
        Self {
            creds,
            add_player_event,
        }
    }
}

pub async fn run(state: AppState, port: u16, cancel: CancellationToken) -> Result<()> {
    use axum::routing::post;
    use axum::Router;

    let app = Router::new()
    .route(
            "/api/forward_creds",
            post(|State(state): State<AppState>,  Json(payload): Json<ForwardCreds>| async move {
                tracing::debug!(?payload.key, ?payload.creds.username, ?payload.device_name, "got forwarded creds");
                let mut reg = state.creds.write().unwrap();
                match reg.insert(payload) {
                    true => StatusCode::OK,
                    false => StatusCode::CONFLICT,
                }
            }),
        ).route(
            "/api/player_events",
            post(handle_player_event),
        ).with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(cancel.cancelled())
        .await?;

    Ok(())
}

#[debug_handler]
async fn handle_player_event(
    State(state): State<AppState>,
    Query(params): Query<PeParams>,
    Json(payload): Json<protocol::PlayerEvent>,
) -> Result<StatusCode, AppError> {
    tracing::debug!(?params, ?payload, "got player event");
    state
        .add_player_event
        .send(PlayerEventWithToken {
            token: params.token.clone(),
            event: payload.clone(),
        })
        .await?;

    Ok(StatusCode::OK)
}

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
