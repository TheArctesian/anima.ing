package devicelink

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"

	"github.com/TheArctesian/anima/link/identity"
	"github.com/TheArctesian/anima/link/invites"
	"github.com/TheArctesian/anima/link/tunnel"
)

var (
	// ErrNoAnswer means that the hub did not answer: a wrong, used or expired invite and an off hub look the same.
	ErrNoAnswer = errors.New("devicelink: the hub did not answer; the invite is used, expired or wrong, or the hub is off")
	// ErrRefused means that the hub ended the invite but did not permit the device.
	ErrRefused = errors.New("devicelink: the hub refused this device; ask for a new invite")
)

// Join uses an invite to give the hub the device's public key (LNK-003), and returns the identity for the caller to save.
func Join(ctx context.Context, invite invites.Invite, device identity.Device, name string) (identity.Device, error) {
	temporary, err := tunnel.NewDevice(tunnel.DeviceConfig{Key: invite.TemporaryKey(), Hub: invite.Hub})
	if err != nil {
		return identity.Device{}, err
	}
	defer temporary.Close()
	reply, err := enrol(ctx, temporary, device.Key.Public(), name)
	if err != nil {
		return identity.Device{}, err
	}
	device.Hub, device.Expires = invite.Hub, reply.Expires
	return device, nil
}

func enrol(ctx context.Context, sender *tunnel.Device, key tunnel.DeviceKey, name string) (invites.EnrolReply, error) {
	stream, err := sender.Dial(ctx, tunnel.EnrolPort)
	if err != nil {
		return invites.EnrolReply{}, ErrNoAnswer
	}
	defer stream.Close()
	if deadline, ok := ctx.Deadline(); ok {
		_ = stream.SetDeadline(deadline)
	}
	if err := json.NewEncoder(stream).Encode(invites.EnrolRequest{Key: key, Name: name}); err != nil {
		return invites.EnrolReply{}, ErrNoAnswer
	}
	var reply invites.EnrolReply
	line, err := bufio.NewReader(stream).ReadBytes('\n')
	if err != nil || json.Unmarshal(line, &reply) != nil {
		return invites.EnrolReply{}, ErrNoAnswer
	}
	if !reply.OK {
		// %q keeps a hostile hub's code from writing control characters to the user's terminal.
		return invites.EnrolReply{}, fmt.Errorf("%w (%q)", ErrRefused, reply.Code)
	}
	return reply, nil
}
