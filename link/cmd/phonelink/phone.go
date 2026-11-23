package main

import (
	"context"
	"errors"
	"net"
	"os"
	"runtime"
	"sync"
	"time"

	"github.com/TheArctesian/anima/link/devicelink"
	"github.com/TheArctesian/anima/link/identity"
	"github.com/TheArctesian/anima/link/invites"
	"github.com/TheArctesian/anima/link/statedir"
)

const joinLimit = 30 * time.Second

var errBadInvite = errors.New("phonelink: this is not an invite")

type joined struct {
	Fingerprint string `json:"fingerprint"`
}

type started struct {
	Local string `json:"local"`
}

// linkStatus.Path is "direct", "relayed" or "none" (LNK-006).
type linkStatus struct {
	Running       bool    `json:"running"`
	Local         string  `json:"local,omitempty"`
	Path          string  `json:"path,omitempty"`
	LatencyMS     float64 `json:"latencyMs,omitempty"`
	DeviceKey     string  `json:"deviceKey,omitempty"`
	PublicKey     string  `json:"publicKey,omitempty"`
	MetricsSocket string  `json:"metricsSocket,omitempty"`
}

// failure is the answer of a call that failed: a stable code, never a path, a key or the invite.
type failure struct {
	Error string `json:"error"`
}

var (
	guard       sync.Mutex
	running     *devicelink.Link
	held        *statedir.Dir
	deviceKey   string
	publicKey   string
	metricsPath string
)

func join(state, line string) (joined, error) {
	invite, err := invites.Parse(line)
	if err != nil {
		return joined{}, errBadInvite
	}
	guard.Lock()
	defer guard.Unlock()
	dir, err := statedir.Open(state)
	if err != nil {
		return joined{}, err
	}
	defer dir.Close()
	device, err := identity.LoadDevice(dir)
	if err != nil {
		return joined{}, err
	}
	ctx, cancel := context.WithTimeout(context.Background(), joinLimit)
	defer cancel()
	name, _ := os.Hostname()
	done, err := devicelink.Join(ctx, invite, device, name)
	if err != nil {
		return joined{}, err
	}
	if err := done.Save(dir); err != nil {
		return joined{}, err
	}
	return joined{Fingerprint: done.Key.Public().Fingerprint()}, nil
}

func start(state, listen string) (started, error) {
	guard.Lock()
	defer guard.Unlock()
	if running != nil {
		return started{Local: running.LocalAddr().String()}, nil
	}
	dir, err := statedir.Open(state)
	if err != nil {
		return started{}, err
	}
	device, err := identity.LoadDevice(dir)
	if err != nil {
		_ = dir.Close()
		return started{}, err
	}
	key := device.Key.Public()
	socket := metricsSocket(dir, key.Fingerprint())
	link, err := devicelink.Start(devicelink.Config{Identity: device, Listen: listen, DaemonSocket: socket, Save: func(renewed identity.Device) error {
		return renewed.Save(dir)
	}})
	if err != nil {
		_ = dir.Close()
		return started{}, err
	}
	running, held = link, dir
	if socket != nil {
		metricsPath = socket.Addr().String()
	}
	deviceKey = key.Fingerprint()
	encoded, _ := key.MarshalText() // A populated public key always has a text form.
	publicKey = string(encoded)
	return started{Local: link.LocalAddr().String()}, nil
}

func metricsSocket(dir *statedir.Dir, fingerprint string) net.Listener {
	switch runtime.GOOS {
	case "ios":
		return iosMetricsSocket()
	case "android":
		socket, _ := dir.Listen("metrics-" + fingerprint + ".sock")
		return socket
	default:
		return nil
	}
}

func status() linkStatus {
	guard.Lock()
	defer guard.Unlock()
	if running == nil {
		return linkStatus{}
	}
	path := running.Path()
	return linkStatus{
		Running: true, Local: running.LocalAddr().String(),
		Path: string(path.Kind), LatencyMS: float64(path.Latency.Microseconds()) / 1000,
		DeviceKey: deviceKey, PublicKey: publicKey, MetricsSocket: metricsPath,
	}
}

func stop() {
	guard.Lock()
	defer guard.Unlock()
	metricsPath = ""
	if running == nil {
		return
	}
	_ = running.Close()
	_ = held.Close()
	running, held = nil, nil
	deviceKey = ""
	publicKey = ""
}

// code is the stable code of err that the shell gives the client, as the desktop link's codes.
func code(err error) string {
	switch {
	case errors.Is(err, errBadInvite):
		return "bad-invite"
	case errors.Is(err, devicelink.ErrNoAnswer):
		return "no-answer"
	case errors.Is(err, devicelink.ErrRefused):
		return "refused"
	case errors.Is(err, devicelink.ErrNotEnrolled):
		return "not-joined"
	case errors.Is(err, statedir.ErrInUse), errors.Is(err, statedir.ErrExposed), errors.Is(err, statedir.ErrDamaged),
		errors.Is(err, statedir.ErrLongPath):
		return "folder"
	default:
		return "failed"
	}
}
