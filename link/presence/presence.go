// Package presence is the reverse-port protocol: a device's "here" lines (HMD-047), and the hub's dial lines (HMD-048).
package presence

import (
	"bufio"
	"errors"
	"io"
	"net"
	"net/netip"
	"sync"
	"time"
)

// ErrNotConnected means that the device has no stream on the reverse port.
var ErrNotConnected = errors.New("presence: the device is not connected")

const (
	// Here is the line that a device writes.
	Here = "here\n"
	// MaxLine is the longest line of either side, without its newline; a longer line ends the stream.
	MaxLine = 1024
	// Silence is how long a stream can be silent before the hub link closes it: three missed lines.
	Silence    = 45 * time.Second
	writeLimit = 5 * time.Second
)

// Book holds each device's one stream and the time of its last line, by link address. The zero value is empty.
type Book struct {
	mu      sync.Mutex
	streams map[netip.Addr]net.Conn
	seen    map[netip.Addr]time.Time
	// writing keeps the lines of the hub whole when two of them go at once.
	writing sync.Mutex
}

// Hold makes stream the device's stream and returns the one that it replaces, which the caller closes.
func (b *Book) Hold(device netip.Addr, stream net.Conn) (replaced net.Conn) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.streams == nil {
		b.streams = map[netip.Addr]net.Conn{}
	}
	replaced = b.streams[device]
	b.streams[device] = stream
	return replaced
}

// Release forgets the stream of a device, unless a newer one replaced it.
func (b *Book) Release(device netip.Addr, stream net.Conn) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.streams[device] == stream {
		delete(b.streams, device)
	}
}

// Saw records the time of a line of a device.
func (b *Book) Saw(device netip.Addr, at time.Time) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.seen == nil {
		b.seen = map[netip.Addr]time.Time{}
	}
	b.seen[device] = at
}

// Seen is the time of the last line of a device.
func (b *Book) Seen(device netip.Addr) (time.Time, bool) {
	b.mu.Lock()
	defer b.mu.Unlock()
	at, seen := b.seen[device]
	return at, seen
}

// Write writes one line of the hub to a device. On a failed write it closes the stream and returns ErrNotConnected.
func (b *Book) Write(device netip.Addr, line string) error {
	b.mu.Lock()
	stream := b.streams[device]
	b.mu.Unlock()
	if stream == nil {
		return ErrNotConnected
	}
	b.writing.Lock()
	defer b.writing.Unlock()
	_ = stream.SetWriteDeadline(time.Now().Add(writeLimit))
	if _, err := io.WriteString(stream, line); err != nil {
		stream.Close()
		return ErrNotConnected
	}
	return nil
}

// Read calls here for each "here" line, and returns at the end of the stream, after Silence, or on any other line.
func Read(stream net.Conn, here func()) {
	lines := bufio.NewReaderSize(stream, MaxLine+1)
	for {
		_ = stream.SetReadDeadline(time.Now().Add(Silence))
		line, err := lines.ReadSlice('\n')
		if err != nil || string(line) != Here {
			return
		}
		here()
	}
}
