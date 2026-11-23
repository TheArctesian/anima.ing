package main

import (
	"context"
	"encoding/json"
	"errors"
	"net/netip"
	"time"

	"github.com/TheArctesian/anima/link/control"
	"github.com/TheArctesian/anima/link/devices"
	"github.com/TheArctesian/anima/link/hublink"
	"github.com/TheArctesian/anima/link/tunnel"
)

type hubStatus struct {
	Role    string `json:"role"`
	Relay   string `json:"relay"`
	Devices int    `json:"devices"`
}

type listedDevice struct {
	Key tunnel.DeviceKey `json:"key"`
	// Fingerprint matches what "anima-link join" prints on the device, for the operator to compare.
	Fingerprint string     `json:"fingerprint"`
	Name        string     `json:"name"`
	Address     netip.Addr `json:"address"`
	Permitted   time.Time  `json:"permitted"`
	Expires     time.Time  `json:"expires"`
	Streams     int        `json:"streams"`
	// NextFingerprint is set while the device renews its key (LNK-005).
	NextFingerprint string `json:"nextFingerprint,omitempty"`
	// Path is "direct", "relayed" or "none" (LNK-006).
	Path  string `json:"path"`
	Relay string `json:"relay,omitempty"`
	// Seen is the last "here" line on the reverse port (HMD-047), absent when none came since the hub link started.
	Seen *time.Time `json:"seen,omitempty"`
}

type revokeRequest struct {
	Key tunnel.DeviceKey `json:"key"`
}

type revoked struct {
	StreamsClosed int `json:"streamsClosed"`
}

type permitRequest struct {
	Key  tunnel.DeviceKey `json:"key"`
	Name string           `json:"name"`
}

func hubVerbs(link *hublink.Link, permitted *devices.Store) map[string]control.Verb {
	return map[string]control.Verb{
		"status": func(context.Context, json.RawMessage) (any, error) {
			return hubStatus{Role: "hub", Relay: link.Relay().Name(), Devices: permitted.Count()}, nil
		},
		// The hub address is a secret: it goes only to the caller, never to a log (LNK-008).
		"address.show": func(context.Context, json.RawMessage) (any, error) {
			return link.Address().Reveal(), nil
		},
		"devices.list": func(context.Context, json.RawMessage) (any, error) {
			listed := []listedDevice{}
			for _, device := range permitted.All() {
				path := link.Path(device.Key)
				next := ""
				if !device.NextKey.IsZero() {
					next = device.NextKey.Fingerprint()
				}
				var seen *time.Time
				if at, known := link.Seen(device.Address); known {
					at = at.UTC()
					seen = &at
				}
				listed = append(listed, listedDevice{
					NextFingerprint: next, Seen: seen,
					Key: device.Key, Fingerprint: device.Key.Fingerprint(), Name: device.Name, Address: device.Address, Permitted: device.Permitted, Expires: device.Expires,
					Streams: link.Streams(device.Key), Path: string(path.Kind), Relay: path.Relay,
				})
			}
			return listed, nil
		},
		"devices.permit": func(_ context.Context, args json.RawMessage) (any, error) {
			var request permitRequest
			if err := control.Arguments(args, &request); err != nil {
				return nil, err
			}
			_, err := permitted.Permit(devices.Device{Key: request.Key, Name: request.Name})
			return nil, refusalOf(err)
		},
		"devices.revoke": func(_ context.Context, args json.RawMessage) (any, error) {
			var request revokeRequest
			if err := control.Arguments(args, &request); err != nil {
				return nil, err
			}
			closed, err := link.Revoke(request.Key)
			return revoked{closed}, refusalOf(err)
		},
	}
}

func refusalOf(err error) error {
	var rule interface {
		error
		Code() string
	}
	if errors.As(err, &rule) {
		return &control.Error{Code: rule.Code(), Message: rule.Error()}
	}
	return err
}
