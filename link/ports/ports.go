// Package ports is a device's list of local ports that its hub may reach (HMD-048, HMD-049); only the person on the computer changes it.
package ports

import (
	"errors"
	"runtime"
	"slices"

	"github.com/TheArctesian/anima/link/statedir"
)

// ErrNotAllowed means that the port cannot be in the list on this system.
var ErrNotAllowed = errors.New("ports: only 22 (SSH), 5900 (screen sharing) and, on Windows or Linux, 3389 (remote desktop) can be open")

const fileName = "open-ports"

// Allowed reports whether a port can be in the list on the system goos, a value of runtime.GOOS.
func Allowed(port uint16, goos string) bool {
	switch port {
	case 22, 5900:
		return true
	case 3389:
		return goos == "windows" || goos == "linux"
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
