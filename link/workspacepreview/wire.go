// Package workspacepreview carries a leased workspace endpoint through the private link.
package workspacepreview

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"regexp"
	"time"
)

// ErrRefused means the workspace preview request or its transport was refused.
var ErrRefused = errors.New("workspace preview unavailable")
var identity = regexp.MustCompile(`^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`)

// Binding identifies the principal, workspace generation and port of a preview grant.
type Binding struct {
	Endpoint   string `json:"endpoint"`
	Principal  string `json:"principal"`
	Grant      string `json:"grant"`
	Workspace  string `json:"workspace"`
	Generation uint64 `json:"generation"`
	Port       uint16 `json:"port"`
	ExpiresAt  uint64 `json:"expires_at"`
}

// Open requests one preview connection within its delivery deadline.
type Open struct {
	Binding    Binding   `json:"binding"`
	ValidUntil time.Time `json:"valid_until"`
}

// Valid checks the workspace preview identity and deadlines against now.
func (o Open) Valid(now time.Time) bool {
	for _, id := range []string{o.Binding.Endpoint, o.Binding.Principal, o.Binding.Grant, o.Binding.Workspace} {
		if !identity.MatchString(id) || id == "00000000-0000-0000-0000-000000000000" {
			return false
		}
	}
	return o.Binding.Generation > 0 && o.Binding.Generation <= 9007199254740991 && o.Binding.Port > 0 &&
		o.Binding.ExpiresAt > uint64(now.Unix()) && o.Binding.ExpiresAt <= uint64(now.Add(10*time.Minute).Unix()) &&
		o.ValidUntil.After(now) && !o.ValidUntil.After(now.Add(15*time.Second))
}

// Encode writes a valid workspace preview request for the link.
func Encode(o Open) (string, error) {
	if !o.Valid(time.Now()) {
		return "", ErrRefused
	}
	raw, err := json.Marshal(o)
	if err != nil || len(raw) > 640 {
		return "", ErrRefused
	}
	return base64.RawURLEncoding.EncodeToString(raw), nil
}

// Decode reads and validates a workspace preview request from the link.
func Decode(text string) (Open, error) {
	var o Open
	if len(text) > 856 {
		return o, ErrRefused
	}
	raw, err := base64.RawURLEncoding.Strict().DecodeString(text)
	if err != nil {
		return o, ErrRefused
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&o) != nil || decoder.Decode(new(any)) != io.EOF || !o.Valid(time.Now()) {
		return Open{}, ErrRefused
	}
	return o, nil
}
