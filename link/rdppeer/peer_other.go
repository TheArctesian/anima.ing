//go:build !darwin || !arm64 || !cgo

package rdppeer

import "net"

// Supported reports whether this build has Apple's bundled-host authenticator.
const Supported = false

func admit(net.Conn) bool { return false }
