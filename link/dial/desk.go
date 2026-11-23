package dial

import (
	"context"
	"net"
	"net/netip"
	"sync"
	"time"
)

const (
	// AnswerLimit is how long the dial socket waits for the device.
	AnswerLimit = 10 * time.Second
	lineLimit   = 5 * time.Second
)

// Desk holds each request until its device answers; the zero value is empty.
type Desk struct {
	mu      sync.Mutex
	waiting map[string]*waiter
}

type waiter struct {
	device netip.Addr
	answer chan answer
}

// answer holds the stream only for "ok"; done closes when the hub has finished with it.
type answer struct {
	word   string
	stream net.Conn
	done   chan struct{}
}

// Open asks a device for a local port and waits for its stream; the caller calls release when done.
func (d *Desk) Open(ctx context.Context, device netip.Addr, port uint16, ask func(line string) error) (stream net.Conn, release func(), err error) {
	if port == 0 {
		return nil, nil, ErrBadRequest
	}
	id := newID()
	waits := &waiter{device: device, answer: make(chan answer, 1)}
	d.mu.Lock()
	if d.waiting == nil {
		d.waiting = map[string]*waiter{}
	}
	d.waiting[id] = waits
	d.mu.Unlock()
	if ask(openLine(port, id)) != nil {
		if !d.forget(id) {
			(<-waits.answer).drop()
		}
		return nil, nil, ErrNotConnected
	}
	select {
	case got := <-waits.answer:
		return got.result()
	case <-ctx.Done():
		if d.forget(id) {
			return nil, nil, ErrTimeout
		}
		// Deliver took the request first: its answer comes now.
		return (<-waits.answer).result()
	}
}

// forget removes a request. It reports false when Deliver took it first.
func (d *Desk) forget(id string) bool {
	d.mu.Lock()
	defer d.mu.Unlock()
	_, waits := d.waiting[id]
	delete(d.waiting, id)
	return waits
}

func (d *Desk) take(id string, device netip.Addr) *waiter {
	d.mu.Lock()
	defer d.mu.Unlock()
	waits := d.waiting[id]
	if waits == nil || waits.device != device {
		return nil
	}
	delete(d.waiting, id)
	return waits
}

func (a answer) result() (net.Conn, func(), error) {
	switch a.word {
	case WordOK:
		return a.stream, sync.OnceFunc(a.drop), nil
	case WordRefused:
		return nil, nil, ErrRefused
	default:
		return nil, nil, ErrClosed
	}
}

// drop gives the stream of an answer back to Deliver, which then returns.
func (a answer) drop() {
	if a.done != nil {
		close(a.done)
	}
}

// Deliver gives a device's stream to the request with its id and returns when that request releases it.
// device is the link address that the tunnel authenticated; only the asked device can answer. The caller then closes the stream.
func (d *Desk) Deliver(device netip.Addr, stream net.Conn) {
	_ = stream.SetReadDeadline(time.Now().Add(lineLimit))
	line, read := readLine(stream)
	_ = stream.SetReadDeadline(time.Time{})
	if !read {
		return
	}
	id, word, valid := parseAnswer(line)
	if !valid {
		return
	}
	waits := d.take(id, device)
	if waits == nil {
		return
	}
	got := answer{word: word}
	if word == WordOK {
		got.stream, got.done = stream, make(chan struct{})
	}
	waits.answer <- got
	if got.done != nil {
		<-got.done
	}
}
