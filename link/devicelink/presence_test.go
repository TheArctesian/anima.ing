package devicelink_test

import (
	"bytes"
	"context"
	"crypto/tls"
	"encoding/pem"
	"errors"
	"fmt"
	"image/png"
	"io"
	"net"
	"os"
	"testing"
	"time"

	"github.com/TheArctesian/anima/link/devicelink/testvnc"
	"github.com/TheArctesian/anima/link/dial"
	"github.com/TheArctesian/anima/link/hublink"
	"github.com/TheArctesian/anima/link/ports"
	"github.com/TheArctesian/anima/link/statedir"
	"github.com/TheArctesian/anima/link/tunnel"
	"golang.org/x/crypto/ssh"
)

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

func TestHMD048TheHubLinkReachesOnlyTheOpenLocalPortsOfADevice(t *testing.T) {
	open, shut := localPort(t, true), localPort(t, false)
	s := newSite(t)
	s.openPort = func(port uint16) context.Context {
		if port == open {
			return context.Background()
		}
		return nil
	}
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

func TestOSPrivateAccess(t *testing.T) {
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
	s.openPort = ports.Selected(dir).Permit
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
	key := osSSHHostKey(t)
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
	checkOSVNC(t, password, func() net.Conn {
		opening, stop := context.WithTimeout(context.Background(), 15*time.Second)
		defer stop()
		stream, release, err := hub.Open(opening, address, 5900)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(release)
		t.Cleanup(func() { stream.Close() })
		return stream
	})
}

func checkOSVNC(t *testing.T, password string, open func() net.Conn) {
	t.Helper()
	frame := testvnc.Check(t, open, osCertificate(t, "/run/anima-test-vnc.pem"), password)
	file, err := os.Create("/run/anima-test-vnc/frame.png")
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	if err := png.Encode(file, frame); err != nil {
		t.Fatal(err)
	}
}

func osCertificate(t *testing.T, path string) []byte {
	t.Helper()
	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	certificate, _ := pem.Decode(contents)
	if certificate == nil || certificate.Type != "CERTIFICATE" || len(certificate.Bytes) == 0 {
		t.Fatal("fixture certificate is missing")
	}
	return certificate.Bytes
}

func osSSHHostKey(t *testing.T) ssh.PublicKey {
	t.Helper()
	keyBytes, err := os.ReadFile("/run/anima-test-ssh.pub")
	if err != nil {
		t.Fatal(err)
	}
	key, _, _, _, err := ssh.ParseAuthorizedKey(keyBytes)
	if err != nil {
		t.Fatal(err)
	}
	return key
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
	certificate := osCertificate(t, "/run/anima-test-rdp.pem")
	// VerifyConnection pins the certificate that the VM exports.
	secure := tls.Client(stream, &tls.Config{MinVersion: tls.VersionTLS12, InsecureSkipVerify: true, VerifyConnection: func(state tls.ConnectionState) error {
		if len(state.PeerCertificates) != 1 || !bytes.Equal(state.PeerCertificates[0].Raw, certificate) {
			return fmt.Errorf("RDP certificate changed")
		}
		return nil
	}})
	if err := secure.Handshake(); err != nil {
		t.Fatal(err)
	}
}
