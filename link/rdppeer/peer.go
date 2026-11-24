// Package rdppeer dials only the signed Apple Silicon host carried inside the desktop app (MRDP-004).
package rdppeer

import (
	"context"
	"errors"
	"net"

	"github.com/TheArctesian/anima/link/statedir"
)

// ErrHost means no live bundled host could be authenticated.
var ErrHost = errors.New("rdppeer: bundled host could not be authenticated")

// Connect admits the host on the connected socket before returning a stream to Link.
func Connect(ctx context.Context, dir *statedir.Dir) (net.Conn, error) {
	if !Supported {
		return nil, ErrHost
	}
	stream, err := dir.DialOwned(ctx, "rdp.sock")
	if err != nil {
		return nil, ErrHost
	}
	if !admit(stream) || ctx.Err() != nil {
		stream.Close()
		return nil, ErrHost
	}
	return stream, nil
}
