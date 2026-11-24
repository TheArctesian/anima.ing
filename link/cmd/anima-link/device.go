package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net"
	"runtime"
	"strconv"
	"time"

	"github.com/TheArctesian/anima/link/control"
	"github.com/TheArctesian/anima/link/devicelink"
	"github.com/TheArctesian/anima/link/identity"
	"github.com/TheArctesian/anima/link/ports"
	"github.com/TheArctesian/anima/link/rdppeer"
	"github.com/TheArctesian/anima/link/statedir"
	"github.com/TheArctesian/anima/link/tunnel"
)

type deviceStatus struct {
	Role      string           `json:"role"`
	Local     string           `json:"local"`
	PublicKey tunnel.DeviceKey `json:"publicKey"`
	// Path is "direct", "relayed" or "none" (LNK-006).
	Path      string  `json:"path"`
	LatencyMS float64 `json:"latencyMs"`
	// OpenPorts is the local ports that the hub may reach (HMD-049).
	OpenPorts []uint16 `json:"openPorts"`
}

type openPorts struct {
	Open []uint16 `json:"open"`
}

// runPorts shows or turns the device's open ports (HMD-049).
func runPorts(ctx context.Context, args []string, streams stdio) error {
	flags, statePath := stateFlags("ports")
	if err := flags.Parse(args); err != nil || *statePath == "" || (flags.NArg() != 0 && flags.NArg() != 2) {
		return errUsage
	}
	dir, err := statedir.Beside(*statePath)
	if err != nil {
		return err
	}
	if flags.NArg() == 2 {
		port, err := strconv.ParseUint(flags.Arg(1), 10, 16)
		if err != nil || (flags.Arg(0) != "on" && flags.Arg(0) != "off") {
			return errUsage
		}
		enabled := flags.Arg(0) == "on"
		change := portChoice{Port: uint16(port), Enabled: &enabled}
		if err := turnPort(ctx, *statePath, change); err != nil {
			return err
		}
	}
	open, err := ports.List(dir)
	if err != nil {
		return err
	}
	answer, err := json.Marshal(openPorts{Open: append([]uint16{}, open...)})
	if err != nil {
		return err
	}
	_, err = fmt.Fprintln(streams.out, string(answer))
	return err
}

const portTurnVerb = "ports.turn"

type portChoice struct {
	Port    uint16 `json:"port"`
	Enabled *bool  `json:"enabled"`
}

func turnPort(ctx context.Context, statePath string, change portChoice) error {
	if !ports.Allowed(change.Port, runtime.GOOS) {
		return ports.ErrNotAllowed
	}
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	conn, err := statedir.Dial(ctx, statePath, controlSocket)
	if err == nil {
		defer conn.Close()
		if deadline, ok := ctx.Deadline(); ok {
			_ = conn.SetDeadline(deadline)
		}
		return control.Call(conn, portTurnVerb, change, nil)
	}

	if ctx.Err() != nil {
		return ctx.Err()
	}
	// An offline edit is safe only while this process owns the directory.
	owned, err := statedir.Open(statePath)
	if err != nil {
		return err
	}
	defer owned.Close()
	return ports.Turn(owned, change.Port, *change.Enabled)
}

func turnVerb(selected *ports.Selection) control.Verb {
	return func(_ context.Context, args json.RawMessage) (any, error) {
		var change portChoice
		if err := control.Arguments(args, &change); err != nil {
			return nil, err
		}
		if change.Enabled == nil || !ports.Allowed(change.Port, runtime.GOOS) {
			return nil, &control.Error{Code: control.CodeBadRequest, Message: "Choose a supported port and whether to enable it."}
		}
		return nil, selected.Turn(change.Port, *change.Enabled)
	}
}

// listenForDaemon makes the daemon's token socket before the link starts; an empty path means none.
func listenForDaemon(path string) (net.Listener, error) {
	if path == "" {
		return nil, nil
	}
	return statedir.ListenOwned(path)
}

func bundledRDP(dir *statedir.Dir) func(context.Context) (net.Conn, error) {
	if !rdppeer.Supported {
		return nil
	}
	return func(ctx context.Context) (net.Conn, error) {
		return rdppeer.Connect(ctx, dir)
	}
}

func runDevice(ctx context.Context, args []string, streams stdio) error {
	flags, statePath := stateFlags("device")
	listen := flags.String("listen", "127.0.0.1:0", "the local loopback address, host:port")
	daemonSocket := flags.String("daemon-socket", "", "the path of a Unix socket for the metrics daemon")
	stopWithStdin := flags.Bool("stop-with-stdin", false, "stop when standard input closes")
	if err := flags.Parse(args); err != nil || *statePath == "" || flags.NArg() != 0 {
		return errUsage
	}
	log := slog.New(slog.NewTextHandler(streams.err, nil))

	dir, err := statedir.Open(*statePath)
	if err != nil {
		return err
	}
	defer dir.Close()
	device, err := identity.LoadDevice(dir)
	if err != nil {
		return err
	}
	forDaemon, err := listenForDaemon(*daemonSocket)
	if err != nil {
		return err
	}
	selected := ports.Selected(dir)
	link, err := devicelink.Start(devicelink.Config{
		Identity: device, Listen: *listen, DaemonSocket: forDaemon,
		Save:       func(renewed identity.Device) error { return renewed.Save(dir) },
		PermitPort: selected.Permit,
		DialRDP:    bundledRDP(dir),
	})
	if err != nil {
		return err
	}
	defer link.Close()
	listener, err := dir.Listen(controlSocket)
	if err != nil {
		return err
	}

	ctx, stop := context.WithCancel(ctx)
	defer stop()
	if *stopWithStdin {
		go stopWhenClosed(streams.in, stop)
	}
	log.Info("the device link is ready", "local", link.LocalAddr().String(), "daemonSocket", *daemonSocket, "key", device.Key.Public())
	err = control.Serve(ctx, listener, map[string]control.Verb{
		portTurnVerb: turnVerb(selected),
		"status": func(context.Context, json.RawMessage) (any, error) {
			path := link.Path()
			open, err := ports.List(dir)
			return deviceStatus{
				Role: "device", Local: link.LocalAddr().String(), PublicKey: device.Key.Public(),
				Path: string(path.Kind), LatencyMS: float64(path.Latency.Microseconds()) / 1000,
				OpenPorts: append([]uint16{}, open...),
			}, err
		},
	})
	log.Info("the device link stopped")
	return err
}
