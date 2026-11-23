package invites

import (
	"time"

	"github.com/TheArctesian/anima/link/tunnel"
)

// EnrolRequest is the one line that a device sends on the enrolment port: an invite key joins (LNK-003),
// a current key gives its next key (LNK-005), and a next key confirms the renewal.
type EnrolRequest struct {
	// Key is the device's new public key; the hub does not read it in a confirmation.
	Key  tunnel.DeviceKey `json:"key"`
	Name string           `json:"name,omitempty"`
}

// Notice is the one line that the hub link writes on the notice port.
type Notice struct {
	// NextHub is the hub's address after its relay change, empty when none is planned. It is a secret.
	NextHub string `json:"nextHub,omitempty"`
}

// EnrolReply is the one line that the hub link answers.
type EnrolReply struct {
	OK bool `json:"ok"`
	// Expires is when the device's key stops being permitted; a join and a confirmation give it.
	Expires time.Time `json:"expires,omitzero"`
	// Code names the rule that refused the device, for a program.
	Code string `json:"code,omitempty"`
}
