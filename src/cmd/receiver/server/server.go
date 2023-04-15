package server

import (
	"errors"
	"fmt"
	"io"
	"sync"

	"coldcutz.net/spotify-remote/src/pb"
	"golang.org/x/exp/slog"
)

type Server struct {
	pb.UnsafeSpotifyRemoteServer
	log    *slog.Logger
	sinks  map[string]chan []byte
	sinkMu *sync.Mutex
}

func New(log *slog.Logger) *Server {
	return &Server{
		log:    log,
		sinks:  make(map[string]chan []byte),
		sinkMu: &sync.Mutex{},
	}
}

// TODO: streams shouldnt be able to overwrite each other?
func (s *Server) SendAudio(stream pb.SpotifyRemote_SendAudioServer) error {
	for {
		audio, err := stream.Recv()
		if err != nil {
			if errors.Is(err, io.EOF) {
				s.DeleteSink(audio.Id)
				return stream.SendAndClose(&pb.SendAudioResponse{})
			}
			return fmt.Errorf("failed to receive audio: %w", err)
		}
		s.log.Debug("received audio", "len", len(audio.Data))

		sink := s.GetSink(audio.Id)
		select {
		case sink <- audio.Data:
		default: // /dev/null
		}
	}
}

func (s *Server) GetSink(id string) chan []byte {
	s.sinkMu.Lock()
	defer s.sinkMu.Unlock()

	sink, ok := s.sinks[id]
	if !ok {
		sink = make(chan []byte)
		s.sinks[id] = sink
	}
	return sink
}

func (s *Server) DeleteSink(id string) {
	s.sinkMu.Lock()
	defer s.sinkMu.Unlock()

	close(s.sinks[id])
	delete(s.sinks, id)
}

var _ pb.SpotifyRemoteServer = (*Server)(nil)
