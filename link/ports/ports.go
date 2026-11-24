// Package ports is a device's list of local ports that its hub may reach (HMD-048, HMD-049); only the person on the computer changes it.
package ports

import (
	"context"
	"errors"
	"runtime"
	"slices"
	"time"

	"github.com/TheArctesian/anima/link/rdppeer"
	"github.com/TheArctesian/anima/link/statedir"
)

// ErrNotAllowed means that the port cannot be in the list on this system.
var ErrNotAllowed = errors.New("ports: only 22 (SSH), 5900 (screen sharing) and 3389 (remote desktop) on supported systems can be open")

const fileName = "open-ports"

// ErrHostNotReady means that the owner-only macOS RDP listener is absent.
var ErrHostNotReady = errors.New("ports: the bundled macOS RDP host is not ready")

// Allowed reports whether a port can be in the list on the system goos, a value of runtime.GOOS.
func Allowed(port uint16, goos string) bool {
	switch port {
	case 22, 5900:
		return true
	case 3389:
		return goos == "windows" || goos == "linux" || (goos == "darwin" && rdppeer.Supported)
	default:
		return false
	}
}

// List reads the open ports, in order. An absent file is an empty list, and a port this system does not allow is left out.
func List(dir *statedir.Dir) ([]uint16, error) {
	var named []uint16
	if err := dir.Read(fileName, &named); err != nil {
		if errors.Is(err, statedir.ErrAbsent) {
			return nil, nil
		}
		return nil, err
	}
	open := make([]uint16, 0, len(named))
	for _, port := range named {
		if Allowed(port, runtime.GOOS) && !slices.Contains(open, port) {
			open = append(open, port)
		}
	}
	slices.Sort(open)
	return open, nil
}

// IsOpen reports whether a port is in the list now; it reads the file at each call, and an unreadable list opens no port.
func IsOpen(dir *statedir.Dir, port uint16) bool {
	open, err := List(dir)
	return err == nil && slices.Contains(open, port)
}

// Turn puts a port in the list or takes it out.
func Turn(dir *statedir.Dir, port uint16, on bool) error {
	if !Allowed(port, runtime.GOOS) {
		return ErrNotAllowed
	}
	if on && runtime.GOOS == "darwin" && port == 3389 {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		connection, err := rdppeer.Connect(ctx, dir)
		if err != nil {
			return ErrHostNotReady
		}
		_ = connection.Close()
	}
	open, err := List(dir)
	if err != nil {
		return err
	}
	open = slices.DeleteFunc(append([]uint16{}, open...), func(listed uint16) bool { return listed == port })
	if on {
		open = append(open, port)
		slices.Sort(open)
	}
	return dir.Write(fileName, open)
}
