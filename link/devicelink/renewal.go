package devicelink

import (
	"context"
	"time"

	"github.com/TheArctesian/anima/link/devices"
	"github.com/TheArctesian/anima/link/invites"
	"github.com/TheArctesian/anima/link/tunnel"
)

const renewStepLimit = 5 * time.Second

// renewIfDue replaces the device's key in its renew window (LNK-005): save the next key, give it with the current key,
// confirm it with the next key, then save it as current. Each step keeps one working key, so a stopped device resumes.
func (l *Link) renewIfDue(ctx context.Context) {
	current := l.identity()
	if l.save == nil || current.Expires.IsZero() {
		return
	}
	if current.NextKey.IsZero() {
		if l.now().Before(current.Expires.Add(-devices.RenewWindow)) {
			return
		}
		current.NextKey = tunnel.NewPrivateKey()
		if l.save(current) != nil {
			return
		}
		l.setIdentity(current)
	}
	next, err := tunnel.NewDevice(tunnel.DeviceConfig{Key: current.NextKey, Hub: current.Hub, DebugLogf: l.debugLogf})
	if err != nil {
		return
	}
	_, _ = step(ctx, l.tunnel(), current.NextKey.Public())
	confirmed, err := step(ctx, next, current.NextKey.Public())
	if err != nil {
		next.Close()
		return // the next keepalive tries again
	}
	renewed := current
	renewed.Key, renewed.NextKey, renewed.Expires = current.NextKey, tunnel.PrivateKey{}, confirmed.Expires
	if l.save(renewed) != nil {
		next.Close()
		return
	}
	l.replaceTunnel(next, renewed).Close()
}

func step(ctx context.Context, sender *tunnel.Device, key tunnel.DeviceKey) (invites.EnrolReply, error) {
	ctx, cancel := context.WithTimeout(ctx, renewStepLimit)
	defer cancel()
	return enrol(ctx, sender, key)
}
