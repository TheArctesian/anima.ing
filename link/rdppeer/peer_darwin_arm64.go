//go:build darwin && arm64 && cgo

package rdppeer

// #cgo LDFLAGS: -framework Security -framework CoreFoundation
// #include "peer.h"
import "C"

import "net"

// Supported reports whether this build has Apple's bundled-host authenticator.
const Supported = true

func admit(stream net.Conn) bool {
	unix, ok := stream.(*net.UnixConn)
	if !ok {
		return false
	}
	socket, err := unix.SyscallConn()
	if err != nil {
		return false
	}
	var admitted bool
	err = socket.Control(func(fd uintptr) {
		admitted = C.anima_rdp_host_peer(C.int(fd)) != 0
	})
	return err == nil && admitted
}
