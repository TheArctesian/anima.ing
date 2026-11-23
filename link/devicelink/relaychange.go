package devicelink

import (
	"bufio"
	"context"
	"encoding/json"

	"github.com/TheArctesian/anima/link/invites"
	"github.com/TheArctesian/anima/link/tunnel"
)

const failuresBeforeNextHub = 3

// readNotice asks the hub for its next address and keeps it (LNK-007).
func (l *Link) readNotice(ctx context.Context) {
	if l.save == nil {
		return
	}
	ctx, cancel := context.WithTimeout(ctx, l.pingLimit)
	defer cancel()
	stream, err := l.tunnel().Dial(ctx, tunnel.NoticePort)
	if err != nil {
		return
	}
	defer stream.Close()
	if deadline, ok := ctx.Deadline(); ok {
		_ = stream.SetDeadline(deadline)
	}
	var notice invites.Notice
	line, err := bufio.NewReader(stream).ReadBytes('\n')
	if err != nil || json.Unmarshal(line, &notice) != nil || notice.NextHub == "" {
		return
	}
	next, err := tunnel.ParseAddress(notice.NextHub)
	current := l.identity()
	if err != nil || next == current.NextHub || next == current.Hub || !next.SameHub(current.Hub) {
		return
	}
	current.NextHub = next
	if l.save(current) == nil {
		l.setIdentity(current)
	}
}

func (l *Link) followHub(ctx context.Context) {
	current := l.identity()
	if l.save == nil {
		return
	}
	candidates := []tunnel.Address{}
	if !current.NextHub.IsZero() {
		candidates = append(candidates, current.NextHub)
	}
	for _, named := range l.addressesOfName(ctx, current.HubName) {
		if named != current.Hub && named != current.NextHub {
			candidates = append(candidates, named)
		}
	}
	for _, candidate := range candidates {
		if l.moveTo(ctx, candidate) {
			return
		}
	}
}

func (l *Link) moveTo(ctx context.Context, address tunnel.Address) bool {
	current := l.identity()
	if !address.SameHub(current.Hub) {
		return false
	}
	moved, err := tunnel.NewDevice(tunnel.DeviceConfig{Key: current.Key, Hub: address, DebugLogf: l.debugLogf})
	if err != nil {
		return false
	}
	ctx, cancel := context.WithTimeout(ctx, l.registerLimit)
	defer cancel()
	if !answersAsHub(ctx, moved) {
		moved.Close()
		return false
	}
	current.Hub, current.NextHub = address, tunnel.Address{}
	if l.save(current) != nil {
		moved.Close()
		return false
	}
	l.replaceTunnel(moved, current).Close()
	return true
}

// answersAsHub needs a stream, not a Register answer: anybody on the relay can answer Register,
// but only the holder of the hub's keys completes the tunnel handshake.
func answersAsHub(ctx context.Context, device *tunnel.Device) bool {
	if device.Register(ctx) != nil {
		return false
	}
	stream, err := device.Dial(ctx, tunnel.NoticePort)
	if err != nil {
		return false
	}
	defer stream.Close()
	if deadline, ok := ctx.Deadline(); ok {
		_ = stream.SetDeadline(deadline)
	}
	_, err = bufio.NewReader(stream).ReadBytes('\n')
	return err == nil
}

// hubNameLabel prefixes the hub's domain name in the TXT record that holds the hub address.
const hubNameLabel = "_anima-link."

// addressesOfName reads the hub addresses that a domain name holds (LNK-007).
// The record is public, so only the hub's key list, not the pre-shared key, protects a named hub.
func (l *Link) addressesOfName(ctx context.Context, name string) []tunnel.Address {
	if name == "" {
		return nil
	}
	ctx, cancel := context.WithTimeout(ctx, l.pingLimit)
	defer cancel()
	records, err := l.lookupTXT(ctx, hubNameLabel+name)
	if err != nil {
		return nil
	}
	found := []tunnel.Address{}
	for _, record := range records {
		if address, err := tunnel.ParseAddress(record); err == nil {
			found = append(found, address)
		}
	}
	return found
}
