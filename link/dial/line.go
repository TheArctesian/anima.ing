// Package dial lets the hub reach a local port of a device, which opens the stream (HMD-048, HMD-049).
//
// The hub link writes "open <port> <id>" on the reverse port; the device answers on the open port with
// "<id> ok" and the bytes, or "<id> refused" or "<id> closed". The hub's dial socket takes
// "dial <link address> <port>" and answers "ok" and the bytes, or one word. An id is not logged.
package dial

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"io"
	"net/netip"
	"strconv"
	"strings"
)

// The words of the answers. The device says the first three; the dial socket also says the others.
const (
	WordOK           = "ok"
	WordRefused      = "refused"
	WordClosed       = "closed"
	WordNotConnected = "not_connected"
	WordTimeout      = "timeout"
	WordBadRequest   = "bad_request"
)

// Errors of this package: why the hub got no connection.
var (
	ErrNotConnected = errors.New("dial: the device has no stream on the reverse port")
	ErrRefused      = errors.New("dial: the port is not open on the device")
	ErrClosed       = errors.New("dial: nothing listens on the port of the device")
	ErrTimeout      = errors.New("dial: the device did not answer in time")
	ErrBadRequest   = errors.New("dial: the request is not valid")
)

const maxLine = 128

const idBytes = 16

func newID() string {
	id := make([]byte, idBytes)
	_, _ = rand.Read(id)
	return hex.EncodeToString(id)
}

func validID(id string) bool {
	if len(id) != 2*idBytes {
		return false
	}
	_, err := hex.DecodeString(id)
	return err == nil && strings.ToLower(id) == id
}

func parsePort(text string) (uint16, bool) {
	if text == "" || text[0] == '0' || text[0] == '+' {
		return 0, false
	}
	port, err := strconv.ParseUint(text, 10, 16)
	return uint16(port), err == nil
}

func openLine(port uint16, id string) string {
	return "open " + strconv.Itoa(int(port)) + " " + id + "\n"
}

// Request is one request of the hub for a local port of the device.
type Request struct {
	Port uint16
	id   string
}

// ParseRequest reads a line of the hub on the reverse port, without its newline.
func ParseRequest(line string) (Request, bool) {
	fields := strings.Split(line, " ")
	if len(fields) != 3 || fields[0] != "open" || !validID(fields[2]) {
		return Request{}, false
	}
	port, valid := parsePort(fields[1])
	return Request{Port: port, id: fields[2]}, valid
}

func answerLine(id, word string) string { return id + " " + word + "\n" }

func parseAnswer(line string) (id, word string, valid bool) {
	id, word, found := strings.Cut(line, " ")
	switch {
	case !found || !validID(id):
		return "", "", false
	case word == WordOK, word == WordRefused, word == WordClosed:
		return id, word, true
	default:
		return "", "", false
	}
}

func parseDial(line string) (netip.Addr, uint16, bool) {
	fields := strings.Split(line, " ")
	if len(fields) != 3 || fields[0] != "dial" {
		return netip.Addr{}, 0, false
	}
	device, err := netip.ParseAddr(fields[1])
	port, valid := parsePort(fields[2])
	return device, port, err == nil && valid
}

// Word is the word that the dial socket says for an error of Open; any other error is a timeout.
func Word(err error) string {
	switch {
	case errors.Is(err, ErrNotConnected):
		return WordNotConnected
	case errors.Is(err, ErrRefused):
		return WordRefused
	case errors.Is(err, ErrClosed):
		return WordClosed
	case errors.Is(err, ErrBadRequest):
		return WordBadRequest
	default:
		return WordTimeout
	}
}

// readLine reads one byte at a time: the bytes after the line belong to the connection that follows.
func readLine(stream io.Reader) (string, bool) {
	line := make([]byte, 0, maxLine)
	one := make([]byte, 1)
	for len(line) <= maxLine {
		if _, err := io.ReadFull(stream, one); err != nil {
			return "", false
		}
		if one[0] == '\n' {
			return string(line), true
		}
		line = append(line, one[0])
	}
	return "", false
}
