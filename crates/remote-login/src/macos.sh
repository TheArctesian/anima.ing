set -eu
PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH
[ "$(id -u)" = 0 ]
user=ANIMA_LOGIN_USER
existing=ANIMA_SSH_EXISTS
id "$user" >/dev/null
mkdir /var/run/anima-ssh-setup.lock || exit 81
trap 'rmdir /var/run/anima-ssh-setup.lock' EXIT
if [ "$existing" = no ]; then
    install -d -m 700 /etc/anima/ssh
    if [ ! -f /etc/anima/ssh/host_ed25519 ]; then
        ssh-keygen -q -t ed25519 -N '' -f /etc/anima/ssh/host_ed25519
    fi
    cat > /etc/anima/ssh/sshd_config <<CONFIG
Port 22
ListenAddress 127.0.0.1
HostKey /etc/anima/ssh/host_ed25519
PermitRootLogin no
PermitEmptyPasswords no
PasswordAuthentication yes
KbdInteractiveAuthentication no
UsePAM yes
AllowUsers $user
Subsystem sftp internal-sftp
CONFIG
    /usr/sbin/sshd -t -f /etc/anima/ssh/sshd_config
    cat > /Library/LaunchDaemons/ing.anima.private-ssh.plist <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>ing.anima.private-ssh</string>
<key>ProgramArguments</key><array><string>/usr/sbin/sshd</string><string>-D</string><string>-e</string><string>-f</string><string>/etc/anima/ssh/sshd_config</string></array>
<key>RunAtLoad</key><true/><key>KeepAlive</key><true/>
</dict></plist>
PLIST
    chmod 644 /Library/LaunchDaemons/ing.anima.private-ssh.plist
    launchctl print system/ing.anima.private-ssh >/dev/null 2>&1 || launchctl bootstrap system /Library/LaunchDaemons/ing.anima.private-ssh.plist
    launchctl kickstart system/ing.anima.private-ssh
fi
install -d -m 755 /etc/anima
if [ ! -e /etc/anima/remote-access.json ]; then
    temporary=$(mktemp /etc/anima/.remote-access.XXXXXX)
    printf '{"sshUser":"%s","rdpUser":null}\n' "$user" > "$temporary"
    chmod 644 "$temporary"
    ln "$temporary" /etc/anima/remote-access.json
    rm "$temporary"
fi
