# Prompt: set up the Pop!_OS machine for Anima

Paste everything below the line into Claude Code on the Pop!_OS machine.

---

You are setting up this Pop!_OS computer as the second operator computer and a Linux test device for Anima, a self-hosted device manager. The repository is `TheArctesian/anima`. Work branch: `phase-2`. Read `AGENTS.md` and `docs/spec/design-spec.md` Sections 4.2, 12 and 13 before you change anything.

Rules:
- Never read `.env*` files or key files, and never print secrets.
- Run every `sudo` step yourself only if I type the password into the prompt. Never pipe a password into `sudo -S`.
- The VPS (`anima-admin@100.64.0.1`) and Headscale are changed only after I say yes in chat (VPS-005). For each change, show me the command first, then wait.
- Use `tailscale`, not a second VPN. If another full-tunnel VPN is on, tell me (VPN-007).

## 1. Join the Anima VPN (Headscale, personal login like the Mac)

1. Install Tailscale 1.80 or later: `curl -fsSL https://tailscale.com/install.sh | sh`. Show me the script before you run it.
2. Run `sudo tailscale login --login-server https://hs.anima.ing --accept-routes --hostname pop-os`.
3. It prints an auth URL with an auth ID. Tell me the auth ID. On the Mac I will register it on the VPS:
   `hsctl auth register --user operator --auth-id <AUTH_ID>`, then set the 180-day expiry (VPN-003) as in WP-03 step 7.
4. Check:
   - `tailscale ip -4` gives a 100.64.0.x address;
   - `tailscale ping hub` works;
   - `curl -sS https://hub.anima.ing/health` prints `ok`.

## 2. Allow the hub to reach this computer (VPN rule, needs my yes)

Draft, but do not apply, the change to `deploy/headscale/config/policy.hujson`:
- under `hosts`, add `"device-pop-os": "<this computer's 100.64 address>/32"`;
- a grant like the Mac's: `{"src": ["tag:hub"], "dst": ["device-pop-os"], "ip": ["tcp:22", "tcp:5900"]}`.
Show me the diff. Once I say yes, I apply it on the VPS the same way as the WP-23 policy step, and you log the change in `docs/plans/phase-3/operator-changes.md`.

## 3. Turn on SSH and screen sharing

- SSH: `sudo apt install openssh-server`, then `sudo systemctl enable --now ssh`.
- Screen sharing. It must take keyboard and mouse input, not only show the screen. Check the desktop first: `echo $XDG_CURRENT_DESKTOP $XDG_SESSION_TYPE`.
  - GNOME: use GNOME Remote Desktop with VNC (Settings → Sharing), if this version offers VNC.
  - COSMIC on Wayland (the Pop!_OS 24.04 default): use krfb through the COSMIC RemoteDesktop portal, as in "Screen sharing on COSMIC" below. Do not use wayvnc there: the COSMIC compositor has no virtual pointer, so wayvnc is view only. GNOME Remote Desktop needs mutter and does not run on COSMIC.
  - An X11 session: `x11vnc`.
  - Set a password, and confirm it listens on port 5900: `ss -ltnp | grep 5900`.
  - Allow port 5900 only on the VPN: `sudo ufw allow in on tailscale0 to any port 5900 proto tcp`, then `sudo ufw deny 5900/tcp`, then `sudo ufw enable`. If `ufw enable` says "Could not load logging rules", run `sudo ufw logging off` and enable again. From a different device on the home network, `nc -zv 192.168.x.x 5900` must time out.
  - Tell me which server and display system (Wayland or X11) you used.

### Screen sharing on COSMIC (krfb)

Checked on `pop-os` on 2026-11-23: VNC with keyboard and mouse, and krfb starting at login with no dialog.

