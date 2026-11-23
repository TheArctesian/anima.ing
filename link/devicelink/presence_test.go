package devicelink_test

import (
	"bytes"
	"context"
	"crypto/tls"
	"encoding/pem"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"testing"
	"time"

	"github.com/TheArctesian/anima/link/dial"
	"github.com/TheArctesian/anima/link/hublink"
	"github.com/TheArctesian/anima/link/ports"
	"github.com/TheArctesian/anima/link/statedir"
	"github.com/TheArctesian/anima/link/tunnel"
	"golang.org/x/crypto/ssh"
)

// seenBy waits until a hub link has seen the site's device, and returns how
// long it took.
func (s *site) seenBy(t *testing.T, hub *hublink.Link, limit time.Duration) time.Duration {
	t.Helper()
	address, permitted := s.permitted.AddressOf(s.device.Key.Public())
	if !permitted {
		t.Fatal("the device is not in the list")
	}
	start := time.Now()
	for {
		if _, seen := hub.Seen(address); seen {
			return time.Since(start)
		}
		if time.Since(start) > limit {
			t.Fatalf("the hub link did not see the device in %v", limit)
		}
		time.Sleep(100 * time.Millisecond)
	}
}

func TestHMD047ARunningDeviceLinkIsSeenByItsHubAndAgainAfterAHubRestart(t *testing.T) {
	s := newSite(t)
	hub := s.startHub(t)
	device := s.startDevice(t, time.Second)
	s.seenBy(t, hub, 30*time.Second)
	address, _ := s.permitted.AddressOf(s.device.Key.Public())
	first, _ := hub.Seen(address)
	deadline := time.Now().Add(10 * time.Second)
	for {
		if again, _ := hub.Seen(address); again.After(first) {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("the device wrote no second line in 10 s, with a keepalive of 1 s")
		}
		time.Sleep(100 * time.Millisecond)
	}

	hub.Close()
	restarted := s.startHub(t)

	took := s.seenBy(t, restarted, 45*time.Second)
	t.Logf("the restarted hub link saw the device after %v", took.Round(time.Millisecond))
	getUntil(t, device, 30*time.Second)
}

// A hub link that is older than the reverse port serves the hub port only.
func TestHMD047AHubWithNoReversePortStillAnswersThroughTheLocalPort(t *testing.T) {
	s := newSite(t)
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	old, err := tunnel.StartHub(ctx, tunnel.HubConfig{
		Key: s.hub.Key, PresharedKey: s.hub.PresharedKey, Relay: s.hub.Relay,
		Admit: func(tunnel.DeviceKey) bool { return true },
	})
	if err != nil {
		t.Fatalf("StartHub: %v", err)
	}
	t.Cleanup(func() { old.Close() })
	listener, err := old.Listen(tunnel.HubPort)
	if err != nil {
		t.Fatal(err)
	}
	go func() {
		for {
			stream, err := listener.Accept()
			if err != nil {
				return
			}
			go func() {
				target, err := net.Dial("tcp", s.target)
				if err != nil {
					stream.Close()
					return
				}
				tunnel.Join(stream, target)
			}()
		}
	}()
	s.device.Hub = old.Address()
	device := s.startDevice(t, 500*time.Millisecond)

	getUntil(t, device, 30*time.Second)
	// The device has asked for the reverse port and was refused by now.
	time.Sleep(2 * time.Second)
	getUntil(t, device, 10*time.Second)

	closed := make(chan struct{})
	go func() { device.Close(); close(closed) }()
	select {
	case <-closed:
	case <-time.After(5 * time.Second):
		t.Fatal("Close still waits 5 s after the call, with a hub that has no reverse port")
	}
}

// localPort is a port on loopback of this computer. With answers, a program
// on it sends back what it reads; without, each connection waits unread.
func localPort(t *testing.T, answers bool) uint16 {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { listener.Close() })
	if answers {
		go func() {
			for {
				conn, err := listener.Accept()
				if err != nil {
					return
				}
				go func() { _, _ = io.Copy(conn, conn); conn.Close() }()
			}
		}()
	}
	return uint16(listener.Addr().(*net.TCPAddr).Port)
}

