set -eu
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH
[ "$(id -u)" = 0 ]
[ -d /run/systemd/system ]
[ -S /tmp/.X11-unix/X0 ]
user=ANIMA_LOGIN_USER
id "$user" >/dev/null
exec 9>/run/anima-vnc-setup.lock
flock -n 9 || exit 81
if ! command -v x11vnc >/dev/null; then
    command -v apt-get >/dev/null || exit 80
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y x11vnc
fi
# Authentication uses the machine's account; no password is stored in a unit or argument.
cat > /etc/systemd/system/anima-x11vnc.service <<UNIT
[Unit]
Description=Anima private screen sharing
After=display-manager.service
[Service]
# Anima encrypts the network hop; x11vnc still binds only loopback.
Environment=UNIXPW_DISABLE_SSL=1
ExecStart=/usr/bin/x11vnc -display :0 -auth guess -localhost -rfbport 5900 -forever -shared -unixpw $user
Restart=on-failure
RestartSec=5
[Install]
WantedBy=graphical.target
UNIT
systemctl daemon-reload
systemctl enable --now anima-x11vnc.service
