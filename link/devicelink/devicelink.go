// Package devicelink is the device side of the link: local ways to the hub, a keepalive and the presence stream.
package devicelink

import (
	"context"
	"errors"
	"net"
	"net/netip"
	"sync"
	"time"

	"github.com/TheArctesian/anima/link/identity"
	"github.com/TheArctesian/anima/link/tunnel"
)

var (
	// ErrListen means that the local address is not on loopback: the local port gives the hub to whoever reaches it.
	ErrListen = errors.New("devicelink: the local address must be a loopback address and a port, for example 127.0.0.1:0")
	// ErrNotEnrolled means that the device has no hub address yet.
	ErrNotEnrolled = errors.New("devicelink: this device has no hub; it needs an invite")
)

const (
	// DefaultKeepalive keeps the link ready: a link idle for 150 s stalls at the next request (spike K-A, 3.6).
	DefaultKeepalive = 15 * time.Second
	// DefaultNoticeEvery makes the device read the hub's next address each 5 minutes with the default keepalive.
	DefaultNoticeEvery = 20
	// DefaultAnswerLimit is how long the device waits for one answer of the hub.
	DefaultAnswerLimit = 5 * time.Second
	dialLimit          = 15 * time.Second
)

// Config is what a device link needs to start.
type Config struct {
	Identity identity.Device
	// Listen is the loopback address of the local port. Port 0 takes a free port.
	Listen string
	// DaemonSocket is a Unix socket for the metrics daemon, whose token stays off a port (K8 Section 1).
	// The link closes it. Nil means none.
	DaemonSocket net.Listener
	// Keepalive is the time between pings. Zero means DefaultKeepalive.
	Keepalive time.Duration
	// AnswerLimit is the wait for one answer of the hub; a registration gets twice this. Zero means DefaultAnswerLimit.
	AnswerLimit time.Duration
	// Save keeps the device's identity when a renewal changes it (LNK-005). Nil means that the key is not renewed.
	Save func(identity.Device) error
	// NoticeEvery is the keepalives between two reads of the hub's next address (LNK-007). Zero means DefaultNoticeEvery.
	NoticeEvery int
	// LookupTXT reads the TXT records of the identity's HubName (LNK-007). Nil means the system's resolver.
	LookupTXT func(ctx context.Context, name string) ([]string, error)
	// OpenPort reports, for each request, whether the hub may reach a local port (HMD-048, HMD-049). Nil opens none.
	OpenPort func(port uint16) bool
	// Now is the clock. Nil means the system clock.
	Now       func() time.Time
	DebugLogf tunnel.Logf
}

// Link is a running device link.
type Link struct {
	listener  net.Listener
	daemon    net.Listener
	stop      context.CancelFunc
	done      sync.WaitGroup
	save      func(identity.Device) error
	lookupTXT func(ctx context.Context, name string) ([]string, error)
	now       func() time.Time
	debugLogf tunnel.Logf

	// beats and failures belong to the keepalive goroutine.
	noticeEvery, beats, failures int
	pingLimit, registerLimit     time.Duration

	mu         sync.Mutex
	device     *tunnel.Device
	saved      identity.Device
	path       tunnel.Path
	reverse    net.Conn
	presentNow chan struct{}
	openPort   func(port uint16) bool
	giving     chan struct{}
}

func (l *Link) tunnel() *tunnel.Device {
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.device
}

func (l *Link) identity() identity.Device {
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.saved
}

func (l *Link) setIdentity(device identity.Device) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.saved = device
}

func (l *Link) replaceTunnel(next *tunnel.Device, renewed identity.Device) (old *tunnel.Device) {
	l.mu.Lock()
	defer l.mu.Unlock()
	old, l.device, l.saved = l.device, next, renewed
	return old
}

// Start opens the local port and starts the keepalive and the presence stream without waiting for the hub.
// A start that fails closes Config.DaemonSocket.
func Start(cfg Config) (*Link, error) {
	link, err := start(cfg)
	if err != nil && cfg.DaemonSocket != nil {
		cfg.DaemonSocket.Close()
	}
	return link, err
}

