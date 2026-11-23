package hublink

import (
	"errors"
	"net"
	"net/netip"
	"path/filepath"

	"github.com/TheArctesian/anima/link/statedir"
)

// ErrTarget means that the target is neither a loopback address with a port nor the path of a socket.
var ErrTarget = errors.New("hublink: the target must be the path of the hub's socket, or a loopback address and a port, for example 127.0.0.1:8443")

type target struct {
	socket   string
	loopback string
}

func parseTarget(text string) (target, error) {
	if filepath.IsAbs(text) {
		return target{socket: filepath.Clean(text)}, nil
	}
	loopback, err := netip.ParseAddrPort(text)
	if err != nil || !loopback.Addr().IsLoopback() || loopback.Port() == 0 {
		return target{}, ErrTarget
	}
	return target{loopback: loopback.String()}, nil
}

// dial checks the socket at each connection: the hub makes it again at each start, which can be after the link.
func (t target) dial() (net.Conn, error) {
	if t.socket != "" {
		return statedir.DialOwned(t.socket, targetDialLimit)
	}
	return net.DialTimeout("tcp", t.loopback, targetDialLimit)
}