// The whole path over a real tunnel: the hub link asks on the device's
// stream of the reverse port, and the device gives a stream to its local
// port, only for a port of its list.
func TestHMD048TheHubLinkReachesOnlyTheOpenLocalPortsOfADevice(t *testing.T) {
	open, shut := localPort(t, true), localPort(t, false)
	s := newSite(t)
	s.openPort = func(port uint16) bool { return port == open }
	hub := s.startHub(t)
	s.startDevice(t, time.Second)
	s.seenBy(t, hub, 30*time.Second)
	address, _ := s.permitted.AddressOf(s.device.Key.Public())
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()

	stream, release, err := hub.Open(ctx, address, open)
	if err != nil {
		t.Fatalf("Open of the open port: %v", err)
	}
	_ = stream.SetDeadline(time.Now().Add(10 * time.Second))
	if _, err := io.WriteString(stream, "from the hub"); err != nil {
		t.Fatal(err)
	}
	got := make([]byte, len("from the hub"))
	if _, err := io.ReadFull(stream, got); err != nil || string(got) != "from the hub" {
		t.Fatalf("the local port answered %q, %v", got, err)
	}
	stream.Close()
	release()

	if _, _, err := hub.Open(ctx, address, shut); !errors.Is(err, dial.ErrRefused) {
		t.Fatalf("Open of a port that is not in the list: %v, want ErrRefused", err)
	}
	if _, _, err := hub.Open(ctx, address.Next(), open); !errors.Is(err, dial.ErrNotConnected) {
		t.Fatalf("Open on a device that is not connected: %v, want ErrNotConnected", err)
	}
}

// This case runs inside the disposable OS VM against its real loopback services.
func TestOSPrivateSSHAndRDPTLS(t *testing.T) {
	password := os.Getenv("ANIMA_OS_TEST_PASSWORD")
	if password == "" {
		t.Skip("requires the disposable Anima OS acceptance VM")
	}
	dir, err := statedir.Beside("/var/lib/anima-link")
	if err != nil {
		t.Fatal(err)
	}
	defer dir.Close()
	s := newSite(t)
	s.openPort = func(port uint16) bool { return ports.IsOpen(dir, port) }
	hub := s.startHub(t)
	s.startDevice(t, time.Second)
	s.seenBy(t, hub, 30*time.Second)
	address, _ := s.permitted.AddressOf(s.device.Key.Public())
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	stream, release, err := hub.Open(ctx, address, 22)
	if err != nil {
		t.Fatal(err)
	}
	defer release()
	defer stream.Close()
	keyBytes, err := os.ReadFile("/run/anima-test-ssh.pub")
	if err != nil {
		t.Fatal(err)
	}
	key, _, _, _, err := ssh.ParseAuthorizedKey(keyBytes)
	if err != nil {
		t.Fatal(err)
	}
	_ = stream.SetDeadline(time.Now().Add(15 * time.Second))
	conn, channels, requests, err := ssh.NewClientConn(stream, "anima-os:22", &ssh.ClientConfig{User: "admin", Auth: []ssh.AuthMethod{ssh.Password(password)}, HostKeyCallback: ssh.FixedHostKey(key)})
	if err != nil {
		t.Fatal(err)
	}
	client := ssh.NewClient(conn, channels, requests)
	defer client.Close()
	session, err := client.NewSession()
	if err != nil {
		t.Fatal(err)
	}
	defer session.Close()
	out, err := session.Output("id -un")
	if err != nil || string(out) != "admin\n" {
		t.Fatalf("private SSH identity %q: %v", out, err)
	}
	desktop, done, err := hub.Open(ctx, address, 3389)
	if err != nil {
		t.Fatal(err)
	}
	defer done()
	defer desktop.Close()
	checkOSRDP(t, desktop)
	if _, _, err := hub.Open(ctx, address, 5900); !errors.Is(err, dial.ErrRefused) {
		t.Fatalf("VNC must stay closed: %v", err)
	}
}

func checkOSRDP(t *testing.T, stream net.Conn) {
	t.Helper()
	_ = stream.SetDeadline(time.Now().Add(15 * time.Second))
	request := []byte{3, 0, 0, 19, 14, 224, 0, 0, 0, 0, 0, 1, 0, 8, 0, 1, 0, 0, 0}
	if _, err := stream.Write(request); err != nil {
		t.Fatal(err)
	}
	response := make([]byte, 19)
	if _, err := io.ReadFull(stream, response); err != nil {
		t.Fatal(err)
	}
	if response[11] != 2 || response[15] != 1 {
		t.Fatalf("RDP did not negotiate TLS: %x", response)
	}
	certBytes, err := os.ReadFile("/run/anima-test-rdp.pem")
	if err != nil {
		t.Fatal(err)
	}
	certificate, _ := pem.Decode(certBytes)
	if certificate == nil {
		t.Fatal("fixture certificate is missing")
	}
	// Pin the certificate exported by the VM owner; never accept an arbitrary peer.
	secure := tls.Client(stream, &tls.Config{MinVersion: tls.VersionTLS12, InsecureSkipVerify: true, VerifyConnection: func(state tls.ConnectionState) error {
		if len(state.PeerCertificates) != 1 || !bytes.Equal(state.PeerCertificates[0].Raw, certificate.Bytes) {
			return fmt.Errorf("RDP certificate changed")
		}
		return nil
	}})
	if err := secure.Handshake(); err != nil {
		t.Fatal(err)
	}
}
