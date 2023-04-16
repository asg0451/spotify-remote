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
    Result as SerenityResult,
};
use songbird::{
    input::{children_to_reader, Codec, Container, Input},
    SerenityInit,
};

use receiver::creds_registry::CredsRegistry;

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

#[derive(Debug, Parser)]
struct Options {
    #[clap(short, long, default_value = "8080")]
    grpc_port: u16,
    #[clap(short, long, default_value = "player")]
    player_path: String,
}

impl TypeMapKey for Options {
    type Value = Arc<RwLock<Options>>;
}

#[tokio::main]
async fn main() -> Result<()> {
    receiver::util::setup_logging()?;
    let _ = receiver::util::load_env(".env");

    let opts = Options::parse();

    let stream_registry = Arc::new(RwLock::new(CredsRegistry::default()));

    // start rpc server
    let rpc_server_jh = {
        let registry = Arc::clone(&stream_registry);
        tokio::spawn(async move {
            let srv = receiver::server::Server::new(registry);
            srv.run(opts.grpc_port).await?;
            Ok::<(), anyhow::Error>(())
        })
    };

    // Configure the client with your Discord bot token in the environment.
    let token = std::env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!"))
        .group(&GENERAL_GROUP)
        .after(after)
        .unrecognised_command(unrecognized_command);

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

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<CredsRegistry>(Arc::clone(&stream_registry));
        data.insert::<Options>(Arc::new(RwLock::new(opts)));
    }

    let disc_jh = tokio::spawn(async move { client.start().await });

    tokio::select! {
        _ = rpc_server_jh => {
            tracing::info!("grpc server exited");
        }
        _ = disc_jh => {
            tracing::info!("discord client exited");
        }
        _ = receiver::util::ctrl_c() => {
            tracing::info!("received ctrl-c");
        }
    };

    Ok(())
}

#[hook]
async fn after(ctx: &Context, msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => tracing::info!("Processed command '{}'", command_name),
        Err(why) => {
            // attempt to communicate error to user
            let _ = msg
                .reply(ctx, format!("command {} failed: {:?}", command_name, why))
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
    _play_spotify(ctx, msg, args).await.map_err(|e| {
        tracing::error!("play_spotify failed: {:?}", e);
        e
    })?;

    Ok(())
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
            check_msg(msg.reply(ctx, "Not in a voice channel").await);
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
        tracing::debug!(?registry);
        registry.take(&name)
    };

    if creds_req.is_none() {
        tracing::info!(?name, "no creds found");
        msg.reply(ctx, "no stream found").await?;
        return Ok(());
    }
    let creds_req = creds_req.unwrap();

    let player_path = {
        let data = ctx.data.read().await;
        let opts = data.get::<Options>().unwrap().read().unwrap();
        opts.player_path.clone()
    };
    tracing::debug!(?player_path, "starting player");
    let mut player_command = Command::new(player_path)
        .stderr(Stdio::inherit())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
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
    call_handler.enqueue_source(input);

    tracing::debug!(?name, "enqueued source");

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let voice_manager = songbird::get(ctx).await.unwrap().clone();

    if let Some(handler_lock) = voice_manager.get(guild.id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        queue.stop();

        check_msg(msg.channel_id.say(&ctx.http, "Queue cleared.").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    let voice_manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = voice_manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = voice_manager.remove(guild_id).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        tracing::warn!("Error sending message: {:?}", why);
    }
}
