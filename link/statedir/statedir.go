// Package statedir keeps the files of one anima-link in an owner-only directory, one process at a time (LNK-008).
package statedir

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"net"
	"os"
	"path/filepath"
	"time"
)

// Errors of this package. None of them holds the content of a file.
var (
	// ErrInUse means that another anima-link has the directory.
	ErrInUse = errors.New("statedir: another anima-link uses this directory")
	// ErrExposed means that other users can reach or replace the directory or a file.
	ErrExposed = errors.New("statedir: other users can reach this path; set owner-only access")
	// ErrDamaged means that a file cannot be read as what it must hold.
	ErrDamaged = errors.New("statedir: the file is damaged")
	// ErrAbsent means that the file was never written.
	ErrAbsent = errors.New("statedir: no such file")
	// ErrLongPath means that the directory's path leaves no room for a socket name.
	ErrLongPath = errors.New("statedir: the path is too long for a socket; use a shorter state directory")
)

// maxSocketPath is the smaller of the macOS and Linux limits, less the end byte.
const maxSocketPath = 103

const lockName = "lock"

// Dir is an open state directory. Close it to let another process use it.
type Dir struct {
	path string
	lock *os.File
}

// Open makes the directory if it is absent, checks that only its owner can reach it, and takes it for this process.
func Open(path string) (*Dir, error) {
	path, err := filepath.Abs(path)
	if err != nil {
		return nil, err
	}
	if err := makeDirectory(path); err != nil {
		return nil, fmt.Errorf("statedir: make the directory: %w", err)
	}
	if err := private(path); err != nil {
		return nil, err
	}
	// Only now: the directory is this user's own and nobody else reaches it.
	if err := dropInheritedAccess(path); err != nil {
		return nil, err
	}
	lock, err := os.OpenFile(filepath.Join(path, lockName), os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return nil, err
	}
	if err := lockFile(lock); err != nil {
		lock.Close()
		return nil, fmt.Errorf("%w: %s", ErrInUse, path)
	}
	return &Dir{path: path, lock: lock}, nil
}

// Beside opens the directory of an anima-link that may run now, without taking it; it must pass the test of Open.
func Beside(path string) (*Dir, error) {
	path, err := filepath.Abs(path)
	if err != nil {
		return nil, err
	}
	if err := private(path); err != nil {
		return nil, err
	}
	return &Dir{path: path}, nil
}

// Close gives the directory back.
func (d *Dir) Close() error {
	if d.lock == nil {
		return nil
	}
	return d.lock.Close()
}

// Read fills value from the JSON file name. It returns ErrAbsent when the file was never written.
func (d *Dir) Read(name string, value any) error {
	file, err := readFile(filepath.Join(d.path, name))
	if errors.Is(err, fs.ErrNotExist) {
		return fmt.Errorf("%w: %s", ErrAbsent, name)
	}
	if err != nil {
		return fmt.Errorf("%w: %s", ErrExposed, name)
	}
	defer file.Close()
	// The check and the read use the same open file, so the file cannot be replaced between them.
	stat, err := file.Stat()
	if err != nil {
		return err
	}
	if !stat.Mode().IsRegular() || !ownerOnly(stat, filepath.Join(d.path, name)) {
		return fmt.Errorf("%w: %s", ErrExposed, name)
	}
	text, err := io.ReadAll(file)
	if err != nil {
		return err
	}
	if err := json.Unmarshal(text, value); err != nil {
		// The cause can quote the file, and the file can hold a key.
		return fmt.Errorf("%w: %s", ErrDamaged, name)
	}
	return nil
}

// Write replaces the JSON file name with value. A reader sees the old file or the new file, never a part of one.
func (d *Dir) Write(name string, value any) error {
	text, err := json.Marshal(value)
	if err != nil {
		return err
	}
	temporary, err := os.CreateTemp(d.path, name+".*")
	if err != nil {
		return err
	}
	defer os.Remove(temporary.Name())
	if _, err := temporary.Write(text); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Sync(); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	if err := os.Rename(temporary.Name(), filepath.Join(d.path, name)); err != nil {
		return err
	}
	return d.sync()
}

