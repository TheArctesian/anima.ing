package devicelink

import (
	"bufio"
	"context"
	"io"
	"net"
	"strings"
	"time"

	"github.com/TheArctesian/anima/link/dial"
	"github.com/TheArctesian/anima/link/presence"
	"github.com/TheArctesian/anima/link/tunnel"
)

// A hub older than the reverse port refuses the presence stream; the device retries quietly, as nothing else uses it.

const (
	presenceRetryFirst = 5 * time.Second
	presenceRetryLimit = 60 * time.Second
	// maxLocalConnections bounds the hub's connections to local ports; a request above it gets no answer.
	maxLocalConnections = 64
)

func (l *Link) stayPresent(ctx context.Context, every time.Duration) {
	defer l.done.Done()
	wait := presenceRetryFirst
	for {
		if l.present(ctx, every) {
			wait = presenceRetryFirst
		}
		select {
		case <-ctx.Done():
			return
		case <-l.presentNow:
			wait = presenceRetryFirst
			continue
		case <-time.After(wait):
		}
		wait = min(2*wait, presenceRetryLimit)
	}
}

// present reports whether the stream stayed open for one keepalive, which shows that the hub has the reverse port.
func (l *Link) present(ctx context.Context, every time.Duration) (held bool) {
	dialCtx, cancel := context.WithTimeout(ctx, dialLimit)
	stream, err := l.tunnel().Dial(dialCtx, tunnel.ReversePort)
	cancel()
	if err != nil {
		return false
	}
	l.setReverse(stream)
	ended := make(chan struct{})
	go func() {
		defer close(ended)
		l.readRequests(ctx, stream)
	}()
	defer func() {
		stream.Close()
		<-ended
	}()
	ticker := time.NewTicker(every)
	defer ticker.Stop()
	for {
		_ = stream.SetWriteDeadline(time.Now().Add(l.pingLimit))
		if _, err := io.WriteString(stream, presence.Here); err != nil {
			return held
		}
		select {
		case <-ctx.Done():
			return held
		case <-ended:
			return held
		case <-ticker.C:
			held = true
		}
	}
}

func (l *Link) readRequests(ctx context.Context, stream net.Conn) {
	lines := bufio.NewReaderSize(stream, presence.MaxLine+1)
	for {
		line, err := lines.ReadSlice('\n')
		if err != nil {
			return
		}
		request, asked := dial.ParseRequest(strings.TrimSuffix(string(line), "\n"))
		if !asked {
			continue
		}
		select {
		case l.giving <- struct{}{}:
			go func() {
				defer func() { <-l.giving }()
				l.local().Give(ctx, request)
			}()
		default:
		}
	}
}

func (l *Link) local() dial.Local {
	return dial.Local{IsOpen: l.openPort, Hub: func(ctx context.Context) (net.Conn, error) {
		ctx, cancel := context.WithTimeout(ctx, dialLimit)
		defer cancel()
		return l.tunnel().Dial(ctx, tunnel.OpenPort)
	}}
}

func (l *Link) setReverse(stream net.Conn) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.reverse = stream
}

// presentAgain reopens the stream with no wait after a new registration: the hub forgot the stream,
// and the tunnel reports that only after its own long limit.
func (l *Link) presentAgain() {
	l.mu.Lock()
	stream := l.reverse
	l.mu.Unlock()
	if stream != nil {
		stream.Close()
	}
	select {
	case l.presentNow <- struct{}{}:
	default:
	}
}
