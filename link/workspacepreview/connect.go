package workspacepreview

import (
	"context"
	"encoding/json"
	"net"
	"os"
	"time"

	"github.com/TheArctesian/anima/link/ownerpipe"
)

// Connect asks the root owner to validate and connect a leased workspace endpoint.
func Connect(ctx context.Context, request Open) (net.Conn, error) {
	if os.Getenv("ANIMA_MANAGED_PREVIEWS") != "1" || !request.Valid(time.Now()) {
		return nil, ErrRefused
	}
	frame, err := json.Marshal(request)
	if err != nil {
		return nil, ErrRefused
	}
	conn, err := ownerpipe.Connect(ctx, ownerpipe.Endpoint{
		Socket: "/run/anima-execution/preview.sock", Protocol: "workspace-preview/v1",
	}, frame)
	if err != nil {
		return nil, ErrRefused
	}
	return conn, nil
}
