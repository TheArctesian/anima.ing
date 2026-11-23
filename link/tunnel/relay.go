package tunnel

import (
	"encoding/json"
	"errors"
	"net"
	"strconv"

	"tailscale.com/tailcfg"
)

// ErrBadRelay means that a text does not name a relay.
var ErrBadRelay = errors.New("tunnel: a relay is a host name, or a host name and a port")

// ownRegion is the region number of a relay that is not in Tailscale's map.
const ownRegion = 900

// OwnRelay describes an operator-run relay (LNK-007) from "host" or "host:port"; the port defaults to 443.
func OwnRelay(hostPort string) (Relay, error) {
	host, port := hostPort, 443
	if h, p, err := net.SplitHostPort(hostPort); err == nil {
		number, err := strconv.Atoi(p)
		if err != nil || number < 1 || number > 65535 {
			return Relay{}, ErrBadRelay
		}
		host, port = h, number
	}
	if host == "" || net.ParseIP(host) == nil && !ValidHostName(host) {
		return Relay{}, ErrBadRelay
	}
	node := &tailcfg.DERPNode{Name: "own", RegionID: ownRegion, HostName: host, DERPPort: port}
	return Relay{&tailcfg.DERPRegion{RegionID: ownRegion, RegionCode: host, RegionName: host, Nodes: []*tailcfg.DERPNode{node}}}, nil
}

// SameAs reports whether two relays are one relay: the same host and port.
func (r Relay) SameAs(other Relay) bool {
	if r.region == nil || other.region == nil || len(r.region.Nodes) == 0 || len(other.region.Nodes) == 0 {
		return r.region == nil && other.region == nil
	}
	one, two := r.region.Nodes[0], other.region.Nodes[0]
	return one.HostName == two.HostName && one.DERPPort == two.DERPPort
}

// ValidHostName reports whether a text can be a host name.
func ValidHostName(host string) bool {
	if host == "" || len(host) > 253 {
		return false
	}
	for _, r := range host {
		letter := r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9'
		if !letter && r != '-' && r != '.' {
			return false
		}
	}
	return true
}

// Relay is a hub's relay region; the hub address contains it, so a hub saves it to keep its address (LNK-001).
// The zero value means not chosen yet: the hub then takes the nearest region of the default map.
type Relay struct{ region *tailcfg.DERPRegion }

// IsZero reports whether no relay was chosen.
func (r Relay) IsZero() bool { return r.region == nil }

// Name is the relay's name for the operator, or "" when no relay was chosen.
func (r Relay) Name() string {
	if r.region == nil {
		return ""
	}
	return r.region.RegionCode
}

// MarshalJSON gives the form that a state file holds.
func (r Relay) MarshalJSON() ([]byte, error) { return json.Marshal(r.region) }

// UnmarshalJSON reads the form that MarshalJSON gives.
func (r *Relay) UnmarshalJSON(text []byte) error {
	var region *tailcfg.DERPRegion
	if err := json.Unmarshal(text, &region); err != nil {
		return err
	}
	r.region = region
	return nil
}
