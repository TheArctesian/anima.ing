package dial_test

import (
	"context"
	"errors"
	"io"
	"net"
	"path/filepath"
	"testing"
	"time"

	"github.com/TheArctesian/anima/link/ports"
	"github.com/TheArctesian/anima/link/statedir"
)

func TestTurningOffOnePortClosesItsLiveStreamEvenAfterRapidReenable(t *testing.T) {
	dir, err := statedir.Open(filepath.Join(t.TempDir(), "state"))
	if err != nil {
		t.Fatal(err)
	}
	defer dir.Close()
	choices := ports.Selected(dir)
	for _, port := range []uint16{22, 5900} {
		if err := choices.Turn(port, true); err != nil {
			t.Fatal(err)
		}
	}
	deviceClosed := make(chan struct{}, 2)
	ssh := listen(t, func(conn net.Conn) {
		_, _ = io.Copy(conn, conn)
		_ = conn.Close()
		deviceClosed <- struct{}{}
	})
	vnc, _ := echo(t)
	b := newBench(t, nil)
	// Use disposable listeners for the two actual owner choices.
	b.permit = func(port uint16) context.Context {
		switch int(port) {
		case ssh.Port:
			return choices.Permit(22)
		case vnc:
			return choices.Permit(5900)
		default:
			return nil
		}
	}
	before := openStream(t, b, ssh.Port)
	other := openStream(t, b, vnc)
	exchange(t, before, "ssh before disable")
	exchange(t, other, "vnc before disable")

	for _, on := range []bool{false, true} {
		if err := choices.Turn(22, on); err != nil {
			t.Fatal(err)
		}
	}
	_ = before.SetReadDeadline(time.Now().Add(2 * time.Second))
	var timeout net.Error
	if _, err := before.Read(make([]byte, 1)); err == nil || (errors.As(err, &timeout) && timeout.Timeout()) {
		t.Fatalf("previous SSH stream stayed open: %v", err)
	}
	select {
	case <-deviceClosed:
	case <-time.After(2 * time.Second):
		t.Fatal("the local SSH listener still holds the revoked connection")
	}
	exchange(t, other, "vnc after disable")
	exchange(t, openStream(t, b, ssh.Port), "new ssh after re-enable")
}

func openStream(t *testing.T, b *bench, port int) net.Conn {
	t.Helper()
	stream, release, err := b.open(port, 5*time.Second)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(release)
	t.Cleanup(func() { stream.Close() })
	return stream
}

func TestBundledRDPClosesBothSidesWhenItsPermitEnds(t *testing.T) {
	grant, revoke := context.WithCancel(context.Background())
	t.Cleanup(revoke)
	b := newBench(t, nil)
	b.permit = func(port uint16) context.Context {
		if port == 3389 {
			return grant
		}
		return nil
	}
	hostClosed := make(chan struct{})
	b.rdp = func(context.Context) (net.Conn, error) {
		device, host := net.Pipe()
		go func() {
			defer close(hostClosed)
			defer host.Close()
			_, _ = io.Copy(host, host)
		}()
		return device, nil
	}

	stream := openStream(t, b, 3389)
	exchange(t, stream, "through the bundled host")
	revoke()
	_ = stream.SetReadDeadline(time.Now().Add(2 * time.Second))
	var timeout net.Error
	if _, err := stream.Read(make([]byte, 1)); err == nil || (errors.As(err, &timeout) && timeout.Timeout()) {
		t.Fatalf("the hub retained the revoked RDP stream: %v", err)
	}
	select {
	case <-hostClosed:
	case <-time.After(2 * time.Second):
		t.Fatal("the bundled host retained the revoked RDP stream")
	}
}
