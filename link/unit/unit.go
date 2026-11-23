// Package unit manages the user's unit that keeps the device link running, never as root (HMD-058, R-240).
package unit

import (
	"errors"
	"fmt"
	unixpath "path"
	"strconv"
	"strings"
	"time"
)

// ErrRefused means that the unit cannot be made from what was given; the message names the wrong value.
var ErrRefused = errors.New("unit: refused")

// Label is the name of the LaunchAgent, and ServiceName the name of the systemd unit.
const (
	Label       = "ing.anima.link"
	ServiceName = "anima-link.service"
)

// Settings is what the unit runs: `Program device -state State -listen 127.0.0.1:Port [-daemon-socket DaemonSocket]`.
type Settings struct {
	// Program is the absolute path of anima-link, outside an application or a mounted image.
	Program string
	// State is the absolute path of the state directory of the device.
	State string
	// Port is the fixed loopback port of the link, so that the application finds the link again.
	Port uint16
	// DaemonSocket is the absolute path of the socket of the metrics daemon; empty means no socket.
	DaemonSocket string
}

// System is the computer and the user that the unit is for.
type System struct {
	// GOOS is a value of runtime.GOOS.
	GOOS string
	// SID and Windows are the current Windows account and system directory.
	SID     string
	Windows string
	UID     int
	// Home is the home folder of the user, and ConfigHome the value of XDG_CONFIG_HOME (empty when it is not set).
	Home       string
	ConfigHome string
	// RuntimeDir is the value of XDG_RUNTIME_DIR (empty when it is not set).
	RuntimeDir string
}

// Step is one fixed command: an absolute program, no shell and no input.
type Step struct {
	Program string
	Args    []string
	Env     []string
	// MayFail means that a failure is expected, such as to stop a unit that is not loaded.
	MayFail bool
	// RetryAfter, when not zero, runs a failed command once more after this pause.
	RetryAfter time.Duration
}

// Plan is the file of the unit and the commands around it.
type Plan struct {
	File    string
	Content string
	// Start runs after the file is written, Stop before it is deleted, and After after it is deleted.
	Start []Step
	Stop  []Step
	After []Step
}

func refused(format string, args ...any) error {
	return fmt.Errorf("%w: %s", ErrRefused, fmt.Sprintf(format, args...))
}

// plainPath accepts an absolute, clean Unix path, whatever the OS, with no character that could end a value in a unit file.
func plainPath(path string) bool {
	if !unixpath.IsAbs(path) || unixpath.Clean(path) != path {
		return false
	}
	for _, char := range path {
		if char < 0x20 || char == 0x7f || strings.ContainsRune(`"'\%$<>&`, char) {
			return false
		}
	}
	return true
}

// carried reports whether a path is in an application bundle or a mounted image, which an update or a move changes.
func carried(path string) bool {
	return strings.Contains(path, ".app/") || strings.HasPrefix(path, "/Volumes/") ||
		strings.HasPrefix(path, "/tmp/.mount_") || strings.Contains(path, "/AppTranslocation/")
}

func (s Settings) check() error {
	switch {
	case !plainPath(s.Program):
		return refused("the program path must be absolute and plain")
	case carried(s.Program):
		return refused("the program is inside an application or a mounted image: copy it to a folder that stays, like ~/.local/bin")
	case !plainPath(s.State):
		return refused("the state directory must be an absolute, plain path")
	case s.Port == 0:
		return refused("the unit needs a fixed port: give -listen 127.0.0.1:PORT")
	case s.DaemonSocket != "" && !plainPath(s.DaemonSocket):
		return refused("the path of the daemon socket must be absolute and plain")
	}
	return nil
}

func (s Settings) arguments() []string {
	args := []string{s.Program, "device", "-state", s.State, "-listen", "127.0.0.1:" + strconv.Itoa(int(s.Port))}
	if s.DaemonSocket != "" {
		args = append(args, "-daemon-socket", s.DaemonSocket)
	}
	return args
}

