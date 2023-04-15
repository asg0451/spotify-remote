package main

import (
	"context"
	"fmt"
	"net/http"
	_ "net/http/pprof"
	"os"
	"os/signal"
	"strings"
	"syscall"

	"coldcutz.net/spotify-remote/src/cmd/receiver/server"
	"coldcutz.net/spotify-remote/src/logging"

	"github.com/bwmarrin/discordgo"
	"github.com/jessevdk/go-flags"
)

type Opts struct {
	Name            string `short:"n" long:"name" description:"Name of transmitter" default:"transmitter"`
	ReceiverAddr    string `short:"r" long:"receiver" description:"Address of receiver" required:"true"`
	PprofPort       int    `short:"p" long:"pprof" description:"Port for pprof" default:"6060"`
	LibrespotPath   string `short:"l" long:"librespot" description:"Path to librespot binary" required:"true"`
	DiscordBotToken string `short:"d" long:"discord" description:"Discord bot token" required:"true"`
}

func main() {
	var opts Opts
	_, err := flags.Parse(&opts)
	if err != nil {
		os.Exit(1)
	}

	log := logging.New()
	ctx, cancel := context.WithCancel(logging.NewContext(context.Background(), log))

	// signal handling
	go func() {
		term := make(chan os.Signal, 1)
		signal.Notify(term, syscall.SIGTERM, syscall.SIGINT)
		<-term
		cancel()
	}()

	// pprof
	go func() {
		log.Info("pprof listening", "port", opts.PprofPort)
		log.Error("pprof error", "err", http.ListenAndServe(fmt.Sprintf(":%d", opts.PprofPort), nil))
	}()

	// start the server
	srv := server.New(log)

	// discord shit. TODO: should make a guy to manage this
	// https://github.com/bwmarrin/discordgo/blob/master/examples/airhorn/main.go
	dg, err := discordgo.New("Bot " + opts.DiscordBotToken)
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

			// stream the audio in
			// TODO: abstract, cleanup, cancellation, etc
			vc, err := s.ChannelVoiceJoin(m.GuildID, m.ChannelID, false, true)
			if err != nil {
				panic(fmt.Sprintf("error joining voice channel: %v", err))
			}
			if err = vc.Speaking(true); err != nil {
				panic(fmt.Sprintf("error setting speaking: %v", err))
			}

			for buf := range sink {
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

	dg.Identify.Intents = discordgo.IntentsGuilds | discordgo.IntentsGuildMessages | discordgo.IntentsGuildVoiceStates

	log.Debug("receiver starting")

	err = dg.Open()
	if err != nil {
		fmt.Println("Error opening Discord session: ", err)
	}
	defer dg.Close()

	<-ctx.Done()

	// Cleanly close down the Discord session.
}
