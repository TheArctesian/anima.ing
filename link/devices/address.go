package devices

import (
	"crypto/sha256"
	"net/netip"

	"github.com/TheArctesian/anima/link/tunnel"
)

// A link address names a device to the hub; no packet goes to it, and it stays when the key renews (LNK-005).

// linkRange holds every link address; the hub accepts no other address from the link.
var linkRange = netip.MustParsePrefix("fd41:4e49:4d41::/48")

const addressLabel = "anima-link device address 1\n"

func addressOf(key tunnel.DeviceKey) netip.Addr {
	text, _ := key.MarshalText() // never fails for a key
	sum := sha256.Sum256(append([]byte(addressLabel), text...))
	raw := linkRange.Addr().As16()
	copy(raw[linkRange.Bits()/8:], sum[:])
	return netip.AddrFrom16(raw)
}

func (l list) addressTaken(address netip.Addr) bool {
	for _, device := range l {
		if device.Address == address {
			return true
		}
	}
	return false
}
