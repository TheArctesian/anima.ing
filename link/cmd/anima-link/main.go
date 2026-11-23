// Command anima-link carries the hub's traffic over an encrypted link (docs/plans/one-click/one-click-plan.md, Section 4.1).
package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"os/signal"
	"syscall"

	"github.com/TheArctesian/anima/link/unit"
)

const usage = `usage: anima-link hub -state DIR -target SOCKET-PATH|127.0.0.1:PORT [-relay HOST[:PORT] [-replace-relay]] [-stop-with-stdin]
       anima-link device -state DIR [-listen 127.0.0.1:PORT] [-daemon-socket SOCKET-PATH] [-stop-with-stdin]
       anima-link join -state DIR [-hub-name NAME]   (the invite is read from standard input)
       anima-link ports -state DIR [on|off PORT]   (the local ports that the hub may reach: 22, 5900)
       anima-link ctl -state DIR VERB [JSON-ARGUMENTS]
       anima-link unit install -state DIR -listen 127.0.0.1:PORT [-daemon-socket SOCKET-PATH]   (the device link as a unit of this user)
       anima-link unit remove|show`

type stdio struct {
	in  io.Reader
	out io.Writer
	err io.Writer
}

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()
	os.Exit(run(ctx, os.Args[1:], stdio{os.Stdin, os.Stdout, os.Stderr}))
}

// run returns the exit code: 0 done, 1 failed, 2 wrong use.
func run(ctx context.Context, args []string, streams stdio) int {
	commands := map[string]func(context.Context, []string, stdio) error{
		"hub":    runHub,
		"device": runDevice,
		"join":   runJoin,
		"ports":  runPorts,
		"ctl":    runCtl,
		"unit": func(ctx context.Context, args []string, streams stdio) error {
			return unit.Command(ctx, args, streams.out)
		},
	}
	if len(args) == 0 || commands[args[0]] == nil {
		fmt.Fprintln(streams.err, usage)
		return 2
	}
	err := commands[args[0]](ctx, args[1:], streams)
	switch {
	case err == nil:
		return 0
	case errors.Is(err, errUsage), errors.Is(err, flag.ErrHelp):
		fmt.Fprintln(streams.err, usage)
		return 2
	default:
		fmt.Fprintln(streams.err, "anima-link:", err)
		return 1
	}
}

var errUsage = errors.New("wrong use")

func stateFlags(name string) (*flag.FlagSet, *string) {
	flags := flag.NewFlagSet(name, flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	return flags, flags.String("state", "", "the state directory")
}

// stopWhenClosed ends the command when the application that holds the other end of the stdin pipe stops.
func stopWhenClosed(stream io.Reader, stop context.CancelFunc) {
	_, _ = io.Copy(io.Discard, stream)
	stop()
}
