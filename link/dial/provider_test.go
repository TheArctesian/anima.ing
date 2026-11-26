package dial_test

import (
	"context"
	"errors"
	"sync/atomic"
	"testing"
	"time"

	"github.com/TheArctesian/anima/link/dial"
	"github.com/TheArctesian/anima/link/workspaceauth"
)

func TestUnavailableProviderLoginNeverConsultsHostPorts(t *testing.T) {
	t.Setenv("ANIMA_MANAGED_PROVIDER_LOGIN", "0")
	var hostPolicyCalls atomic.Int32
	b := newBench(t, func(uint16) bool {
		hostPolicyCalls.Add(1)
		return true
	})
	now := time.Now()
	login := workspaceauth.Login{
		Session:    "11111111-1111-4111-8111-111111111111",
		Principal:  "22222222-2222-4222-8222-222222222222",
		Workspace:  "33333333-3333-4333-8333-333333333333",
		Generation: 2,
		Provider:   "codex",
		ValidUntil: now.Add(10 * time.Second),
		ExpiresAt:  now.Add(time.Minute),
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	stream, release, err := b.desk.OpenProvider(ctx, theDevice, login, b.ask)
	if release != nil {
		defer release()
	}
	if stream != nil {
		stream.Close()
		t.Fatal("disabled sign-in returned a stream")
	}
	if !errors.Is(err, dial.ErrRefused) {
		t.Fatalf("want provider refusal, got %v", err)
	}
	if hostPolicyCalls.Load() != 0 {
		t.Fatal("provider sign-in consulted host port policy")
	}
}
