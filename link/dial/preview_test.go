package dial_test

import (
	"context"
	"errors"
	"sync/atomic"
	"testing"
	"time"

	"github.com/TheArctesian/anima/link/dial"
	"github.com/TheArctesian/anima/link/workspacepreview"
)

func TestUnavailableWorkspacePreviewNeverDialsTheHostPort(t *testing.T) {
	t.Setenv("ANIMA_MANAGED_PREVIEWS", "0")
	port, connections := echo(t)
	var hostPolicyCalls atomic.Int32
	b := newBench(t, func(uint16) bool {
		hostPolicyCalls.Add(1)
		return true
	})
	now := time.Now()
	opening := workspacepreview.Open{
		Binding: workspacepreview.Binding{
			Endpoint:   "11111111-1111-4111-8111-111111111111",
			Principal:  "22222222-2222-4222-8222-222222222222",
			Grant:      "33333333-3333-4333-8333-333333333333",
			Workspace:  "44444444-4444-4444-8444-444444444444",
			Generation: 2,
			Port:       uint16(port),
			ExpiresAt:  uint64(now.Add(time.Minute).Unix()),
		},
		ValidUntil: now.Add(10 * time.Second),
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	stream, release, err := b.desk.OpenPreview(ctx, theDevice, opening, b.ask)
	if release != nil {
		defer release()
	}
	if stream != nil {
		stream.Close()
		t.Fatal("unavailable workspace returned a stream")
	}
	if !errors.Is(err, dial.ErrRefused) {
		t.Fatalf("want workspace refusal, got %v", err)
	}
	if hostPolicyCalls.Load() != 0 || connections.Load() != 0 {
		t.Fatal("workspace preview consulted or connected to the host port")
	}
}
