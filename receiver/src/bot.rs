use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex, RwLock},
};

use anyhow::Result;
use clap::Parser;

use poise::serenity_prelude::GatewayIntents;
use songbird::input::{children_to_reader, Codec, Container, Input};
use songbird::SerenityInit;
use tokio_util::sync::CancellationToken;

use crate::{
    creds_registry::CredsRegistry,
    player_events_manager::{MsgHandle, PlayerEventWithToken, PlayerEventsManager},
};

#[derive(Debug, Parser, Clone)]
pub struct BotOptions {
    #[clap(short, long, default_value = "player")]
    player_path: String,
    #[clap(short, long, env = "DISCORD_TOKEN")]
    discord_token: String,
}

// User data, which is stored and accessible in all command invocations
struct Data {
    bot_options: BotOptions,
    creds_registry: Arc<RwLock<CredsRegistry>>,
    add_player_event_message: tokio::sync::mpsc::Sender<(String, MsgHandle)>,
}
type Error = anyhow::Error;
type Context<'a> = poise::Context<'a, Data, Error>;

// NOTE: Your bot also needs to be invited with the applications.commands scope. For example, in Discord’s invite link generator (discord.com/developers/applications/XXX/oauth2/url-generator), tick the applications.commands box.

pub async fn run_bot(
    opts: BotOptions,
    stream_registry: Arc<RwLock<CredsRegistry>>,
    recv_pe: tokio::sync::mpsc::Receiver<PlayerEventWithToken>,
    cancel: CancellationToken,
) -> Result<()> {
    // TODO: pare down
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_BANS
        | GatewayIntents::GUILD_EMOJIS_AND_STICKERS
        | GatewayIntents::GUILD_INTEGRATIONS
        | GatewayIntents::GUILD_WEBHOOKS
        | GatewayIntents::GUILD_INVITES
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::GUILD_MESSAGE_TYPING
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::DIRECT_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGE_TYPING;

    let pem_cancel = cancel.clone();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![play_spotify(), leave(), stop()],
            on_error: |error| Box::pin(on_error(error)),
            ..Default::default()
        })
        .client_settings(|settings| settings.register_songbird())
        .token(&opts.discord_token)
        .intents(intents)
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                // start player events mgr in here so we have access to a ctx for editing msgs
                let mut mgr = PlayerEventsManager::new(ctx.clone(), pem_cancel.clone(), recv_pe);
                let add_player_event_message = mgr.add_msg_id.clone();
                tokio::spawn(async move {
                    mgr.run().await.unwrap();
                });

                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    bot_options: opts,
                    creds_registry: stream_registry,
                    add_player_event_message,
                })
            })
        });

    tokio::select! {
        _ = cancel.cancelled() => {
            tracing::info!("Bot cancelled");
        }
        res = framework.run() => {
            res?;
        }
    };
    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx } => {
            tracing::warn!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                tracing::warn!("Error while handling error: {}", e)
            }
        }
    }
}

#[poise::command(slash_command)]
async fn play_spotify(ctx: Context<'_>, #[description = "Stream key"] key: String) -> Result<()> {
    let guild = match ctx.guild() {
        None => {
            ctx.say("This command can only be used in a guild").await?;
            return Ok(());
        }
        Some(g) => g,
    };

    let voice_manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();
    let channel_id = guild
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            ctx.say("Not in a voice channel").await?;
            return Ok(());
        }
    };
    let (call_handler_lock, res) = voice_manager.join(guild.id, connect_to).await;
    res?;

    let creds_req = {
        let mut registry = ctx.data().creds_registry.write().unwrap();
        registry.take(&key)
    };

    let creds_req = match creds_req {
        Some(creds) => creds,
        None => {
            ctx.say(format!("No stream found for {key}")).await?;
            return Ok(());
        }
    };

    let player_path = ctx.data().bot_options.player_path.clone();
    tracing::debug!(?player_path, "starting player");

    let pe_token = gen_token();
    let mut player_command = Command::new(player_path)
        .args(["--player-updates-token", &pe_token]) // TODO: pe addr too
        .stderr(Stdio::inherit())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .env("TOKIO_CONSOLE_BIND", "0.0.0.0:0") // so multiple streams don't conflict
        .spawn()?;
    let mut player_stdin = player_command.stdin.take().unwrap();
    serde_json::to_writer(&mut player_stdin, &creds_req.creds)?;

    // spotify streams at 44.1khz, we want 48khz, so use gstreamer to resample it.
    let gstreamer_command = Command::new("gst-launch-1.0")
        .args([
            "filesrc",
            "location=/dev/stdin",
            "!",
            "rawaudioparse",
            "use-sink-caps=false",
            "format=pcm",
            "pcm-format=s16le",
            "sample-rate=44100",
            "num-channels=2",
            "!",
            "audioconvert",
            "!",
            "audioresample",
            "!",
            "audio/x-raw,",
            "rate=48000",
            "!",
            "filesink",
            "location=/dev/stdout",
        ])
        .stderr(Stdio::inherit())
        .stdin(player_command.stdout.take().unwrap())
        .stdout(Stdio::piped())
        .spawn()?;

    tracing::debug!(?key, "started player processes");

    let reader = children_to_reader::<i16>(vec![player_command, gstreamer_command]);

    let input = Input::new(true, reader, Codec::Pcm, Container::Raw, None);

    let mut call_handler = call_handler_lock.lock().await;
    call_handler.play_source(input);

    tracing::debug!(?key, "playing source");
    let status_reply = ctx.say("playing..").await?;
    let msg = status_reply.message().await?;

    ctx.data()
        .add_player_event_message
        .send((
            pe_token,
            MsgHandle {
                channel_id: msg.channel_id.0,
                msg_id: msg.id.0,
            },
        ))
        .await?;

    Ok(())
}

#[poise::command(slash_command)]
async fn leave(ctx: Context<'_>) -> Result<()> {
    let guild = match ctx.guild() {
        None => {
            ctx.say("This command can only be used in a guild").await?;
            return Ok(());
        }
        Some(g) => g,
    };

    let voice_manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();
    let call_handler_lock = voice_manager.get(guild.id);
    if let Some(call_handler_lock) = call_handler_lock {
        let mut call_handler = call_handler_lock.lock().await;
        call_handler.leave().await?;
        ctx.say("left").await?;
    }

    Ok(())
}

#[poise::command(slash_command)]
async fn stop(ctx: Context<'_>) -> Result<()> {
    let guild = match ctx.guild() {
        None => {
            ctx.say("This command can only be used in a guild").await?;
            return Ok(());
        }
        Some(g) => g,
    };
    let voice_manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();
    let call_handler_lock = voice_manager.get(guild.id);
    if let Some(call_handler_lock) = call_handler_lock {
        let mut call_handler = call_handler_lock.lock().await;
        call_handler.stop();
        ctx.say("stopped playback").await?;
    }

    Ok(())
}

fn gen_token() -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut token = [0u8; 32];
    rng.fill_bytes(&mut token);
    base64::engine::general_purpose::STANDARD.encode(&token)
}