1. Update the system first (`sudo apt full-upgrade`), then log out and in. The COSMIC portal must be a build that offers "Always allow" for remote control. An older running portal asked again on each start, even with a stored grant. Check there is room on the disk before the upgrade: an upgrade that fills the disk stops halfway (`sudo dpkg --configure -a` finishes it).
2. `sudo apt install krfb`. In `~/.config/krfbrc`, set `preferredFrameBufferPlugin=pw` under `[FrameBuffer]` and `[Framebuffer]`, and under `[Security]` set `allowDesktopControl=true`, `allowUnattendedAccess=true` and `noWallet=true`. Set the unattended password in krfb itself, and keep a copy only in a file readable by this user (`chmod 600`).
3. Patch krfb so the portal remembers the approval. Ubuntu's krfb 23.08 does not ask the portal to keep the permission, so COSMIC shows a dialog each time krfb starts. Newer krfb does; back-port that change to the 23.08 source:
   1. `sudo apt build-dep -y krfb`, then `apt-get source krfb` in `~/.local/src/krfb-restore`.
   2. In `framebuffers/pipewire/pw_framebuffer.cpp`, in `handleSessionCreated`: add `persist_mode` = 2 (a `uint`) to the `SelectDevices` options, and if the state config group `XdgPortal` has a `RestoreToken`, pass it as `restore_token`. In `handleRemoteDesktopStarted`, after the PipeWire stream is created, write `results["restore_token"]` to that group and call `sync()` (systemd stops krfb with a signal, so a write at exit is lost). Include `KConfigGroup` and `KSharedConfig`.
   3. In `framebuffers/pipewire/CMakeLists.txt`, link `KF5::ConfigCore` to `krfb_framebuffer_pw`.
   4. Build only the plugin: `cmake .. -DBUILD_TESTING=OFF` in a `build` folder, then `make krfb_framebuffer_pw`. Install `build/bin/pw.so` as `~/.local/lib/krfb-qtplugins/krfb/framebuffer/pw.so`. The system package is not changed, and an apt update does not overwrite the plugin.
4. Run krfb as a user unit, `~/.config/systemd/user/krfb.service`:

   ```ini
   [Unit]
   Description=krfb VNC through the COSMIC RemoteDesktop portal
   PartOf=graphical-session.target
   After=graphical-session.target

   [Service]
   Environment=QT_QPA_PLATFORM=wayland
   Environment=QT_PLUGIN_PATH=%h/.local/lib/krfb-qtplugins
   ExecStart=/usr/bin/krfb --nodialog
   Restart=on-failure
   RestartSec=5

   [Install]
   WantedBy=graphical-session.target
   ```

   Then `systemctl --user daemon-reload` and `systemctl --user enable --now krfb`. Check that the patched plugin is loaded: `grep pw.so /proc/$(systemctl --user show krfb -p MainPID --value)/maps` must show the path under `~/.local`.
5. Approve the dialog once with "Always allow", with all input types and the monitor. krfb saves the token in `~/.local/share/krfb/krfbstaterc`.
6. Check: `systemctl --user restart krfb`. The screen must come back within a few seconds with no dialog.

Limits: krfb shares one monitor. If the monitor layout changes, COSMIC may ask once more; choose "Always allow" again. To make it ask again, delete `krfbstaterc`. To undo the patch, remove the `QT_PLUGIN_PATH` line from the unit.

## 4. Build the desktop app

1. Install the build tools:
   - `sudo apt install build-essential curl wget file libssl-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev pkg-config`
   - Rust via rustup (stable)
   - bun
2. Get the code: clone `TheArctesian/anima` and check out `phase-2`. If the branch is not on GitHub yet, stop and tell me, and I will push it or copy a bundle over.
3. Build:
   - `bun install --frozen-lockfile`
   - `bun run --cwd apps/client build`
   - `bun run --cwd apps/desktop tauri build --bundles deb,appimage`

   A "no private key" error about the updater after the bundles are made is expected (the signing key is not set up yet).
4. Run the built app, not `tauri dev`: the hub only accepts the app's own origin.

## 5. First checks (report pass or fail with what you saw)

- Sign in with my Anima user name, password and authenticator code. I type them myself.
- The device list loads, and the "Not on the network" banner is gone.
- Add this computer: the app should offer "This computer" (R-060).
- Phase 2 acceptance O-3 (VNC to a Linux desktop): open VNC to this computer from the Mac, and from this computer to the Mac. See `docs/archive/phase-2/phase-2-acceptance.md`.
- Note every Linux-only failure (web view, key store, system-settings button, fonts, scaling) as a finding with the spec ID. Do not fix it in this session; list it for me.

At the end, give me:
- the 100.64 address and node name;
- the VNC server you chose;
- the app build result;
- the list of findings.
