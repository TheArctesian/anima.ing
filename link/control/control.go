// Package control is the owner-only socket of a running anima-link: one JSON request and one JSON reply per line.
package control

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net"
	"sync"
	"syscall"
	"time"
)

// Error is a refusal that crosses the socket; neither field may hold a secret (LNK-008).
type Error struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

func (e *Error) Error() string { return e.Message }

// The codes that this package makes itself. Verbs add their own.
const (
	CodeUnknownVerb = "unknown_verb"
	CodeBadRequest  = "bad_request"
	CodeFailed      = "failed"
)

// Verb does one kind of request; an error that is not an *Error reaches the caller as CodeFailed with a fixed message.
type Verb func(ctx context.Context, args json.RawMessage) (any, error)

// Arguments reads a request's arguments into value and refuses unknown fields.
func Arguments(args json.RawMessage, value any) error {
	decoder := json.NewDecoder(bytes.NewReader(args))
	decoder.DisallowUnknownFields()
	if decoder.Decode(value) != nil {
		return &Error{CodeBadRequest, "the arguments of this verb are not valid"}
	}
	return nil
}

type request struct {
	Verb string          `json:"verb"`
	Args json.RawMessage `json:"args,omitempty"`
}

type reply struct {
	OK     bool            `json:"ok"`
	Result json.RawMessage `json:"result,omitempty"`
	Error  *Error          `json:"error,omitempty"`
}

// maxLine is the largest request or reply; a device list is the largest reply.
const maxLine = 1 << 20

// Serve answers requests until ctx ends; a started request, also one that ends ctx, gets its reply first.
func Serve(ctx context.Context, listener net.Listener, verbs map[string]Verb) error {
	go func() {
		<-ctx.Done()
		listener.Close()
	}()
	var inFlight sync.WaitGroup
	defer inFlight.Wait()
	for {
		conn, err := AcceptPatiently(ctx, listener)
		if err != nil {
			if ctx.Err() != nil {
				return nil
			}
			return err
		}
		go answer(ctx, conn, verbs, &inFlight)
	}
}

// AcceptPatiently retries when the system has no free file, so that many connections cannot stop a server.
func AcceptPatiently(ctx context.Context, listener net.Listener) (net.Conn, error) {
	for wait := 5 * time.Millisecond; ; wait = min(2*wait, time.Second) {
		conn, err := listener.Accept()
		if err == nil || !(errors.Is(err, syscall.EMFILE) || errors.Is(err, syscall.ENFILE)) {
			return conn, err
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(wait):
		}
	}
}

func answer(ctx context.Context, conn net.Conn, verbs map[string]Verb, inFlight *sync.WaitGroup) {
	defer conn.Close()
	lines := bufio.NewScanner(conn)
	lines.Buffer(make([]byte, 0, 4096), maxLine)
	encoder := json.NewEncoder(conn)
	for lines.Scan() {
		inFlight.Add(1)
		err := encoder.Encode(run(ctx, lines.Bytes(), verbs))
		inFlight.Done()
		if err != nil {
			return
		}
	}
}

func run(ctx context.Context, line []byte, verbs map[string]Verb) reply {
	var req request
	if json.Unmarshal(line, &req) != nil {
		return refusal(&Error{CodeBadRequest, "the request is not valid JSON"})
	}
	verb, known := verbs[req.Verb]
	if !known {
		return refusal(&Error{CodeUnknownVerb, "no such verb"})
	}
	result, err := verb(ctx, req.Args)
	if err != nil {
		return refusal(err)
	}
	text, err := json.Marshal(result)
	if err != nil {
		return refusal(err)
	}
	return reply{OK: true, Result: text}
}

func refusal(err error) reply {
	var told *Error
	if !errors.As(err, &told) {
		told = &Error{CodeFailed, "the request failed; see the log of anima-link"}
	}
	return reply{Error: told}
}

// Call sends one request and reads its reply into result (nil drops it); a refusal returns as an *Error.
func Call(conn net.Conn, verb string, args, result any) error {
	argsText, err := json.Marshal(args)
	if err != nil {
		return err
	}
	if err := json.NewEncoder(conn).Encode(request{Verb: verb, Args: argsText}); err != nil {
		return err
	}
	lines := bufio.NewScanner(conn)
	lines.Buffer(make([]byte, 0, 4096), maxLine)
	if !lines.Scan() {
		if err := lines.Err(); err != nil {
			return err
		}
		return &Error{CodeFailed, "anima-link closed the connection with no reply"}
	}
	var got reply
	if err := json.Unmarshal(lines.Bytes(), &got); err != nil {
		return &Error{CodeFailed, "the reply is not valid JSON"}
	}
	if !got.OK {
		if got.Error == nil {
			return &Error{CodeFailed, "the reply has no error"}
		}
		return got.Error
	}
	if result == nil {
		return nil
	}
	return json.Unmarshal(got.Result, result)
}
