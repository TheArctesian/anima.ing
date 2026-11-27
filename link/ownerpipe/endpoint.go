// Package ownerpipe connects enrolled transport to a root-owned execution socket.
package ownerpipe

import "errors"

// ErrRefused means the root execution owner did not accept the stream.
var ErrRefused = errors.New("execution owner unavailable")

// Endpoint is fixed by each adapter, never supplied by a remote request.
type Endpoint struct {
	Socket   string
	Protocol string
}
