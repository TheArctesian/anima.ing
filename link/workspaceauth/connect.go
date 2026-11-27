package workspaceauth

import (
	"context"
	"encoding/json"
	"net"
	"os"
	"time"

	"github.com/TheArctesian/anima/link/ownerpipe"
)

// Connect asks the root owner to start the fixed provider sign-in worker.
func Connect(ctx context.Context, request Login) (net.Conn, error) {
	if os.Getenv("ANIMA_MANAGED_PROVIDER_LOGIN") != "1" || !request.Valid(time.Now()) {
		return nil, ErrRefused
	}
	frame, err := json.Marshal(request)
	if err != nil {
		return nil, ErrRefused
	}
	conn, err := ownerpipe.Connect(ctx, ownerpipe.Endpoint{
		Socket: "/run/anima-execution/provider.sock", Protocol: "workspace-provider/v1",
	}, frame)
	if err != nil {
		return nil, ErrRefused
	}
	return conn, nil
}
