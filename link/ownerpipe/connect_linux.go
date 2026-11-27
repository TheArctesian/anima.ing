//go:build linux

package ownerpipe

import (
	"context"
	"encoding/json"
	"io"
	"net"
	"syscall"
	"time"
)

// Connect verifies the root-owned peer before exchanging a bounded handshake.
func Connect(ctx context.Context, endpoint Endpoint, frame []byte) (net.Conn, error) {
	if len(frame) == 0 || len(frame) > 2048 {
		return nil, ErrRefused
	}
	var dialer net.Dialer
	conn, err := dialer.DialContext(ctx, "unix", endpoint.Socket)
	if err != nil {
		return nil, ErrRefused
	}
	accepted := false
	defer func() {
		if !accepted {
			conn.Close()
		}
	}()
	if !rootPeer(conn) {
		return nil, ErrRefused
	}
	deadline := time.Now().Add(10 * time.Second)
	if d, ok := ctx.Deadline(); ok && d.Before(deadline) {
		deadline = d
	}
	_ = conn.SetDeadline(deadline)
	if _, err = conn.Write(append(frame, '\n')); err != nil {
		return nil, ErrRefused
	}
	if !handshake(conn, endpoint.Protocol) {
		return nil, ErrRefused
	}
	_ = conn.SetDeadline(time.Time{})
	accepted = true
	return conn, nil
}

func rootPeer(conn net.Conn) bool {
	unix, ok := conn.(*net.UnixConn)
	if !ok {
		return false
	}
	raw, err := unix.SyscallConn()
	if err != nil {
		return false
	}
	root := false
	if raw.Control(func(fd uintptr) {
		cred, e := syscall.GetsockoptUcred(int(fd), syscall.SOL_SOCKET, syscall.SO_PEERCRED)
		root = e == nil && cred.Uid == 0
	}) != nil || !root {
		return false
	}
	return true
}

func handshake(conn net.Conn, protocol string) bool {
	// Read only the handshake; subsequent bytes belong to the caller.
	line := make([]byte, 0, 128)
	ended := false
	for len(line) < 256 {
		var one [1]byte
		if _, err := io.ReadFull(conn, one[:]); err != nil {
			return false
		}
		if one[0] == '\n' {
			ended = true
			break
		}
		line = append(line, one[0])
	}
	var answer struct {
		OK     bool   `json:"ok"`
		Stream string `json:"stream"`
	}
	if !ended || json.Unmarshal(line, &answer) != nil || !answer.OK || answer.Stream != protocol {
		return false
	}
	return true
}
