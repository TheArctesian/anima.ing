package devices

import (
	"errors"
	"fmt"
	"net/netip"
	"sync"
	"time"

	"github.com/TheArctesian/anima/link/statedir"
	"github.com/TheArctesian/anima/link/tunnel"
)

const fileName = "devices.json"

// Store is the list of permitted devices of a running hub link. Each change is on disk before the call returns.
type Store struct {
	dir *statedir.Dir
	now func() time.Time

	mu      sync.RWMutex
	current list
	retired retired
}

// Open reads the list from the state directory; with no list it refuses every key. Keys expire by now (LNK-005).
func Open(dir *statedir.Dir, now func() time.Time) (*Store, error) {
	var saved []Device
	if err := dir.Read(fileName, &saved); err != nil && !errors.Is(err, statedir.ErrAbsent) {
		return nil, err
	}
	current := list{}
	for _, device := range saved {
		// A list that was written before devices had addresses.
		if !linkRange.Contains(device.Address) {
			device.Address = addressOf(device.Key)
		}
		if current.addressTaken(device.Address) {
			return nil, fmt.Errorf("%w: two devices have one address in %s", statedir.ErrDamaged, fileName)
		}
		current[device.Key] = device
	}
	var gone retired
	if err := dir.Read(retiredFileName, &gone); err != nil && !errors.Is(err, statedir.ErrAbsent) {
		return nil, err
	}
	return &Store{dir: dir, now: now, current: current, retired: gone}, nil
}

// Permit adds a device to the list. A device with no Permitted time gets
// the time of the call.
func (s *Store) Permit(device Device) (Device, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if device.Permitted.IsZero() {
		device.Permitted = s.now().UTC()
	}
	if s.retired.refuses(device.Key) {
		return Device{}, ErrRetired
	}
	next, err := s.current.with(device)
	if err != nil {
		return Device{}, err
	}
	return next[device.Key], s.save(next)
}

// Revoke removes a device with its current and its next key from the list, and returns it (LNK-004).
func (s *Store) Revoke(key tunnel.DeviceKey) (Device, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	next, device, err := s.current.without(key)
	if err != nil {
		return Device{}, err
	}
	return device, s.saveWithout(next, s.retired.withDevice(device))
}

// Renew records the next key of the device that is connected with current
// (LNK-005). The device can then connect with either key.
func (s *Store) Renew(current, next tunnel.DeviceKey) (Device, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.retired.hasKey(next) {
		return Device{}, ErrRetired
	}
	replaced := s.current[current].NextKey
	updated, device, err := s.current.renewing(current, next, s.now())
	if err != nil {
		return Device{}, err
	}
	if replaced == next {
		return device, s.save(updated)
	}
	return device, s.saveWithout(updated, s.retired.withKeys(replaced))
}

// Promote makes a next key the current key of its device; the caller's stream from the next key proves the device holds it.
// For a key that is not a next key it returns nil and changes nothing.
func (s *Store) Promote(next tunnel.DeviceKey) (*Promotion, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	updated, promotion, promoted := s.current.promoted(next, s.now().UTC())
	if !promoted {
		return nil, nil
	}
	return &promotion, s.saveWithout(updated, s.retired.withKeys(promotion.Replaced))
}

// Permits reports whether a key is the current or next key of a device whose key has not expired.
// A connected device stays connected after it leaves the list or expires, until the hub link disconnects it (LNK-004).
func (s *Store) Permits(key tunnel.DeviceKey) bool {
	_, permitted := s.AddressOf(key)
	return permitted
}

// AddressOf returns the link address of the device that a key can connect as now.
func (s *Store) AddressOf(key tunnel.DeviceKey) (netip.Addr, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	device, permitted := s.current.permitted(key, s.now())
	return device.Address, permitted
}

// Count is the number of devices in the list.
func (s *Store) Count() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.current)
}

// All returns the devices in the list, oldest first.
func (s *Store) All() []Device {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.current.sorted()
}

func (s *Store) save(next list) error {
	if err := s.dir.Write(fileName, next.sorted()); err != nil {
		return err
	}
	s.current = next
	return nil
}

// saveWithout writes the retired keys before the list, so a key that left the list cannot come back after a stop.
func (s *Store) saveWithout(next list, gone retired) error {
	if err := s.dir.Write(retiredFileName, gone); err != nil {
		return err
	}
	s.retired = gone
	return s.save(next)
}
