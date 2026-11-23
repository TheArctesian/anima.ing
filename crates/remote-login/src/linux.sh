set -eu
PATH=/usr/sbin:/usr/bin:/sbin:/bin:/run/current-system/sw/bin
export PATH
[ "$(id -u)" = 0 ]
user=ANIMA_LOGIN_USER
existing=ANIMA_SSH_EXISTS
id "$user" >/dev/null
exec 9>/run/anima-ssh-setup.lock
flock -n 9 || exit 81
if [ "$existing" = no ]; then
    [ -d /run/systemd/system ] || exit 80
    sshd=$(command -v sshd || true)
    if [ -z "$sshd" ]; then
        command -v apt-get >/dev/null || command -v dnf >/dev/null || command -v pacman >/dev/null || command -v zypper >/dev/null || exit 80
        # Packages must not start a public SSH listener, now or at the next boot.
        for unit in ssh.service ssh.socket sshd.service sshd.socket; do
            [ -e "/etc/systemd/system/$unit" ] && exit 82
            [ -L "/etc/systemd/system/$unit" ] && exit 82
        done
        systemctl mask ssh.service ssh.socket sshd.service sshd.socket
        if command -v apt-get >/dev/null; then
            apt-get update
            DEBIAN_FRONTEND=noninteractive apt-get install -y openssh-server
        elif command -v dnf >/dev/null; then
            dnf install -y openssh-server
        elif command -v pacman >/dev/null; then
            pacman -S --needed --noconfirm openssh
        elif command -v zypper >/dev/null; then
            zypper --non-interactive install openssh
        else
            exit 80
        fi
        sshd=$(command -v sshd)
    fi
    install -d -m 755 /etc/ssh
    if [ ! -f /etc/ssh/ssh_host_anima_ed25519_key ]; then
        ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_anima_ed25519_key
    fi
    install -d -m 755 /run/sshd
    cat > /etc/ssh/sshd_config.anima <<CONFIG
Port 22
ListenAddress 127.0.0.1
HostKey /etc/ssh/ssh_host_anima_ed25519_key
PidFile /run/anima-sshd.pid
PermitRootLogin no
PermitEmptyPasswords no
PasswordAuthentication yes
KbdInteractiveAuthentication no
UsePAM yes
AllowUsers $user
Subsystem sftp internal-sftp
CONFIG
    if command -v restorecon >/dev/null; then restorecon /etc/ssh/ssh_host_anima_ed25519_key* /etc/ssh/sshd_config.anima; fi
    "$sshd" -t -f /etc/ssh/sshd_config.anima
    cat > /etc/systemd/system/anima-sshd.service <<UNIT
[Unit]
Description=Anima private SSH
After=network.target
[Service]
Type=simple
ExecStart=$sshd -D -e -f /etc/ssh/sshd_config.anima
Restart=on-failure
RuntimeDirectory=sshd
RuntimeDirectoryMode=0755
RuntimeDirectoryPreserve=yes
[Install]
WantedBy=multi-user.target
UNIT
    systemctl daemon-reload
    systemctl enable --now anima-sshd.service
fi
install -d -m 755 /etc/anima
if [ ! -e /etc/anima/remote-access.json ]; then
    temporary=$(mktemp /etc/anima/.remote-access.XXXXXX)
    printf '{"sshUser":"%s","rdpUser":null}\n' "$user" > "$temporary"
    chmod 644 "$temporary"
    ln "$temporary" /etc/anima/remote-access.json
    rm "$temporary"
fi
