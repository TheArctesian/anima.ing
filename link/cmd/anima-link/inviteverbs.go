package main

import (
	"context"
	"crypto/rand"
	"encoding/json"
	"time"

	"github.com/TheArctesian/anima/link/control"
	"github.com/TheArctesian/anima/link/hublink"
	"github.com/TheArctesian/anima/link/invites"
)

type inviteRequest struct {
	Name string `json:"name"`
}

// madeInvite.Invite is a secret until used or expired: it goes to the caller only, never to a log (LNK-008).
type madeInvite struct {
	Invite  string    `json:"invite"`
	Expires time.Time `json:"expires"`
}

// listedInvite.ID is the fingerprint of the invite's temporary key, which "invites.cancel" takes.
type listedInvite struct {
	ID      string    `json:"id"`
	Name    string    `json:"name"`
	Expires time.Time `json:"expires"`
}

type cancelRequest struct {
	ID string `json:"id"`
}

func inviteVerbs(link *hublink.Link, open *invites.Store, now func() time.Time) map[string]control.Verb {
	return map[string]control.Verb{
		"invites.make": func(_ context.Context, args json.RawMessage) (any, error) {
			var request inviteRequest
			if err := control.Arguments(args, &request); err != nil {
				return nil, err
			}
			invite, record, err := open.Make(link.Address(), request.Name, now(), rand.Reader)
			if err != nil {
				return nil, refusalOf(err)
			}
			return madeInvite{invite.Reveal(), record.Expires}, nil
		},
		"invites.list": func(context.Context, json.RawMessage) (any, error) {
			listed := []listedInvite{}
			for _, record := range open.All(now()) {
				listed = append(listed, listedInvite{record.Key.Fingerprint(), record.Name, record.Expires})
			}
			return listed, nil
		},
		"invites.cancel": func(_ context.Context, args json.RawMessage) (any, error) {
			var request cancelRequest
			if err := control.Arguments(args, &request); err != nil {
				return nil, err
			}
			return nil, refusalOf(open.Cancel(request.ID, now()))
		},
	}
}
