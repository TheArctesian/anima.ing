package dial

import (
	"context"
	"io"
	"net"
	"strconv"
	"time"

	"github.com/TheArctesian/anima/link/tunnel"
)

const localLimit = 5 * time.Second

// Local is the device side: what a device needs to answer a request.
type Local struct {
	// IsOpen reports whether a port is open on this computer (HMD-049); nil means none is.
	IsOpen func(port uint16) bool
	Hub    func(ctx context.Context) (net.Conn, error)
}

// Give answers one hub request, dialling the loopback port only when it is open.
func (l Local) Give(ctx context.Context, request Request) {
	word, local := l.connect(ctx, request.Port)
	hub, err := l.Hub(ctx)
	if err != nil {
		closeIfOpen(local)
		return
	}
	_ = hub.SetWriteDeadline(time.Now().Add(lineLimit))
	_, err = io.WriteString(hub, answerLine(request.id, word))
	_ = hub.SetWriteDeadline(time.Time{})
	if err != nil || local == nil {
		hub.Close()
		closeIfOpen(local)
		return
	}
	tunnel.Join(local, hub)
}

func (l Local) connect(ctx context.Context, port uint16) (string, net.Conn) {
	if l.IsOpen == nil || !l.IsOpen(port) {
		return WordRefused, nil
	}
	ctx, cancel := context.WithTimeout(ctx, localLimit)
	defer cancel()
	var dialer net.Dialer
	local, err := dialer.DialContext(ctx, "tcp", net.JoinHostPort("127.0.0.1", strconv.Itoa(int(port))))
	if err != nil {
		return WordClosed, nil
	}
	return WordOK, local
}

func closeIfOpen(local net.Conn) {
	if local != nil {
		local.Close()
	}
}
