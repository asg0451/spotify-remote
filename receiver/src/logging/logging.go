package logging

import (
	"context"
	"os"

	"golang.org/x/exp/slog"
)

type loggerKey string

const key loggerKey = "logger"

func New() *slog.Logger {
	ho := slog.HandlerOptions{
		Level: slog.LevelDebug,
	}
	textHandler := ho.NewTextHandler(os.Stderr)
	return slog.New(textHandler)
}

func NewContext(ctx context.Context, logger *slog.Logger) context.Context {
	return context.WithValue(ctx, key, logger)
}

func FromContext(ctx context.Context) *slog.Logger {
	l, ok := ctx.Value(key).(*slog.Logger)
	if !ok {
		return slog.Default()
	}
	return l
}