// sync makes a rename last through a power loss, or a hub could come back as another hub.
func (d *Dir) sync() error {
	directory, err := os.Open(d.path)
	if err != nil {
		return err
	}
	defer directory.Close()
	return syncDirectory(directory)
}

// Listen makes the owner-only Unix socket name in the directory, and replaces one that an earlier process left.
func (d *Dir) Listen(name string) (net.Listener, error) {
	path := filepath.Join(d.path, name)
	if len(path) > maxSocketPath {
		return nil, ErrLongPath
	}
	if err := os.Remove(path); err != nil && !errors.Is(err, fs.ErrNotExist) {
		return nil, err
	}
	return listenOwnerOnly(path)
}

func listenOwnerOnly(path string) (net.Listener, error) {
	listener, err := net.Listen("unix", path)
	if err != nil {
		return nil, err
	}
	if err := os.Chmod(path, 0o600); err != nil {
		listener.Close()
		return nil, err
	}
	return listener, nil
}

// Dial connects to the socket name of the state directory at path, without taking the directory.
func Dial(ctx context.Context, path, name string) (net.Conn, error) {
	var dialer net.Dialer
	return dialer.DialContext(ctx, "unix", filepath.Join(path, name))
}

// DialOwned joins the owner-only socket named inside the directory.
func (d *Dir) DialOwned(ctx context.Context, name string) (net.Conn, error) {
	return dialOwned(ctx, filepath.Join(d.path, name))
}

// DialOwned connects to a Unix socket of this user's other program in a private folder.
// It refuses a socket that another user owns or can replace; in a private folder only this user or root can swap it after the check.
func DialOwned(path string, limit time.Duration) (net.Conn, error) {
	ctx, cancel := context.WithTimeout(context.Background(), limit)
	defer cancel()
	return dialOwned(ctx, path)
}

func dialOwned(ctx context.Context, path string) (net.Conn, error) {
	if err := private(filepath.Dir(path)); err != nil {
		return nil, err
	}
	stat, err := os.Lstat(path)
	if err != nil {
		return nil, err
	}
	if stat.Mode().Type() != fs.ModeSocket || !ownerOnly(stat, path) {
		return nil, fmt.Errorf("%w: %s", ErrExposed, path)
	}
	var dialer net.Dialer
	return dialer.DialContext(ctx, "unix", path)
}

// private refuses a symbolic link, or a directory that other users can reach or replace.
func private(path string) error {
	stat, err := os.Lstat(path)
	if err != nil {
		return err
	}
	if !stat.IsDir() || !ownerOnly(stat, path) {
		return fmt.Errorf("%w: %s", ErrExposed, path)
	}
	return foldersAbove(path)
}

// ListenOwned makes an owner-only Unix socket at path in a private folder, for this user's other program (K8 Section 1).
// It replaces a socket that an earlier process left, and refuses a path that holds something else.
func ListenOwned(path string) (net.Listener, error) {
	path, err := filepath.Abs(path)
	if err != nil {
		return nil, err
	}
	if len(path) > maxSocketPath {
		return nil, ErrLongPath
	}
	if err := private(filepath.Dir(path)); err != nil {
		return nil, err
	}
	stat, err := os.Lstat(path)
	switch {
	case errors.Is(err, fs.ErrNotExist):
	case err != nil:
		return nil, err
	case stat.Mode().Type() != fs.ModeSocket || !ownerOnly(stat, path):
		return nil, fmt.Errorf("%w: %s", ErrExposed, path)
	default:
		if err := os.Remove(path); err != nil {
			return nil, err
		}
	}
	return listenOwnerOnly(path)
}
