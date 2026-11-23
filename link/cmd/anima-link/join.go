package main

import (
	"bufio"
	"context"
	"fmt"
	"time"

	"github.com/TheArctesian/anima/link/devicelink"
	"github.com/TheArctesian/anima/link/identity"
	"github.com/TheArctesian/anima/link/invites"
	"github.com/TheArctesian/anima/link/statedir"
	"github.com/TheArctesian/anima/link/tunnel"
)

const joinLimit = 30 * time.Second

// runJoin reads the invite from stdin, not an argument, as other users can read a process's arguments (LNK-008).
func runJoin(ctx context.Context, args []string, streams stdio) error {
	flags, statePath := stateFlags("join")
	hubName := flags.String("hub-name", "", "a domain name that holds the hub's address in a TXT record (optional)")
	if err := flags.Parse(args); err != nil || *statePath == "" || flags.NArg() != 0 {
		return errUsage
	}
	if *hubName != "" && !tunnel.ValidHostName(*hubName) {
		return errUsage
	}
	line, _ := bufio.NewReader(streams.in).ReadString('\n')
	invite, err := invites.Parse(line)
	if err != nil {
		return err
	}
	dir, err := statedir.Open(*statePath)
	if err != nil {
		return err
	}
	defer dir.Close()
	device, err := identity.LoadDevice(dir)
	if err != nil {
		return err
	}
	ctx, cancel := context.WithTimeout(ctx, joinLimit)
	defer cancel()
	joined, err := devicelink.Join(ctx, invite, device)
	if err != nil {
		return err
	}
	joined.HubName = *hubName
	if err := joined.Save(dir); err != nil {
		return err
	}
	// The fingerprint lets the operator check in devices.list that this device, not a thief, used the invite.
	_, err = fmt.Fprintf(streams.out, "This device has joined the hub. Its key fingerprint is %s.\nStart the link with: anima-link device -state %s\n",
		joined.Key.Public().Fingerprint(), *statePath)
	return err
}
