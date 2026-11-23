// Package identity holds what a hub or a device keeps between starts to stay the same (LNK-001).
package identity

import (
	"errors"
	"fmt"
	"time"

	"github.com/TheArctesian/anima/link/statedir"
	"github.com/TheArctesian/anima/link/tunnel"
)

const (
	hubFile    = "hub.json"
	deviceFile = "device.json"
)

// Hub is the saved identity of a hub; its key, pre-shared key and relay give the hub address.
type Hub struct {
	Key          tunnel.PrivateKey
	PresharedKey tunnel.PresharedKey
	// Relay is zero until the first start has chosen a relay.
	Relay tunnel.Relay
	// NextRelay is the relay that the hub will change to; while it is set, devices are told the next address (LNK-007).
	NextRelay tunnel.Relay
}

// savedHub is the file form of Hub, the only form that holds the secrets as text.
type savedHub struct {
	Key          string       `json:"key"`
	PresharedKey string       `json:"presharedKey"`
	Relay        tunnel.Relay `json:"relay"`
	NextRelay    tunnel.Relay `json:"nextRelay"`
}

// LoadHub reads the hub's identity; at the first call it makes the keys and saves them.
func LoadHub(dir *statedir.Dir) (Hub, error) {
	var saved savedHub
	err := dir.Read(hubFile, &saved)
	if errors.Is(err, statedir.ErrAbsent) {
		hub := Hub{Key: tunnel.NewPrivateKey(), PresharedKey: tunnel.NewPresharedKey()}
		return hub, hub.Save(dir)
	}
	if err != nil {
		return Hub{}, err
	}
	key, keyErr := tunnel.ParsePrivateKey(saved.Key)
	psk, pskErr := tunnel.ParsePresharedKey(saved.PresharedKey)
	if keyErr != nil || pskErr != nil {
		return Hub{}, fmt.Errorf("%w: %s", statedir.ErrDamaged, hubFile)
	}
	return Hub{Key: key, PresharedKey: psk, Relay: saved.Relay, NextRelay: saved.NextRelay}, nil
}

// ErrNoNextRelay means that the hub has announced no relay to change to.
var ErrNoNextRelay = errors.New("identity: no relay is announced")

// Switched returns the identity with the announced relay as the hub's relay (LNK-007).
func (h Hub) Switched() (Hub, error) {
	if h.NextRelay.IsZero() {
		return Hub{}, ErrNoNextRelay
	}
	h.Relay, h.NextRelay = h.NextRelay, tunnel.Relay{}
	return h, nil
}

// Save writes the hub's identity.
func (h Hub) Save(dir *statedir.Dir) error {
	return dir.Write(hubFile, savedHub{h.Key.Reveal(), h.PresharedKey.Reveal(), h.Relay, h.NextRelay})
}

// Device is the saved identity of a device.
type Device struct {
	Key tunnel.PrivateKey
	// Hub is the address of the device's hub. It is zero before enrolment.
	Hub tunnel.Address
	// HubName is a domain name whose TXT record holds the hub's address, or empty (LNK-007).
	HubName string
	// NextHub is the hub's announced address after its relay change, tried when Hub does not answer (LNK-007).
	NextHub tunnel.Address
	// Expires is when the hub stops permitting Key (LNK-005).
	Expires time.Time
	// NextKey is the key that replaces Key, set from the start of a renewal until the hub accepts it.
	NextKey tunnel.PrivateKey
}

type savedDevice struct {
	Key     string    `json:"key"`
	Hub     string    `json:"hub,omitempty"`
	HubName string    `json:"hubName,omitempty"`
	NextHub string    `json:"nextHub,omitempty"`
	Expires time.Time `json:"expires,omitzero"`
	NextKey string    `json:"nextKey,omitempty"`
}

// LoadDevice reads the device's identity; at the first call it makes the key and saves it.
func LoadDevice(dir *statedir.Dir) (Device, error) {
	var saved savedDevice
	err := dir.Read(deviceFile, &saved)
	if errors.Is(err, statedir.ErrAbsent) {
		device := Device{Key: tunnel.NewPrivateKey()}
		return device, device.Save(dir)
	}
	if err != nil {
		return Device{}, err
	}
	device := Device{Expires: saved.Expires, HubName: saved.HubName}
	if device.Key, err = tunnel.ParsePrivateKey(saved.Key); err != nil {
		return Device{}, fmt.Errorf("%w: %s", statedir.ErrDamaged, deviceFile)
	}
	if saved.NextHub != "" {
		if device.NextHub, err = tunnel.ParseAddress(saved.NextHub); err != nil {
			return Device{}, fmt.Errorf("%w: %s", statedir.ErrDamaged, deviceFile)
		}
	}
	if saved.NextKey != "" {
		if device.NextKey, err = tunnel.ParsePrivateKey(saved.NextKey); err != nil {
			return Device{}, fmt.Errorf("%w: %s", statedir.ErrDamaged, deviceFile)
		}
	}
	if saved.Hub != "" {
		if device.Hub, err = tunnel.ParseAddress(saved.Hub); err != nil {
			return Device{}, fmt.Errorf("%w: %s", statedir.ErrDamaged, deviceFile)
		}
	}
	return device, nil
}

// Save writes the device's identity.
func (d Device) Save(dir *statedir.Dir) error {
	saved := savedDevice{Key: d.Key.Reveal(), Hub: d.Hub.Reveal(), HubName: d.HubName, NextHub: d.NextHub.Reveal(), Expires: d.Expires}
	if !d.NextKey.IsZero() {
		saved.NextKey = d.NextKey.Reveal()
	}
	return dir.Write(deviceFile, saved)
}