// launchAgent needs no XML escaping: plainPath lets no such character through.
func launchAgent(s Settings) string {
	var text strings.Builder
	text.WriteString(`<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- The device link of Anima.ing as a LaunchAgent (HMD-058). "anima-link unit install" writes this file. -->
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>` + Label + `</string>
    <key>ProgramArguments</key>
    <array>
`)
	for _, arg := range s.arguments() {
		text.WriteString("        <string>" + arg + "</string>\n")
	}
	text.WriteString(`    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
`)
	return text.String()
}

// systemdUnit puts each argument in double quotes; plainPath lets no character through that ends them.
func systemdUnit(s Settings) string {
	quoted := make([]string, 0, 8)
	for _, arg := range s.arguments() {
		quoted = append(quoted, `"`+arg+`"`)
	}
	return `# The device link of Anima.ing as a systemd user unit (HMD-058). "anima-link unit install" writes this file.
[Unit]
Description=Anima.ing link: connects this computer to its hub
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=` + strings.Join(quoted, " ") + `
Restart=always
RestartSec=10
NoNewPrivileges=yes
LimitCORE=0

[Install]
WantedBy=default.target
`
}

const (
	launchctl      = "/bin/launchctl"
	systemctl      = "/usr/bin/systemctl"
	bootstrapRetry = time.Second
)

func darwinPlace(on System) Plan {
	file := unixpath.Join(on.Home, "Library", "LaunchAgents", Label+".plist")
	domain := "gui/" + strconv.Itoa(on.UID)
	bootout := Step{Program: launchctl, Args: []string{"bootout", domain + "/" + Label}, MayFail: true}
	return Plan{
		File: file,
		// A loaded unit is taken out first, so that a new file or a new program is used.
		Start: []Step{bootout, {Program: launchctl, Args: []string{"bootstrap", domain, file}, RetryAfter: bootstrapRetry}},
		Stop:  []Step{bootout},
	}
}

func linuxPlace(on System) (Plan, error) {
	config := on.ConfigHome
	if config == "" {
		config = unixpath.Join(on.Home, ".config")
	}
	runtimeDir := on.RuntimeDir
	if runtimeDir == "" {
		runtimeDir = "/run/user/" + strconv.Itoa(on.UID)
	}
	if !plainPath(config) || !plainPath(runtimeDir) {
		return Plan{}, refused("XDG_CONFIG_HOME and XDG_RUNTIME_DIR must be absolute, plain paths")
	}
	user := func(mayFail bool, args ...string) Step {
		return Step{
			Program: systemctl, Args: append([]string{"--user"}, args...),
			Env: []string{"XDG_RUNTIME_DIR=" + runtimeDir}, MayFail: mayFail,
		}
	}
	return Plan{
		File: unixpath.Join(config, "systemd", "user", ServiceName),
		// restart starts a stopped unit and restarts a running one, so a new program is used.
		Start: []Step{user(false, "daemon-reload"), user(false, "enable", ServiceName), user(false, "restart", ServiceName)},
		Stop:  []Step{user(true, "disable", "--now", ServiceName)},
		After: []Step{user(true, "daemon-reload")},
	}, nil
}

// Place is where the user's unit is on the system on, with its start and stop commands and no content.
// It returns ErrRefused for a system that has no unit of the user.
func Place(on System) (Plan, error) {
	if on.GOOS == "windows" {
		return windowsPlace(on)
	}
	if !plainPath(on.Home) {
		return Plan{}, refused("the home folder must be an absolute, plain path")
	}
	switch on.GOOS {
	case "darwin":
		return darwinPlace(on), nil
	case "linux":
		return linuxPlace(on)
	default:
		return Plan{}, refused("no unit of the user on %s", on.GOOS)
	}
}

// PlanFor is the file, the content and the commands of the unit of s on the system on.
// It returns ErrRefused for a value that cannot go into a unit, and for a system that has no unit of the user.
func PlanFor(s Settings, on System) (Plan, error) {
	if on.GOOS == "windows" {
		return windowsTask(s, on)
	}
	if err := s.check(); err != nil {
		return Plan{}, err
	}
	plan, err := Place(on)
	if err != nil {
		return Plan{}, err
	}
	if on.GOOS == "darwin" {
		plan.Content = launchAgent(s)
	} else {
		plan.Content = systemdUnit(s)
	}
	return plan, nil
}
