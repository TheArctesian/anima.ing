package dial

import (
	"context"
	"github.com/TheArctesian/anima/link/workspaceauth"
	"github.com/TheArctesian/anima/link/workspacepreview"
	"io"
	"net"
	"net/netip"
	"strings"
	"time"

	"github.com/TheArctesian/anima/link/tunnel"
)

// Opener gives the connection to a device's local port, as Desk.Open does.
type Opener func(ctx context.Context, device netip.Addr, port uint16) (stream net.Conn, release func(), err error)

// Serve answers the hub link's dial socket, one request per connection, until the listener ends.
func Serve(listener net.Listener, open Opener) { ServeWithPreviews(listener, open, nil) }

// PreviewOpener opens a leased workspace preview through the link.
type PreviewOpener func(context.Context, netip.Addr, workspacepreview.Open) (net.Conn, func(), error)

// ServeWithPreviews adds namespace-bound previews on the existing private hub socket.
func ServeWithPreviews(listener net.Listener, open Opener, preview PreviewOpener) {
	ServeWithProviders(listener, open, preview, nil)
}

// ProviderOpener opens a human provider sign-in stream through the link.
type ProviderOpener func(context.Context, netip.Addr, workspaceauth.Login) (net.Conn, func(), error)

// ServeWithProviders adds transient human sign-in on the private hub socket.
func ServeWithProviders(listener net.Listener, open Opener, preview PreviewOpener, provider ProviderOpener) {
	for {
		local, err := listener.Accept()
		if err != nil {
			return
		}
		go serve(local, open, preview, provider)
	}
}

func serve(local net.Conn, open Opener, preview PreviewOpener, provider ProviderOpener) {
	_ = local.SetDeadline(time.Now().Add(lineLimit))
	line, read := readLine(local)
	device, port, valid := parseDial(line)
	var binding *workspacepreview.Open
	var login *workspaceauth.Login
	if fields := strings.Split(line, " "); read && len(fields) == 3 && fields[0] == "provider" && provider != nil {
		address, addressErr := netip.ParseAddr(fields[1])
		request, requestErr := workspaceauth.Decode(fields[2])
		device, valid = address, addressErr == nil && requestErr == nil
		login = &request
	}
	if fields := strings.Split(line, " "); read && len(fields) == 3 && fields[0] == "preview" && preview != nil {
		address, addressErr := netip.ParseAddr(fields[1])
		request, requestErr := workspacepreview.Decode(fields[2])
		device, valid = address, addressErr == nil && requestErr == nil
		binding = &request
	}
	if !read || !valid {
		say(local, WordBadRequest)
		local.Close()
		return
	}
	ctx, cancel := context.WithTimeout(context.Background(), AnswerLimit)
	var stream net.Conn
	var release func()
	var err error
	if login != nil {
		stream, release, err = provider(ctx, device, *login)
	} else if binding != nil {
		stream, release, err = preview(ctx, device, *binding)
	} else {
		stream, release, err = open(ctx, device, port)
	}
	cancel()
	if err != nil {
		say(local, Word(err))
		local.Close()
		return
	}
	defer release()
	if !say(local, WordOK) {
		local.Close()
		stream.Close()
		return
	}
	_ = local.SetDeadline(time.Time{})
	tunnel.Join(local, stream)
}

func say(local net.Conn, word string) bool {
	_ = local.SetWriteDeadline(time.Now().Add(lineLimit))
	_, err := io.WriteString(local, word+"\n")
	return err == nil
}
