package hublink

import (
	"bufio"
	"encoding/json"
	"errors"
	"io"
	"net"
	"time"

	"github.com/TheArctesian/anima/link/devices"
	"github.com/TheArctesian/anima/link/invites"
	"github.com/TheArctesian/anima/link/tunnel"
)

const (
	enrolLimit       = 10 * time.Second
	enrolRequestSize = 1024
	maxEnrolments    = 16
)

func (l *Link) admitted(key tunnel.DeviceKey) bool {
	return l.devices.Permits(key) || (l.invites != nil && l.invites.Admits(key, l.now()))
}

func (l *Link) enrol(stream net.Conn) {
	defer stream.Close()
	sender, known := l.hub.DeviceOf(stream.RemoteAddr())
	if !known || !l.admitted(sender) {
		return
	}
	select {
	case l.enrolling <- struct{}{}:
		defer func() { <-l.enrolling }()
	default:
		return
	}
	l.enrolments.Add(1)
	defer l.enrolments.Done()
	_ = stream.SetDeadline(time.Now().Add(enrolLimit))
	var request invites.EnrolRequest
	line, err := bufio.NewReader(io.LimitReader(stream, enrolRequestSize)).ReadBytes('\n')
	if err != nil || json.Unmarshal(line, &request) != nil {
		return
	}
	if l.devices.Permits(sender) {
		_ = json.NewEncoder(stream).Encode(l.renewOrConfirm(sender, request.Key))
		return
	}
	reply, answered := l.join(sender, request)
	if !answered {
		return
	}
	_ = json.NewEncoder(stream).Encode(reply)
	stream.Close()
	l.disconnect(sender)
}

// join ends the invite first, so a second use fails even when this use fails later.
func (l *Link) join(temporary tunnel.DeviceKey, request invites.EnrolRequest) (reply invites.EnrolReply, answered bool) {
	now := l.now()
	invite, err := l.invites.Use(temporary, now)
	if err != nil {
		return invites.EnrolReply{}, false
	}
	device := request.Key
	name := invite.Name
	if name == "" {
		name = suggestedName(request.Name)
	}
	if device == temporary || !l.deviceKey(device) {
		l.log.Warn("an invite was used with a key that cannot be a device key", "name", name)
		return invites.EnrolReply{Code: codeBadKey}, true
	}
	permitted, err := l.devices.Permit(devices.Device{Key: device, Name: name, Permitted: now.UTC()})
	if err != nil {
		l.log.Warn("an invite was used, but the device was not permitted", "name", name, "cause", codeOf(err))
		return invites.EnrolReply{Code: codeOf(err)}, true
	}
	l.log.Info("a device joined with an invite", "device", device, "name", name)
	return invites.EnrolReply{OK: true, Expires: permitted.Expires}, true
}

func suggestedName(name string) string {
	if devices.ValidName(name) == nil {
		return name
	}
	return "Device"
}

// renewOrConfirm serves a permitted device (LNK-005): a stream from its next key proves it holds that key.
func (l *Link) renewOrConfirm(sender, next tunnel.DeviceKey) invites.EnrolReply {
	promotion, err := l.devices.Promote(sender)
	if err != nil {
		l.log.Error("a renewed key could not be saved", "device", sender)
		return invites.EnrolReply{Code: codeOf(err)}
	}
	if promotion != nil {
		l.log.Info("a device renewed its key", "device", sender, "replaced", promotion.Replaced, "name", promotion.Device.Name)
		l.disconnect(promotion.Replaced)
		return invites.EnrolReply{OK: true, Expires: promotion.Device.Expires}
	}
	if !l.deviceKey(next) {
		return invites.EnrolReply{Code: codeBadKey}
	}
	device, err := l.devices.Renew(sender, next)
	if err != nil {
		return invites.EnrolReply{Code: codeOf(err)}
	}
	l.log.Info("a device gave its next key", "device", sender, "name", device.Name)
	return invites.EnrolReply{OK: true}
}

const codeBadKey = "bad_key"

// deviceKey refuses an open invite's temporary key, which any later reader of the invite could make (LNK-003).
func (l *Link) deviceKey(key tunnel.DeviceKey) bool {
	return key != l.ownKey && !l.invites.Admits(key, l.now())
}

func codeOf(err error) string {
	var rule interface {
		error
		Code() string
	}
	if errors.As(err, &rule) {
		return rule.Code()
	}
	return "refused"
}
