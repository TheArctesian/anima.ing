package hublink

import (
	"encoding/json"
	"net"
	"time"

	"github.com/TheArctesian/anima/link/invites"
	"github.com/TheArctesian/anima/link/tunnel"
)

// A relay change moves the hub's address (LNK-007): devices learn the next address on the notice port before
// the link restarts on the new relay, so a connected device needs no new invite.

// Announce offers permitted devices the hub's address on relay next, and returns that address.
func (l *Link) Announce(next tunnel.Relay) tunnel.Address {
	address := tunnel.AddressOn(l.identity.Key, l.identity.PresharedKey, next)
	l.next.Store(&address)
	return address
}

func (l *Link) notify(stream net.Conn) {
	defer stream.Close()
	device, known := l.hub.DeviceOf(stream.RemoteAddr())
	if !known || !l.devices.Permits(device) {
		return
	}
	var notice invites.Notice
	if next := l.next.Load(); next != nil {
		notice.NextHub = next.Reveal()
	}
	_ = stream.SetDeadline(time.Now().Add(enrolLimit))
	_ = json.NewEncoder(stream).Encode(notice)
}
