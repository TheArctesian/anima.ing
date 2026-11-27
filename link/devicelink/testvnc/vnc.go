// Package testvnc probes the installed TigerVNC service through the private link.
package testvnc

import (
	"bytes"
	"crypto/tls"
	"encoding/binary"
	"errors"
	"image"
	"io"
	"net"
	"testing"
	"time"
)

const (
	pointerX = 173
	pointerY = 127
)

var errChangedCertificate = errors.New("VNC leaf certificate changed")

// Check covers pin and PAM refusals, then captures the authenticated display.
// The supplied opener gives each attempt a fresh private stream.
func Check(t *testing.T, open func() net.Conn, certificate []byte, password string) image.Image {
	t.Helper()
	wrongCertificate := bytes.Clone(certificate)
	wrongCertificate[0] ^= 1
	if _, err := secure(t, open(), wrongCertificate); !errors.Is(err, errChangedCertificate) {
		t.Fatalf("VNC with a different certificate: %v", err)
	}
	if result := credentials(t, connect(t, open(), certificate), "invalid-disposable-password"); result != 1 {
		t.Fatalf("VNC accepted a wrong password, security result %d", result)
	}

	conn := connect(t, open(), certificate)
	if result := credentials(t, conn, password); result != 0 {
		t.Fatalf("VNC PAM login failed, security result %d", result)
	}
	bounds := display(t, conn)
	frame := capture(t, conn, bounds)
	write(t, conn, []byte{5, 0, 0, pointerX, 0, pointerY})
	capture(t, conn, image.Rect(0, 0, 1, 1))
	return frame
}

func connect(t *testing.T, stream net.Conn, certificate []byte) *tls.Conn {
	t.Helper()
	conn, err := secure(t, stream, certificate)
	if err != nil {
		t.Fatalf("VNC X509Plain TLS: %v", err)
	}
	return conn
}

func secure(t *testing.T, stream net.Conn, certificate []byte) (*tls.Conn, error) {
	t.Helper()
	if err := stream.SetDeadline(time.Now().Add(45 * time.Second)); err != nil {
		t.Fatal(err)
	}
	version := []byte("RFB 003.008\n")
	expect(t, stream, version)
	write(t, stream, version)
	expect(t, stream, []byte{1, 19}) // VeNCrypt only.
	write(t, stream, []byte{19})
	expect(t, stream, []byte{0, 2})
	write(t, stream, []byte{0, 2})
	expect(t, stream, []byte{0, 1, 0, 0, 1, 6}) // VeNCrypt 0.2, X509Plain (262) only.
	write(t, stream, []byte{0, 0, 1, 6})
	expect(t, stream, []byte{1})

	conn := tls.Client(stream, &tls.Config{
		MinVersion: tls.VersionTLS12,
		// The fixture trusts the exact exported leaf for this generated identity.
		InsecureSkipVerify: true,
		VerifyConnection: func(state tls.ConnectionState) error {
			if len(state.PeerCertificates) != 1 || !bytes.Equal(state.PeerCertificates[0].Raw, certificate) {
				return errChangedCertificate
			}
			return nil
		},
	})
	return conn, conn.Handshake()
}

func credentials(t *testing.T, conn net.Conn, password string) uint32 {
	t.Helper()
	const user = "admin"
	message := binary.BigEndian.AppendUint32(nil, uint32(len(user)))
	message = binary.BigEndian.AppendUint32(message, uint32(len(password)))
	message = append(message, user...)
	message = append(message, password...)
	write(t, conn, message)
	return binary.BigEndian.Uint32(read(t, conn, 4))
}

func display(t *testing.T, conn net.Conn) image.Rectangle {
	t.Helper()
	write(t, conn, []byte{1}) // ClientInit asks to share the existing display.
	header := read(t, conn, 24)
	width, height := number(header), number(header[2:])
	nameSize := binary.BigEndian.Uint32(header[20:])
	if width < 800 || height < 600 || width > 4096 || height > 2160 || nameSize > 4096 {
		t.Fatalf("unexpected VNC desktop geometry %dx%d or name size %d", width, height, nameSize)
	}
	read(t, conn, int(nameSize))
	// Request 32-bit true color, little-endian, with each of RGB using 8 bits.
	write(t, conn, []byte{0, 0, 0, 0, 32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0})
	write(t, conn, []byte{2, 0, 0, 1, 0, 0, 0, 0}) // SetEncodings, Raw only.
	return image.Rect(0, 0, width, height)
}

func capture(t *testing.T, conn net.Conn, bounds image.Rectangle) *image.RGBA {
	t.Helper()
	request := []byte{3, 0} // Non-incremental FramebufferUpdateRequest.
	for _, value := range []int{bounds.Min.X, bounds.Min.Y, bounds.Dx(), bounds.Dy()} {
		request = binary.BigEndian.AppendUint16(request, uint16(value))
	}
	write(t, conn, request)
	reply := read(t, conn, 4)
	count := number(reply[2:])
	if reply[0] != 0 || count == 0 || count > 4096 {
		t.Fatalf("unexpected VNC framebuffer header %x", reply)
	}
	frame := image.NewRGBA(bounds)
	for range count {
		rect := read(t, conn, 12)
		point := image.Pt(number(rect), number(rect[2:]))
		area := image.Rectangle{Min: point, Max: point.Add(image.Pt(number(rect[4:]), number(rect[6:])))}
		if area.Empty() || !area.In(bounds) || binary.BigEndian.Uint32(rect[8:]) != 0 {
			t.Fatalf("unexpected raw VNC rectangle %x", rect)
		}
		paint(t, conn, frame, area)
	}
	return frame
}

func paint(t *testing.T, conn net.Conn, frame *image.RGBA, area image.Rectangle) {
	t.Helper()
	row := make([]byte, area.Dx()*4)
	for y := area.Min.Y; y < area.Max.Y; y++ {
		if _, err := io.ReadFull(conn, row); err != nil {
			t.Fatalf("reading the VNC framebuffer: %v", err)
		}
		start := frame.PixOffset(area.Min.X, y)
		for offset := 0; offset < len(row); offset += 4 {
			dest := frame.Pix[start+offset : start+offset+4]
			dest[0], dest[1], dest[2], dest[3] = row[offset+2], row[offset+1], row[offset], 255
		}
	}
}

func number(data []byte) int { return int(binary.BigEndian.Uint16(data)) }

func expect(t *testing.T, stream io.Reader, want []byte) {
	t.Helper()
	if got := read(t, stream, len(want)); !bytes.Equal(got, want) {
		t.Fatalf("VNC negotiation got %x, want %x", got, want)
	}
}

func read(t *testing.T, stream io.Reader, size int) []byte {
	t.Helper()
	data := make([]byte, size)
	if _, err := io.ReadFull(stream, data); err != nil {
		t.Fatalf("reading %d VNC bytes: %v", size, err)
	}
	return data
}

func write(t *testing.T, stream io.Writer, data []byte) {
	t.Helper()
	n, err := stream.Write(data)
	if err != nil || n != len(data) {
		t.Fatalf("writing %d VNC bytes: wrote %d: %v", len(data), n, err)
	}
}
