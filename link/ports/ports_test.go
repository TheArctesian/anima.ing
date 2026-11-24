package ports_test

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"testing"

	"github.com/TheArctesian/anima/link/ports"
	"github.com/TheArctesian/anima/link/rdppeer"
	"github.com/TheArctesian/anima/link/statedir"
	"github.com/TheArctesian/anima/link/testpath"
)

func open(t *testing.T) *statedir.Dir {
	t.Helper()
	dir, err := statedir.Open(filepath.Join(t.TempDir(), "s"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { dir.Close() })
	return dir
}

func listed(t *testing.T, dir *statedir.Dir) []uint16 {
	t.Helper()
	list, err := ports.List(dir)
	if err != nil {
		t.Fatal(err)
	}
	return list
}

func TestHMD049TheListIsEmptyUntilAPortIsTurnedOnAndAPortCanBeTurnedOff(t *testing.T) {
	dir := open(t)
	if got := listed(t, dir); len(got) != 0 || ports.IsOpen(dir, 22) {
		t.Fatalf("a new device has the open ports %v", got)
	}
	for _, port := range []uint16{5900, 22, 22} {
		if err := ports.Turn(dir, port, true); err != nil {
			t.Fatal(err)
		}
	}
	if got := listed(t, dir); !slices.Equal(got, []uint16{22, 5900}) {
		t.Fatalf("the list is %v, want 22 and 5900", got)
	}
	if err := ports.Turn(dir, 22, false); err != nil {
		t.Fatal(err)
	}
	if ports.IsOpen(dir, 22) || !ports.IsOpen(dir, 5900) {
		t.Fatalf("after 22 was turned off the list is %v", listed(t, dir))
	}
}

func TestHMD049OnlyThePortsOfTheSessionsCanBeOpen(t *testing.T) {
	dir := open(t)
	for _, port := range []uint16{80, 443, 5432, 0} {
		if err := ports.Turn(dir, port, true); !errors.Is(err, ports.ErrNotAllowed) {
			t.Errorf("port %d: %v, want ErrNotAllowed", port, err)
		}
	}
	if !ports.Allowed(3389, "windows") || !ports.Allowed(3389, "linux") {
		t.Fatal("port 3389 must remain available on Windows and Linux")
	}
	if ports.Allowed(3389, "darwin") && (runtime.GOOS != "darwin" || runtime.GOARCH != "arm64") {
		t.Fatal("macOS RDP must be limited to Apple Silicon")
	}
	if err := ports.Turn(dir, 3389, true); (err == nil) != (runtime.GOOS == "windows" || runtime.GOOS == "linux") {
		t.Fatalf("port 3389 on %s: %v", runtime.GOOS, err)
	}
}

func TestHMD049APortThatAHandWroteInTheFileIsNotOpen(t *testing.T) {
	dir := open(t)
	if err := dir.Write("open-ports", []uint16{22, 80, 8080, 22}); err != nil {
		t.Fatal(err)
	}
	if got := listed(t, dir); !slices.Equal(got, []uint16{22}) {
		t.Fatalf("the list is %v, want only 22", got)
	}
	if ports.IsOpen(dir, 80) {
		t.Fatal("port 80 is open")
	}
}

func TestHMD049AChangeBesideTheRunningLinkAppliesAtOnce(t *testing.T) {
	path := filepath.Join(t.TempDir(), "s")
	held, err := statedir.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer held.Close()
	beside, err := statedir.Beside(path)
	if err != nil {
		t.Fatalf("Beside: %v", err)
	}
	defer beside.Close()
	if err := ports.Turn(beside, 5900, true); err != nil {
		t.Fatal(err)
	}
	if !ports.IsOpen(held, 5900) {
		t.Fatal("the link does not see the port that the second process turned on")
	}
}

func TestMRDP004SameUserSocketCannotEnableBundledRDP(t *testing.T) {
	if !rdppeer.Supported {
		t.Skip("Apple Silicon macOS only")
	}
	path, err := testpath.MkdirTemp("anima-rdp")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.RemoveAll(path) })
	dir, err := statedir.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer dir.Close()

	if err := ports.Turn(dir, 3389, true); !errors.Is(err, ports.ErrHostNotReady) || ports.IsOpen(dir, 3389) {
		t.Fatalf("opt-in without a bundled host must fail, got %v", err)
	}
	listener, err := dir.Listen("rdp.sock")
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	if ports.IsOpen(dir, 3389) {
		t.Fatal("starting the host must not opt in to the Link route")
	}
	if err := ports.Turn(dir, 3389, true); !errors.Is(err, ports.ErrHostNotReady) || ports.IsOpen(dir, 3389) {
		t.Fatalf("the replacement socket gained an RDP route: %v", err)
	}
}
