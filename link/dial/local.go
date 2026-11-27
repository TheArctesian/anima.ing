package dial

import (
	"context"
	"io"
	"net"
	"strconv"
	"time"

	"github.com/TheArctesian/anima/link/tunnel"
	"github.com/TheArctesian/anima/link/workspaceauth"
	"github.com/TheArctesian/anima/link/workspacepreview"
)

const localLimit = 5 * time.Second

// Local is the device side: what a device needs to answer a request.
type Local struct {
	// Permit is the current local port authorization (HMD-049). A nil result denies the port.
	Permit func(port uint16) context.Context
	Hub    func(ctx context.Context) (net.Conn, error)
}

// Give answers one hub request, dialling the loopback port only when it is open.
func (l Local) Give(ctx context.Context, request Request) {
	var word string
	var local net.Conn
	var permit context.Context
	if request.Provider != nil {
		var err error
		local, err = workspaceauth.Connect(ctx, *request.Provider)
		word = WordOK
		if err != nil {
			word = WordRefused
		}
	} else if request.Preview != nil {
		var err error
		local, err = workspacepreview.Connect(ctx, *request.Preview)
		word = WordOK
		if err != nil {
			word = WordRefused
		}
	} else {
		word, local, permit = l.connect(ctx, request.Port)
	}
	if permit != nil {
		stopLocal := closeOnRevoke(permit, local)
		defer stopLocal()
	}
	hub, err := l.Hub(ctx)
	if err != nil {
		closeIfOpen(local)
		return
	}
	if permit != nil && permit.Err() != nil {
		closeIfOpen(local)
		local, permit, word = nil, nil, WordRefused
	}
	if permit != nil {
		stopHub := closeOnRevoke(permit, hub)
		defer stopHub()
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

func (l Local) connect(ctx context.Context, port uint16) (string, net.Conn, context.Context) {
	if l.Permit == nil {
		return WordRefused, nil, nil
	}
	permit := l.Permit(port)
	if permit == nil || permit.Err() != nil {
		return WordRefused, nil, nil
	}
	ctx, cancel := context.WithTimeout(ctx, localLimit)
	defer cancel()
	stopDial := context.AfterFunc(permit, cancel)
	defer stopDial()
	var dialer net.Dialer
	local, err := dialer.DialContext(ctx, "tcp", net.JoinHostPort("127.0.0.1", strconv.Itoa(int(port))))
	if permit.Err() != nil {
		closeIfOpen(local)
		return WordRefused, nil, nil
	}
	if err != nil {
		return WordClosed, nil, nil
	}
	return WordOK, local, permit
}

func closeOnRevoke(permit context.Context, stream net.Conn) func() bool {
	return context.AfterFunc(permit, func() { stream.Close() })
}

func closeIfOpen(local net.Conn) {
	if local != nil {
		local.Close()
	}
}
