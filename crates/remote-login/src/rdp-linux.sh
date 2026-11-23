set -eu
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH
[ "$(id -u)" = 0 ]
[ -d /run/systemd/system ]
exec 9>/run/anima-rdp-setup.lock
flock -n 9 || exit 81
# An existing installation belongs to its administrator; never replace its configuration.
if [ ! -f /etc/anima/xrdp.ini ]; then
    [ ! -e /etc/xrdp/xrdp.ini ] || [ -f /etc/anima/xrdp-installing ] || exit 80
    command -v apt-get >/dev/null || exit 80
    install -d -m 755 /etc/anima
    touch /etc/anima/xrdp-installing
    for unit in xrdp.service xrdp-sesman.service; do
        [ "$(readlink "/etc/systemd/system/$unit" || true)" = /dev/null ] && continue
        [ ! -e "/etc/systemd/system/$unit" ] && [ ! -L "/etc/systemd/system/$unit" ] || exit 82
    done
    systemctl mask xrdp.service xrdp-sesman.service
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y xrdp xorgxrdp
    install -d -m 755 /etc/anima
    # Only the global listener changes; session definitions keep their original ports.
    awk '/^\[Globals\]/{global=1} /^\[/ && !/^\[Globals\]/{global=0} global && /^port=/{print "port=tcp://127.0.0.1:3389";next} {print}' /etc/xrdp/xrdp.ini > /etc/anima/xrdp.ini.next
    grep -qx 'port=tcp://127.0.0.1:3389' /etc/anima/xrdp.ini.next
    mv /etc/anima/xrdp.ini.next /etc/anima/xrdp.ini
    rm /etc/anima/xrdp-installing
fi
cat > /etc/systemd/system/anima-xrdp-sesman.service <<'UNIT'
[Unit]
Description=Anima private desktop sessions
[Service]
ExecStart=/usr/sbin/xrdp-sesman --nodaemon
RuntimeDirectory=xrdp
[Install]
WantedBy=multi-user.target
UNIT
cat > /etc/systemd/system/anima-xrdp.service <<'UNIT'
[Unit]
Description=Anima private RDP
Requires=anima-xrdp-sesman.service
After=anima-xrdp-sesman.service
[Service]
ExecStart=/usr/sbin/xrdp --nodaemon --config /etc/anima/xrdp.ini
Restart=on-failure
[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload
systemctl enable --now anima-xrdp.service
