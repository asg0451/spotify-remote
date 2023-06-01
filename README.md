# Spotify Remote Discord Bot (name tbd)

[![Build](https://github.com/asg0451/spotify-remote/actions/workflows/build.yaml/badge.svg)](https://github.com/asg0451/spotify-remote/actions/workflows/build.yaml)

This is a self-hosted Discord bot that allows you to stream to it as if it was a Spotify player on your local network.

## Getting started

1. Create a Discord app and bot.
1. Run the `receiver` Docker image either locally or on a server, such as via: `$ docker run -p8080:8080 -e DISCORD_TOKEN=<your-token> TODO_image_name`

    a. NOTE: if you end up exposing this service over the internet, it's strongly recommended to use https!
1. Invite the bot to your server. Make sure it has sufficient permissions to join voice channels, send messages, do slash commands, and read message contents.

    a. TODO: nail down which these are specifically
1. Run the `forwarder` binary from your local machine, and point it at the server you started, such as with `$ ./forwarder -a http://localhost:8080`

    a. The `forwarder` binary is available in the [latest github release](https://github.com/asg0451/spotify-remote/releases/latest). Currently binaries are built for Linux and Windows.
    a. TODO: make the remote address not default to coldcutz.net for public builds
1. Open Spotify and connect to the virtual device (the default name is `danube`)
1. `forwarder` Will connect to the server and output a connect code. In Discord, run `/play_spotify <code>` to finish the connection.
1. You should now be able to play music through the bot, using Spotify normally.
