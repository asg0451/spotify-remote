package main

import (
	"context"
	"fmt"
	"net/http"
	_ "net/http/pprof"
	"os"
	"os/exec"
	"os/signal"
	"syscall"
	"time"

	"coldcutz.net/spotify-remote/src/logging"
	"coldcutz.net/spotify-remote/src/pb"
	"github.com/jessevdk/go-flags"
	"google.golang.org/grpc"
)

type Opts struct {
	Name          string `short:"n" long:"name" description:"Name of transmitter" default:"transmitter"`
	ReceiverAddr  string `short:"r" long:"receiver" description:"Address of receiver" required:"true"`
	PprofPort     int    `short:"p" long:"pprof" description:"Port for pprof" default:"6060"`
	LibrespotPath string `short:"l" long:"librespot" description:"Path to librespot binary" required:"true"`
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

	log.Debug("transmitter starting")

	dialCtx, dialCancel := context.WithTimeout(ctx, 5*time.Second)
	defer dialCancel()
	conn, err := grpc.DialContext(dialCtx, opts.ReceiverAddr, grpc.WithBlock(), grpc.WithInsecure())
	if err != nil {
		panic(fmt.Sprintf("failed to dial receiver: %v", err))
	}
	defer conn.Close()

	client := pb.NewSpotifyRemoteClient(conn)

	// start the librespot binary
	cmd := exec.CommandContext(ctx, opts.LibrespotPath,
		"--name", opts.Name, "--disable-audio-cache", // TODO: why disable cache?
		"--enable-volume-normalization", "--backend", "pipe",
	)

	out, err := cmd.StdoutPipe()
	if err != nil {
		panic(fmt.Sprintf("failed to get stdout pipe: %v", err))
	}

	if err = cmd.Start(); err != nil {
		panic(fmt.Sprintf("failed to start librespot: %v", err))
	}

	// not supposed to call cmd.Wait before finished reading. TODO: think more carefully about exiting..

	// start streaming
	stream, err := client.SendAudio(ctx)
	if err != nil {
		panic(fmt.Sprintf("failed to start audio stream: %v", err))
	}

	// TODO: resample -- spotify streams at 44.1khz, we want 48khz, so use gstreamer to resample it.
	// let gstreamer_command = Command::new("gst-launch-1.0")
	// .args([
	// 	"filesrc",
	// 	"location=/dev/stdin",
	// 	"!",
	// 	"rawaudioparse",
	// 	"use-sink-caps=false",
	// 	"format=pcm",
	// 	"pcm-format=s16le",
	// 	"sample-rate=44100",
	// 	"num-channels=2",
	// 	"!",
	// 	"audioconvert",
	// 	"!",
	// 	"audioresample",
	// 	"!",
	// 	"audio/x-raw,",
	// 	"rate=48000",
	// 	"!",
	// 	"filesink",
	// 	"location=/dev/stdout",

	// batch up audio data and send it to the receiver
	buf := make([]byte, 1024)
	for {
		n, err := out.Read(buf)
		if err != nil {
			panic(fmt.Sprintf("failed to read from librespot: %v", err))
		}

		if err := stream.Send(&pb.AudioChunk{Id: opts.Name, Data: buf[:n]}); err != nil {
			panic(fmt.Sprintf("failed to send audio chunk: %v", err))
		}
	}
}
