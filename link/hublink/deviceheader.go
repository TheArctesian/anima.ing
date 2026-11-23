package hublink

import (
	"encoding/binary"
	"net"
	"net/netip"
)

// A stream to the hub's socket starts with a PROXY v2 header whose source is the device's link address.
// Whoever writes the header names the device, so a loopback target, which any local program can reach, gets none.

var proxySignature = []byte{0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A}

const (
	proxyVersion2Proxy = 0x21 // version 2, command PROXY
	proxyTCPOverIPv6   = 0x21 // family IPv6, protocol stream
	proxyIPv6Addresses = 36   // two addresses and two ports
)

func deviceHeader(device netip.Addr, sourcePort uint16, to netip.AddrPort) []byte {
	header := append([]byte{}, proxySignature...)
	header = append(header, proxyVersion2Proxy, proxyTCPOverIPv6)
	header = binary.BigEndian.AppendUint16(header, proxyIPv6Addresses)
	source, destination := device.As16(), to.Addr().As16()
	header = append(header, source[:]...)
	header = append(header, destination[:]...)
	header = binary.BigEndian.AppendUint16(header, sourcePort)
	return binary.BigEndian.AppendUint16(header, to.Port())
}

func addrPortOf(address net.Addr) netip.AddrPort {
	if tcp, ok := address.(*net.TCPAddr); ok {
		return tcp.AddrPort()
	}
	return netip.AddrPort{}
}
