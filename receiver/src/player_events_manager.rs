use std::collections::HashMap;

use anyhow::Result;
use poise::serenity_prelude::Context;
use tokio::sync::mpsc::{self, Receiver, Sender};

use protocol::{PlayerEvent, TrackInfo};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct PlayerEventWithToken {
    pub token: String,
    pub event: PlayerEvent,
}

#[derive(Debug, Clone)]
pub struct MsgHandle {
    pub msg_id: u64,
    pub channel_id: u64,
}

pub struct PlayerEventsManager {
    ctx: Context,
    recv_event: Receiver<PlayerEventWithToken>,
    msgs: HashMap<String, MsgHandle>, // map of token -> msg_id
    pub add_msg_id: Sender<(String, MsgHandle)>,
    recv_msg_id: Receiver<(String, MsgHandle)>,
    cancel: CancellationToken,
}

impl PlayerEventsManager {
    pub fn new(
        ctx: Context,
        cancel: CancellationToken,
        recv_event: Receiver<PlayerEventWithToken>,
    ) -> Self {
        let (add_msg_id, recv_msg_id) = mpsc::channel(1);
        Self {
            ctx,
            recv_event,
            msgs: Default::default(),
            add_msg_id,
            recv_msg_id,
            cancel,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                Some((token, msg_id)) = self.recv_msg_id.recv() => {
                    self.msgs.insert(token, msg_id);
                }
                Some(event) = self.recv_event.recv() => {
                    self.handle_event(event).await?;
                }
                _ = self.cancel.cancelled() => break,
            }
        }
        Ok(())
    }

    async fn handle_event(&self, event: PlayerEventWithToken) -> Result<()> {
        tracing::debug!(?event, "got event");
        // TODO: ensure we have an edit msg id before the first event comes in
        let msg_handle = match self.msgs.get(&event.token).cloned() {
            Some(h) => h,
            None => {
                tracing::warn!(?event, "no msg handle found");
                return Ok(());
            }
        };
        let content = event_content(event.event);
        // TODO: is this right?
        self.ctx
            .http
            .edit_message(
                msg_handle.channel_id,
                msg_handle.msg_id,
                &serde_json::json!({ "content": content }),
            )
            .await?;
        Ok(())
    }
}

fn event_content(event: PlayerEvent) -> String {
    match event {
        PlayerEvent::Playing(info) => playing_content(info),
        PlayerEvent::Paused(info) => paused_content(info),
        PlayerEvent::Stopped => "⏹️".to_string(),
    }
}

fn playing_content(info: TrackInfo) -> String {
    format!(
        "▶️ {} - {} - {}",
        info.name,
        info.artists.join(", "),
        info.album
    )
}

fn paused_content(info: TrackInfo) -> String {
    format!(
        "⏸️ {} - {} - {}",
        info.name,
        info.artists.join(", "),
        info.album
    )
}
