//go:build !linux

package ownerpipe

import (
	"context"
	"net"
)

// Connect refuses the execution-owner handshake on platforms without its Linux socket.
func Connect(context.Context, Endpoint, []byte) (net.Conn, error) { return nil, ErrRefused }
