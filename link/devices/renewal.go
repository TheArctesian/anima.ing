package devices

import (
	"time"

	"github.com/TheArctesian/anima/link/tunnel"
)

// RenewWindow is the last part of a key's life, in which the device can
// give its next key (LNK-005).
const RenewWindow = 30 * 24 * time.Hour

// A renewing device keeps one entry with both keys, so a revocation removes both.
// Only a tunnel stream proves the device holds its next key, so only such a stream promotes it.

func (l list) renewing(current, next tunnel.DeviceKey, now time.Time) (list, Device, error) {
	device, known := l[current]
	if !known || !now.Before(device.Expires) {
		return nil, Device{}, ErrUnknown
	}
	if now.Before(device.Expires.Add(-RenewWindow)) {
		return nil, Device{}, ErrNotDue
	}
	if next.IsZero() {
		return nil, Device{}, ErrNoKey
	}
	if holder, taken := l.holder(next); taken && (holder.Key != current || next == current) {
		return nil, Device{}, ErrKnown
	}
	device.NextKey = next
	updated := l.copy()
	updated[current] = device
	return updated, device, nil
}

// Promotion is a completed renewal.
type Promotion struct {
	Device   Device
	Replaced tunnel.DeviceKey
}

func (l list) promoted(next tunnel.DeviceKey, now time.Time) (list, Promotion, bool) {
	device, known := l.holder(next)
	if !known || device.NextKey != next || !now.Before(device.Expires) {
		return nil, Promotion{}, false
	}
	old := device.Key
	device.Key, device.NextKey = next, tunnel.DeviceKey{}
	device.Permitted, device.Expires = now, now.Add(KeyLife)
	updated := l.copy()
	delete(updated, old)
	updated[next] = device
	return updated, Promotion{Device: device, Replaced: old}, true
}