func start(cfg Config) (*Link, error) {
	local, err := netip.ParseAddrPort(cfg.Listen)
	if err != nil || !local.Addr().IsLoopback() {
		return nil, ErrListen
	}
	if cfg.Identity.Hub.IsZero() {
		return nil, ErrNotEnrolled
	}
	device, err := tunnel.NewDevice(tunnel.DeviceConfig{Key: cfg.Identity.Key, Hub: cfg.Identity.Hub, DebugLogf: cfg.DebugLogf})
	if err != nil {
		return nil, err
	}
	listener, err := net.Listen("tcp", local.String())
	if err != nil {
		device.Close()
		return nil, err
	}
	ctx, stop := context.WithCancel(context.Background())
	link := &Link{
		device: device, saved: cfg.Identity, listener: listener, daemon: cfg.DaemonSocket, stop: stop, path: tunnel.Path{Kind: tunnel.PathNone},
		save: cfg.Save, now: cfg.Now, debugLogf: cfg.DebugLogf, presentNow: make(chan struct{}, 1),
		openPort: cfg.OpenPort, giving: make(chan struct{}, maxLocalConnections),
	}
	if link.now == nil {
		link.now = time.Now
	}
	link.lookupTXT = cfg.LookupTXT
	if link.lookupTXT == nil {
		link.lookupTXT = net.DefaultResolver.LookupTXT
	}
	link.pingLimit = cfg.AnswerLimit
	if link.pingLimit <= 0 {
		link.pingLimit = DefaultAnswerLimit
	}
	link.registerLimit = 2 * link.pingLimit
	link.noticeEvery = max(cfg.NoticeEvery, 0)
	if link.noticeEvery == 0 {
		link.noticeEvery = DefaultNoticeEvery
	}
	every := cfg.Keepalive
	if every <= 0 {
		every = DefaultKeepalive
	}
	link.done.Add(3)
	go link.keepAlive(ctx, every)
	go link.stayPresent(ctx, every)
	go link.accept(ctx, listener)
	if link.daemon != nil {
		link.done.Add(1)
		go link.accept(ctx, link.daemon)
	}
	return link, nil
}

// LocalAddr is the local port that reaches the hub.
func (l *Link) LocalAddr() net.Addr { return l.listener.Addr() }

// Path is the result of the last keepalive (LNK-006).
func (l *Link) Path() tunnel.Path {
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.path
}

// Close stops the device link and its streams.
func (l *Link) Close() error {
	l.stop()
	l.listener.Close()
	if l.daemon != nil {
		l.daemon.Close()
	}
	err := l.tunnel().Close()
	l.done.Wait()
	return err
}

// keepAlive registers again when the hub does not answer: a restarted hub forgot the device, and the library
// registers only at the first use (spike K-A, 3.2).
func (l *Link) keepAlive(ctx context.Context, every time.Duration) {
	defer l.done.Done()
	ticker := time.NewTicker(every)
	defer ticker.Stop()
	for {
		l.beat(ctx)
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
		}
	}
}

func (l *Link) beat(ctx context.Context) {
	path, err := l.ping(ctx)
	if err != nil {
		registerCtx, cancel := context.WithTimeout(ctx, l.registerLimit)
		registered := l.tunnel().Register(registerCtx) == nil
		cancel()
		if registered {
			path, _ = l.ping(ctx)
			l.presentAgain()
		}
	}
	l.mu.Lock()
	l.path = path
	l.mu.Unlock()
	if path.Kind == tunnel.PathNone {
		l.failures++
		if l.failures >= failuresBeforeNextHub {
			l.followHub(ctx)
		}
		return
	}
	l.failures = 0
	l.renewIfDue(ctx)
	if l.beats%l.noticeEvery == 0 {
		l.readNotice(ctx)
	}
	l.beats++
}

func (l *Link) ping(ctx context.Context) (tunnel.Path, error) {
	ctx, cancel := context.WithTimeout(ctx, l.pingLimit)
	defer cancel()
	return l.tunnel().Path(ctx)
}

func (l *Link) accept(ctx context.Context, listener net.Listener) {
	defer l.done.Done()
	for {
		local, err := listener.Accept()
		if err != nil {
			return
		}
		go l.forward(ctx, local)
	}
}

func (l *Link) forward(ctx context.Context, local net.Conn) {
	ctx, cancel := context.WithTimeout(ctx, dialLimit)
	defer cancel()
	hub, err := l.tunnel().Dial(ctx, tunnel.HubPort)
	if err != nil {
		local.Close()
		return
	}
	tunnel.Join(local, hub)
}
