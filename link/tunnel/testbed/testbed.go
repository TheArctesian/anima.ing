// Package testbed gives tests a relay on loopback, so that no link test needs the internet.
package testbed

import (
	"context"
	"crypto/tls"
	"encoding/json"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"

	"github.com/TheArctesian/anima/link/tunnel"
	"tailscale.com/derp/derpserver"
	"tailscale.com/envknob"
	"tailscale.com/net/netcheck"
	"tailscale.com/syncs"
	"tailscale.com/tailcfg"
	"tailscale.com/tstest/integration"
	"tailscale.com/types/key"
	"tailscale.com/types/logger"
)

// Main is the TestMain of each package that starts a tunnel; it stops the library's outside network checks.
func Main(m *testing.M) {
	envknob.Setenv("IN_TS_TEST", "true")
	netcheck.HookStartCaptivePortalDetection.SetForTest(func(context.Context, *netcheck.Client, *tailcfg.DERPMap, tailcfg.DERPRegionID, func(bool)) (<-chan struct{}, func()) {
		return syncs.ClosedChan(), func() {}
	})
	os.Exit(m.Run())
}

// StartRelay starts a relay on loopback for one test.
func StartRelay(t testing.TB) tunnel.Relay {
	t.Helper()
	regions := integration.RunDERPAndSTUN(t, logger.Discard, "127.0.0.1").Regions
	text, err := json.Marshal(regions[1])
	if err != nil {
		t.Fatal(err)
	}
	var relay tunnel.Relay
	if err := json.Unmarshal(text, &relay); err != nil {
		t.Fatal(err)
	}
	return relay
}

// StartRefusingRelay starts a relay on loopback that refuses each key, like a relay of another network.
func StartRefusingRelay(t testing.TB) tunnel.Relay {
	t.Helper()
	refuse := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_ = json.NewEncoder(w).Encode(tailcfg.DERPAdmitClientResponse{Allow: false})
	}))
	t.Cleanup(refuse.Close)
	relay := derpserver.New(key.NewNode(), logger.Discard)
	relay.SetVerifyClientURL(refuse.URL)
	t.Cleanup(func() { relay.Close() })
	server := httptest.NewUnstartedServer(derpserver.Handler(relay))
	server.Config.TLSNextProto = map[string]func(*http.Server, *tls.Conn, http.Handler){}
	server.StartTLS()
	t.Cleanup(server.Close)

	port := server.Listener.Addr().(*net.TCPAddr).Port
	region := &tailcfg.DERPRegion{RegionID: 1, RegionCode: "refusing", Nodes: []*tailcfg.DERPNode{{
		Name: "r1", RegionID: 1, HostName: "127.0.0.1", IPv4: "127.0.0.1", IPv6: "none",
		STUNPort: -1, DERPPort: port, InsecureForTests: true,
	}}}
	text, err := json.Marshal(region)
	if err != nil {
		t.Fatal(err)
	}
	var out tunnel.Relay
	if err := json.Unmarshal(text, &out); err != nil {
		t.Fatal(err)
	}
	return out
}

// ForceRelay sends all traffic through the relay; call it before a hub or device starts, outside parallel tests.
func ForceRelay(t testing.TB) {
	t.Helper()
	envknob.Setenv("TS_DEBUG_ALWAYS_USE_DERP", "true")
	t.Cleanup(func() { envknob.Setenv("TS_DEBUG_ALWAYS_USE_DERP", "") })
}
