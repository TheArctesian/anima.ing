package main

import (
	"errors"
	"net"
	"os"
	"path/filepath"
)

type metricsListener struct {
	net.Listener
	folder string
}

func (l metricsListener) Close() error {
	return errors.Join(l.Listener.Close(), os.Remove(l.folder))
}

// iOS application support paths are too long for sun_path; MkdirTemp gives a shorter, owner-only folder.
func iosMetricsSocket() net.Listener {
	folder, err := os.MkdirTemp("", "")
	if err != nil {
		return nil
	}
	socket, err := net.Listen("unix", filepath.Join(folder, "m"))
	if err != nil {
		_ = os.Remove(folder)
		return nil
	}
	return metricsListener{Listener: socket, folder: folder}
}
