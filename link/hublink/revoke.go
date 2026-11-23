package hublink

import (
	"time"

	"github.com/TheArctesian/anima/link/tunnel"
)

// Revoke removes a device from the list and ends its open sessions, and returns how many streams it closed (LNK-004).
// The key leaves the list before its streams close, so the device cannot connect again; a sweep finishes a cut revocation.
func (l *Link) Revoke(device tunnel.DeviceKey) (closed int, err error) {
	revoked, err := l.devices.Revoke(device)
	if err != nil {
		return 0, err
	}
	closed = l.disconnect(revoked.Key)
	if !revoked.NextKey.IsZero() {
		closed += l.disconnect(revoked.NextKey)
	}
	l.log.Info("a device was revoked", "device", revoked.Key, "name", revoked.Name, "streams", closed)
	return closed, nil
}

// disconnect closes a key's streams itself, because the tunnel's call does not, and lets the close reach the device first.
func (l *Link) disconnect(key tunnel.DeviceKey) (closed int) {
	closed = l.streams.closeOf(key)
	time.AfterFunc(closeGrace, func() {
		select {
		case <-l.stopped:
		default:
			l.hub.Disconnect(key)
		}
	})
	return closed
}

func (l *Link) sweep(every time.Duration) {
	ticker := time.NewTicker(every)
	defer ticker.Stop()
	for {
		select {
		case <-l.stopped:
			return
		case <-ticker.C:
		}
		for key, path := range l.hub.Paths() {
			connected := path.Kind != tunnel.PathNone || l.streams.count(key) > 0
			if connected && !l.admitted(key) {
				l.log.Info("a device that is not permitted any more was disconnected", "device", key)
				l.disconnect(key)
			}
		}
	}
}
