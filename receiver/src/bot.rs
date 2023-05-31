use std::{
    process::{Command, Stdio},
    sync::{Arc, RwLock},
};

use anyhow::Result;
use clap::Parser;

use poise::serenity_prelude::GatewayIntents;
use songbird::input::{children_to_reader, Codec, Container, Input};
use songbird::SerenityInit;

use crate::creds_registry::CredsRegistry;

#[derive(Debug, Parser, Clone)]
pub struct BotOptions {
    #[clap(short, long, default_value = "player")]
    player_path: String,
    #[clap(short, long, env = "DISCORD_TOKEN")]
    discord_token: String,
}

#[derive(Debug)]
struct Data {
    bot_options: BotOptions,
    creds_registry: Arc<RwLock<CredsRegistry>>,
    currently_playing_pid: Arc<RwLock<Option<u32>>>,
}
type Error = anyhow::Error;
type Context<'a> = poise::Context<'a, Data, Error>;

// NOTE: Your bot also needs to be invited with the applications.commands scope. For example, in Discord’s invite link generator (discord.com/developers/applications/XXX/oauth2/url-generator), tick the applications.commands box.

pub async fn run_bot(opts: BotOptions, stream_registry: Arc<RwLock<CredsRegistry>>) -> Result<()> {
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

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![play_spotify(), leave(), stop(), restart()],
            on_error: |error| Box::pin(on_error(error)),
            ..Default::default()
        })
        .client_settings(|settings| settings.register_songbird())
        .token(&opts.discord_token)
        .intents(intents)
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    bot_options: opts,
                    creds_registry: stream_registry,
                    currently_playing_pid: Arc::new(RwLock::new(None)),
                })
            })
        });

    framework.run().await?;
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

    let mut player_command = Command::new(player_path)
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

    // TODO: send player a signal on stop, so it can shut down gracefully before it's Dropped

    let mut call_handler = call_handler_lock.lock().await;
    call_handler.play_source(input);

    tracing::debug!(?key, "playing source");
    ctx.say("playing..").await?;
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

// TODO: it looks like /stop sometimes isnt making the player stop. i see gstreamer and player still alive in htop. why?
// TODO: although, if you then actually play something on spotify, it hits the broken pipe -> exit(1)
#[poise::command(slash_command)]
async fn stop(ctx: Context<'_>) -> Result<()> {
    let guild = match ctx.guild() {
        None => {
            ctx.say("This command can only be used in a guild").await?;
            return Ok(());
        }
        Some(g) => g,
    };

    {
        let mut pid = ctx.data().currently_playing_pid.read().unwrap();
        if let Some(pid) = pid.as_ref().take() {
            let pid = Pid::from_raw(pid as i32);
            tracing::debug!(?pid, "asking player to stop");
            let _ = nix::sys::signal::kill(pid, Signal::SIGUSR1);

            // wait for it to exit, or timeout
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {},
                _ = nix::sys::wait::waitpid(pid, None) => {},
            }

        }
    }

    let voice_manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();
    let call_handler_lock = voice_manager.get(guild.id);
    if let Some(call_handler_lock) = call_handler_lock {
        let mut call_handler = call_handler_lock.lock().await;
        call_handler.stop();
        ctx.say("stopped playback").await?;
    }

    Ok(())
}

// HACK: there's a bug that makes the bot get into a state where it cant play anymore. so let users unblock themselves
#[poise::command(slash_command)]
async fn restart(_ctx: Context<'_>) -> Result<()> {
    std::process::exit(0);
}
