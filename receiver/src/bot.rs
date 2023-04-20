use std::{
    process::{Command, Stdio},
    sync::{Arc, RwLock},
};

use anyhow::Result;

use clap::Parser;
use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::{
        standard::{
            macros::{command, group, hook},
            Args, CommandResult,
        },
        StandardFramework,
    },
    model::{channel::Message, gateway::Ready},
    prelude::{GatewayIntents, TypeMapKey},
};
use songbird::{
    input::{children_to_reader, Codec, Container, Input},
    SerenityInit,
};

use crate::creds_registry::CredsRegistry;

#[derive(Debug, Parser, Clone)]
pub struct BotOptions {
    #[clap(short, long, default_value = "player")]
    player_path: String,
    #[clap(short, long, env = "DISCORD_TOKEN")]
    discord_token: String,
}

impl TypeMapKey for BotOptions {
    type Value = Arc<RwLock<BotOptions>>;
}

struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        tracing::info!(?ready.user.name, "connected!");
    }
}

#[group]
#[commands(kys, play_spotify, stop, leave)]
struct General;

pub struct Bot {
    client: Client,
}

impl Bot {
    pub async fn new(
        opts: BotOptions,
        stream_registry: Arc<RwLock<CredsRegistry>>,
    ) -> Result<Self> {
        let framework = StandardFramework::new()
            .configure(|c| c.prefix("!"))
            .group(&GENERAL_GROUP)
            .after(after)
            .unrecognised_command(unrecognized_command);

        // TODO: pare down
        let intents = GatewayIntents::default()
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

        let client = Client::builder(&opts.discord_token, intents)
            .event_handler(Handler)
            .framework(framework)
            .register_songbird()
            .await
            .expect("Err creating client");

        {
            let mut data = client.data.write().await;
            data.insert::<CredsRegistry>(Arc::clone(&stream_registry));
            data.insert::<BotOptions>(Arc::new(RwLock::new(opts)));
        }

        Ok(Self { client })
    }

    pub async fn run(mut self) -> Result<()> {
        self.client.start().await?;
        Ok(())
    }
}

#[hook]
async fn after(ctx: &Context, msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => tracing::info!("Processed command '{}'", command_name),
        Err(why) => {
            // attempt to communicate error to user
            let _ = msg
                .reply(
                    ctx,
                    format!(
                        "command {} failed. maybe try !leave, !stop, or !kys?",
                        command_name
                    ),
                )
                .await;
            tracing::info!("Command '{}' returned error {:?}", command_name, why)
        }
    }
}

#[hook]
async fn unrecognized_command(ctx: &Context, msg: &Message, command_name: &str) {
    tracing::info!("unknown command '{}' ", command_name);
    let _ = msg
        .reply(ctx, format!("unknown command '{}' ", command_name))
        .await;
}

#[command]
#[only_in(guilds)]
async fn kys(_: &Context, _: &Message, _: Args) -> CommandResult {
    // this probably isnt a great idea lol
    std::process::exit(0);
}

#[command]
#[only_in(guilds)]
#[aliases(ps)]
async fn play_spotify(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    match _play_spotify(ctx, msg, args.clone()).await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!("play_spotify failed: {:?}", e);

            if e.to_string()
                .to_ascii_lowercase()
                .contains("gateway response from discord timed out")
            {
                tracing::info!("leaving and retrying play_spotify");
                leave(ctx, msg, args.clone()).await?;
                return Ok(_play_spotify(ctx, msg, args).await?);
            }
            Err(e.into())
        }
    }
}

async fn _play_spotify(ctx: &Context, msg: &Message, args: Args) -> Result<()> {
    // queue up a new input or something
    let guild = msg.guild(&ctx.cache).unwrap();
    let voice_manager = songbird::get(ctx).await.unwrap().clone();

    // try and join even if joined already
    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            msg.reply(ctx, "Not in a voice channel").await?;
            return Ok(());
        }
    };
    let (call_handler_lock, res) = voice_manager.join(guild.id, connect_to).await;
    res?;

    // get the creds
    let name = args.message();
    let creds_req = {
        let data = ctx.data.read().await;
        let mut registry = data.get::<CredsRegistry>().unwrap().write().unwrap();
        registry.take(name)
    };

    if creds_req.is_none() {
        tracing::info!(?name, "no creds found");
        msg.reply(ctx, "no stream found").await?;
        return Ok(());
    }
    let creds_req = creds_req.unwrap();

    let player_path = {
        let data = ctx.data.read().await;
        let opts = data.get::<BotOptions>().unwrap().read().unwrap();
        opts.player_path.clone()
    };
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

    tracing::debug!(?name, "started player processes");

    let reader = children_to_reader::<i16>(vec![player_command, gstreamer_command]);

    let input = Input::new(true, reader, Codec::Pcm, Container::Raw, None);

    let mut call_handler = call_handler_lock.lock().await;
    call_handler.play_source(input);

    tracing::debug!(?name, "enqueued source");

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let voice_manager = songbird::get(ctx).await.unwrap().clone();

    if let Some(handler_lock) = voice_manager.get(guild.id) {
        let mut handler = handler_lock.lock().await;
        handler.stop();
        msg.reply(ctx, "Queue cleared.").await?;
    } else {
        msg.reply(ctx, "Not in a voice channel to play in").await?;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let voice_manager = songbird::get(ctx).await.unwrap().clone();
    let has_handler = voice_manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = voice_manager.remove(guild_id).await {
            msg.reply(ctx, format!("Failed: {:?}", e)).await?;
        }
        msg.reply(ctx, "Left voice channel").await?;
    } else {
        msg.reply(ctx, "Not in a voice channel").await?;
    }

    Ok(())
}
