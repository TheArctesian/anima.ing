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
const revokePoll = 250 * time.Millisecond

// Local is the device side: what a device needs to answer a request.
type Local struct {
	// IsOpen reports whether a port is open on this computer (HMD-049); nil means none is.
	IsOpen func(port uint16) bool
	Hub    func(ctx context.Context) (net.Conn, error)
	// RDP dials the bundled host on systems where port 3389 has a dedicated socket.
	RDP func(time.Duration) (net.Conn, error)
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
	if request.Port == 3389 && l.RDP != nil {
		quit := make(chan struct{})
		defer close(quit)
		go l.watchRDP(ctx, quit, local)
	}
	tunnel.Join(local, hub)
}

func (l Local) watchRDP(ctx context.Context, quit <-chan struct{}, local net.Conn) {
	ticker := time.NewTicker(revokePoll)
	defer ticker.Stop()
	for {
		select {
		case <-quit:
			return
		case <-ctx.Done():
			local.Close()
			return
		case <-ticker.C:
			if !l.IsOpen(3389) {
				local.Close()
				return
			}
		}
	}
}

func (l Local) connect(ctx context.Context, port uint16) (string, net.Conn) {
	if l.IsOpen == nil || !l.IsOpen(port) {
		return WordRefused, nil
	}
	ctx, cancel := context.WithTimeout(ctx, localLimit)
	defer cancel()
	local, err := l.dial(ctx, port)
	if err != nil {
		return WordClosed, nil
	}
	return WordOK, local
}

func (l Local) dial(ctx context.Context, port uint16) (net.Conn, error) {
	if port == 3389 && l.RDP != nil {
		return l.RDP(localLimit)
	}
	var dialer net.Dialer
	return dialer.DialContext(ctx, "tcp", net.JoinHostPort("127.0.0.1", strconv.Itoa(int(port))))
}

func closeIfOpen(local net.Conn) {
	if local != nil {
		local.Close()
	}
}
