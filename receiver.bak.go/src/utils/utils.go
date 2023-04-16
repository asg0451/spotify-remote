package utils

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	"golang.org/x/exp/slog"
	"google.golang.org/grpc"
)

func DotEnv() error {
	cnt, err := os.ReadFile(".env")
	if err != nil {
		if !os.IsNotExist(err) {
			return fmt.Errorf("failed to read .env: %w", err)
		}
		return nil
	}
	for _, line := range strings.Split(string(cnt), "\n") {
		if strings.HasPrefix(line, "#") {
			continue
		}
		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 {
			continue
		}
		os.Setenv(parts[0], parts[1])
	}
	return nil
}

func MakeLoggingInterceptor(log *slog.Logger) grpc.UnaryServerInterceptor {
	return func(ctx context.Context, req interface{}, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (interface{}, error) {
		log.Debug("handling request", "method", info.FullMethod, "request", req)
		start := time.Now()
		resp, err := handler(ctx, req)
		if err != nil {
			log.Warn("request failed", "method", info.FullMethod, "request", req, "error", err)
		} else {
			dur := time.Since(start)
			log.Debug("request succeeded", "method", info.FullMethod, "request", req, "response", resp, "duration", dur)
		}
		return resp, err
	}
}

func MakeLoggingStreamInterceptor(log *slog.Logger) grpc.StreamServerInterceptor {
	return func(srv interface{}, ss grpc.ServerStream, info *grpc.StreamServerInfo, handler grpc.StreamHandler) error {
		log.Debug("handling stream request", "method", info.FullMethod)
		start := time.Now()
		err := handler(srv, ss)
		if err != nil {
			log.Warn("stream request failed", "method", info.FullMethod, "error", err)
		} else {
			dur := time.Since(start)
			log.Debug("stream request finished", "method", info.FullMethod, "duration", dur)
		}
		return err
	}
}
