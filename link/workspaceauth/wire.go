// Package workspaceauth carries human provider sign-in through the private link.
package workspaceauth

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"regexp"
	"time"
)

// ErrRefused means the provider sign-in request or its transport was refused.
var ErrRefused = errors.New("workspace sign-in unavailable")
var identity = regexp.MustCompile(`^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`)

// Login carries transient authorization; challenges flow only on the opened stream.
type Login struct {
	Session    string    `json:"session"`
	Principal  string    `json:"principal"`
	Workspace  string    `json:"workspace"`
	Generation uint64    `json:"generation"`
	Provider   string    `json:"provider"`
	ValidUntil time.Time `json:"valid_until"`
	ExpiresAt  time.Time `json:"expires_at"`
}

// Valid checks the provider sign-in identity and deadlines against now.
func (o Login) Valid(now time.Time) bool {
	for _, id := range []string{o.Session, o.Principal, o.Workspace} {
		if !identity.MatchString(id) || id == "00000000-0000-0000-0000-000000000000" {
			return false
		}
	}
	return (o.Provider == "codex" || o.Provider == "claude") && o.Generation > 0 && o.Generation <= 9007199254740991 &&
		o.ValidUntil.After(now) && !o.ValidUntil.After(now.Add(15*time.Second)) &&
		!o.ExpiresAt.Before(o.ValidUntil) && !o.ExpiresAt.After(now.Add(10*time.Minute))
}

// Encode writes a valid provider sign-in request for the link.
func Encode(o Login) (string, error) {
	if !o.Valid(time.Now()) {
		return "", ErrRefused
	}
	raw, err := json.Marshal(o)
	if err != nil || len(raw) > 640 {
		return "", ErrRefused
	}
	return base64.RawURLEncoding.EncodeToString(raw), nil
}

// Decode reads and validates a provider sign-in request from the link.
func Decode(text string) (Login, error) {
	var o Login
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
		return Login{}, ErrRefused
	}
	return o, nil
}
