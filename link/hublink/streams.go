package hublink

import (
	"net"
	"sync"

	"github.com/TheArctesian/anima/link/tunnel"
)

// streams records each device's open streams so that the hub link can close them (LNK-004).
type streams struct {
	mu     sync.Mutex
	open   map[tunnel.DeviceKey]map[net.Conn]struct{}
	total  int
	closed bool
	active sync.WaitGroup
}

// Limits on open streams, which each hold a hub connection: one device cannot take all, and all stay under the system's limit.
const (
	MaxStreamsOfDevice = 128
	MaxStreams         = 1024
)

// add returns false when the hub link has stopped or a limit is reached; the caller then closes the stream.
func (s *streams) add(device tunnel.DeviceKey, stream net.Conn) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed || len(s.open[device]) >= MaxStreamsOfDevice || s.total >= MaxStreams {
		return false
	}
	s.total++
	if s.open == nil {
		s.open = map[tunnel.DeviceKey]map[net.Conn]struct{}{}
	}
	if s.open[device] == nil {
		s.open[device] = map[net.Conn]struct{}{}
	}
	s.open[device][stream] = struct{}{}
	s.active.Add(1)
	return true
}

func (s *streams) remove(device tunnel.DeviceKey, stream net.Conn) {
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.open[device], stream)
	if len(s.open[device]) == 0 {
		delete(s.open, device)
	}
	s.total--
	s.active.Done()
}

func (s *streams) count(device tunnel.DeviceKey) int {
	s.mu.Lock()
	defer s.mu.Unlock()
	return len(s.open[device])
}

func (s *streams) closeOf(device tunnel.DeviceKey) int {
	s.mu.Lock()
	defer s.mu.Unlock()
	for stream := range s.open[device] {
		stream.Close()
	}
	return len(s.open[device])
}

func (s *streams) closing() {
	s.mu.Lock()
	s.closed = true
	s.mu.Unlock()
}

func (s *streams) isClosing() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.closed
}

func (s *streams) closeAll() {
	s.mu.Lock()
	s.closed = true
	for _, ofDevice := range s.open {
		for stream := range ofDevice {
			stream.Close()
		}
	}
	s.mu.Unlock()
	s.active.Wait()
}
