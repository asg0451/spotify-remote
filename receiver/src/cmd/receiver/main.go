package main

import (
	"context"
	"fmt"
	"net"
	"net/http"
	_ "net/http/pprof"
	"os"
	"os/signal"
	"strings"
	"syscall"

	"coldcutz.net/spotify-remote/src/cmd/receiver/server"
	"coldcutz.net/spotify-remote/src/logging"
	"coldcutz.net/spotify-remote/src/pb"
	"coldcutz.net/spotify-remote/src/utils"
	"google.golang.org/grpc"
	"google.golang.org/grpc/reflection"

	"github.com/bwmarrin/discordgo"
	"github.com/jessevdk/go-flags"
)

// TODO: how to integrate with djcc properly?
// might move faster as a standalone thing.. serenity et al is kind of bloated
// on the other hand if we cant get audio working here then just do it there

// but how do we expose the grpc server? grpc tls tailscale - https://seankhliao.com/blog/12023-04-09-tailscale-funnel-secure-grpc/
//   better in go

// also, how do we package up the transmitter in the best way? docker might be too much friction
// maybe it has to be rust?

// probably the BEST way is to split the librespot binary in half and run the credentials and control bit on the transmitter
// and the rest on the receiver. but that sounds hard...

// middle ground:
// - transmitter in rust, wrapping librespot LIBRARY, not binary, streaming data over grpc
// - receiver in go, using tailscale LIBRARY to set up a funnel and all that, getting data from transmitter over grpc (tls),
//   and being a discord bot too (?)

type Opts struct {
	GrpcPort      int    `short:"p" long:"port" description:"Port for grpc" default:"8080"`
	PprofPort     int    `long:"pprof-port" description:"Port for pprof" default:"6060"`
	LibrespotPath string `short:"l" long:"librespot" description:"Path to librespot binary" required:"true"`
}

func main() {
	var opts Opts
	_, err := flags.Parse(&opts)
	if err != nil {
		os.Exit(1)
	}

	if err = utils.DotEnv(); err != nil {
		panic(err)
	}

	discordToken := os.Getenv("DISCORD_TOKEN")
	if discordToken == "" {
		panic("no discord token in DISCORD_TOKEN env var")
	}

	log := logging.New()
	ctx, cancel := context.WithCancel(logging.NewContext(context.Background(), log))

	// pprof
	go func() {
		log.Info("pprof listening", "port", opts.PprofPort)
		log.Error("pprof error", "err", http.ListenAndServe(fmt.Sprintf(":%d", opts.PprofPort), nil))
	}()

	// start the server
	srv := server.New(log)

	s := grpc.NewServer(grpc.UnaryInterceptor(utils.MakeLoggingInterceptor(log)), grpc.StreamInterceptor(utils.MakeLoggingStreamInterceptor(log)))
	pb.RegisterSpotifyRemoteServer(s, srv)
	reflection.Register(s)

	lis, err := net.Listen("tcp", fmt.Sprintf(":%d", opts.GrpcPort))
	if err != nil {
		panic(fmt.Sprintf("failed to listen: %v", err))
	}

	go func() {
		log.Info("grpc listening", "port", opts.GrpcPort)
		if err := s.Serve(lis); err != nil {
			panic(fmt.Sprintf("failed to serve: %v", err))
		}
	}()

	// signal handling
	go func() {
		term := make(chan os.Signal, 1)
		signal.Notify(term, syscall.SIGTERM, syscall.SIGINT)
		<-term
		cancel()
		s.GracefulStop()
	}()

	// discord shit. TODO: should make a guy to manage this
	// https://github.com/bwmarrin/discordgo/blob/master/examples/airhorn/main.go
	// TODO: slash commands as in https://github.com/bwmarrin/discordgo/blob/master/examples/slash_commands/main.go
	dg, err := discordgo.New("Bot " + discordToken)
	if err != nil {
		panic(fmt.Sprintf("error creating Discord session: %v", err))
	}

	// Set the playing status on ready
	dg.AddHandler(func(s *discordgo.Session, _ *discordgo.Ready) {
		if err := s.UpdateGameStatus(0, "spotify, remotely"); err != nil {
			panic(fmt.Sprintf("error updating status: %v", err))
		}
	})

	// on new msg, do stuff
	dg.AddHandler(func(s *discordgo.Session, m *discordgo.MessageCreate) {
		// Ignore all messages created by the bot itself. not sure if this is needed
		if m.Author.ID == s.State.User.ID {
			return
		}

		log.Debug("message received", "msg", m.Content)

		if strings.HasPrefix(m.Content, "!sp") {
			// get the id of the stream
			parts := strings.Split(m.Content, " ")
			if len(parts) != 2 {
				if _, err := s.ChannelMessageSendReply(m.ChannelID, "USAGE: !sp my-stream-id", m.Reference()); err != nil {
					log.Error("error sending message", "err", err)
				}
			}
			id := parts[1]
			sink := srv.GetSink(id)

			log.Debug("got sink", "id", id)

			// Find the channel that the message came from.
			c, err := s.State.Channel(m.ChannelID)
			if err != nil {
				panic(fmt.Sprintf("could not find channel %q: %v", m.ChannelID, err))
			}

			// Find the guild for that channel.
			g, err := s.State.Guild(c.GuildID)
			if err != nil {
				panic(fmt.Sprintf("could not find guild %q: %v", c.GuildID, err))
			}

			var cid, gid string
			// Look for the message sender in that guild's current voice states.
			for _, vs := range g.VoiceStates {
				if vs.UserID == m.Author.ID {
					cid = vs.ChannelID
					gid = g.ID
					break
				}
			}
			if cid == "" || gid == "" {
				if _, err := s.ChannelMessageSendReply(m.ChannelID, "You must be in a voice channel to use this command.", m.Reference()); err != nil {
					log.Error("error sending message", "err", err)
				}
				return
			}

			// stream the audio in
			// TODO: abstract, cleanup, cancellation, etc
			log.Info("joining voice channel", "guild", gid, "channel", cid)
			vc, err := s.ChannelVoiceJoin(gid, cid, false, true)
			if err != nil {
				// https://github.com/bwmarrin/discordgo/issues/829 :(
				if _, ok := s.VoiceConnections[gid]; ok {
					vc = s.VoiceConnections[gid]
				} else {
					panic(fmt.Sprintf("error joining voice channel: %v", err))
				}
			}
			if err = vc.Speaking(true); err != nil {
				panic(fmt.Sprintf("error setting speaking: %v", err))
			}

			log.Debug("streaming...")
			for buf := range sink {
				// log.Debug("sending buffer", "len", len(buf))
				vc.OpusSend <- buf
			}

			log.Debug("stream ended")

			if err = vc.Speaking(false); err != nil {
				panic(fmt.Sprintf("error setting speaking: %v", err))
			}
			if err = vc.Disconnect(); err != nil {
				panic(fmt.Sprintf("error disconnecting: %v", err))
			}
		}
	})

	dg.Identify.Intents = discordgo.IntentsGuilds | discordgo.IntentsGuildMessages | discordgo.IntentsGuildVoiceStates |
		discordgo.IntentMessageContent | discordgo.IntentGuildVoiceStates

	log.Debug("receiver starting")

	err = dg.Open()
	if err != nil {
		fmt.Println("Error opening Discord session: ", err)
	}
	defer dg.Close()

	<-ctx.Done()

	// Cleanly close down the Discord session.
}
