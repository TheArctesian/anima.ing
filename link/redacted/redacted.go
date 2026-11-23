// Package redacted keeps the link's secrets out of printing, logging and encoding (LNK-008).
package redacted

import (
	"fmt"
	"log/slog"
)

// Text is what a secret shows as.
const Text = "[secret]"

// Secret makes a struct that embeds it show as Text in fmt, structured logs and text encodings.
// fmt cannot call the methods of an unexported field, so keep secrets in exported fields.
type Secret struct{}

// String hides the secret.
func (Secret) String() string { return Text }

// GoString hides the secret.
func (Secret) GoString() string { return Text }

// Format hides the secret for every verb, also %d and %x.
func (Secret) Format(state fmt.State, _ rune) { fmt.Fprint(state, Text) }

// LogValue hides the secret in structured logs.
func (Secret) LogValue() slog.Value { return slog.StringValue(Text) }

// MarshalText hides the secret in JSON and in every other text encoding.
func (Secret) MarshalText() ([]byte, error) { return []byte(Text), nil }
