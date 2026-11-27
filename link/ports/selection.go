package ports

import (
	"context"
	"sync"

	"github.com/TheArctesian/anima/link/statedir"
)

// Selection owns the saved port choices while the device link is running.
type Selection struct {
	dir     *statedir.Dir
	mu      sync.Mutex
	permits map[uint16]permit
}

type permit struct {
	ctx    context.Context
	cancel context.CancelFunc
}

// Selected uses the link's exclusively held state directory.
func Selected(dir *statedir.Dir) *Selection {
	return &Selection{dir: dir, permits: make(map[uint16]permit)}
}

// Permit returns nil if this port is closed. An open port's context ends when the owner turns it off.
func (s *Selection) Permit(port uint16) context.Context {
	s.mu.Lock()
	defer s.mu.Unlock()
	if !IsOpen(s.dir, port) {
		return nil
	}
	held, exists := s.permits[port]
	if !exists {
		held.ctx, held.cancel = context.WithCancel(context.Background())
		s.permits[port] = held
	}
	return held.ctx
}

// Turn saves the owner's choice. Turning a port back on does not restore the old permit.
func (s *Selection) Turn(port uint16, on bool) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	err := Turn(s.dir, port, on)
	if !on {
		if held, exists := s.permits[port]; exists {
			held.cancel()
			delete(s.permits, port)
		}
	}
	return err
}
