package devices

import (
	"net/netip"

	"github.com/TheArctesian/anima/link/tunnel"
)

// A key or an address that left the list stays refused: the hub keeps what it bound to that address (LNK-004).

const retiredFileName = "retired.json"

type retired struct {
	Keys      []tunnel.DeviceKey `json:"keys"`
	Addresses []netip.Addr       `json:"addresses"`
}

func (r retired) hasKey(key tunnel.DeviceKey) bool {
	for _, gone := range r.Keys {
		if gone == key {
			return true
		}
	}
	return false
}

func (r retired) hasAddress(address netip.Addr) bool {
	for _, gone := range r.Addresses {
		if gone == address {
			return true
		}
	}
	return false
}

func (r retired) refuses(key tunnel.DeviceKey) bool {
	return r.hasKey(key) || r.hasAddress(addressOf(key))
}

func (r retired) withKeys(keys ...tunnel.DeviceKey) retired {
	next := retired{Keys: append([]tunnel.DeviceKey(nil), r.Keys...), Addresses: r.Addresses}
	for _, key := range keys {
		if !key.IsZero() && !next.hasKey(key) {
			next.Keys = append(next.Keys, key)
		}
	}
	return next
}

func (r retired) withDevice(device Device) retired {
	next := r.withKeys(device.Key, device.NextKey)
	if !next.hasAddress(device.Address) {
		next.Addresses = append(append([]netip.Addr(nil), next.Addresses...), device.Address)
	}
	return next
}
