"""Exercise installed private SSH, RDP and VNC with disposable credentials."""
import json
from pathlib import Path
import shlex

TOOL_FILES = ('test-tools.json', 'test-tools.nar', 'private-access.test')


def check(machine, password, package):
    tools = json.loads((package / 'test-tools.json').read_text())
    machine.command('for unit in sshd xrdp xrdp-sesman anima-shared-session anima-vnc; do '
                    'systemctl is-active "$unit"; done; '
                    'ss -ltnH | awk \'$4 ~ /:(22|3389|5900)$/ && '
                    '$4 !~ /^(127[.]0[.]0[.]1|\\[::1\\]):/ {print $4; exposed=1} '
                    'END {exit exposed}\'')
    machine.command('cp /persist/ssh/ssh_host_ed25519_key.pub /run/anima-test-ssh.pub; '
                    'cp /persist/rdp/cert.pem /run/anima-test-rdp.pem; '
                    'cp /persist/vnc/identity/cert.pem /run/anima-test-vnc.pem; '
                    'chmod 644 /run/anima-test-ssh.pub /run/anima-test-rdp.pem /run/anima-test-vnc.pem; '
                    'install -d -m 0700 -o anima-daemon /run/anima-test-vnc; '
                    'cp /run/update-origin/os/preview/x86_64-linux/private-access.test /tmp/private-access.test; '
                    'chmod 755 /tmp/private-access.test; '
                    'sha256sum /persist/ssh/ssh_host_ed25519_key.pub /persist/rdp/cert.pem '
                    '/persist/vnc/identity/cert.pem >/persist/remote-identity.sha256')
    machine.command('nix-store --import </run/update-origin/os/preview/x86_64-linux/test-tools.nar >/tmp/tools.log', timeout=180)
    capture = shlex.quote(tools['imagemagick'] + '/bin/import')
    ocr = shlex.quote(tools['tesseract'] + '/bin/tesseract')
    observe = (f'until XAUTHORITY=/run/anima-console/Xauthority {capture} -display :99 -window root /run/shared-frame.png && '
               f'{ocr} /run/shared-frame.png stdout 2>/dev/null | grep -q "Local session locked"; do sleep 2; done')
    machine.command('timeout 90 sh -c ' + shlex.quote(observe), timeout=100)
    machine.send("read -rsp 'FIXTURE_PASSWORD> ' ANIMA_OS_TEST_PASSWORD")
    machine.expect('FIXTURE_PASSWORD> '); machine.send(password)
    machine.shell('root')
    machine.command('DISPLAY=:99 XAUTHORITY=/run/anima-console/Xauthority xdotool mousemove 9 10; '
                    'export ANIMA_OS_TEST_PASSWORD; runuser -u anima-daemon --preserve-environment -- '
                    '/tmp/private-access.test -test.run "^TestOSPrivateAccess$" -test.v | tee /run/private-access.log; '
                    'grep -q "^--- PASS: TestOSPrivateAccess (" /run/private-access.log; '
                    'umask 077; printf "%s\\n" "$ANIMA_OS_TEST_PASSWORD" >/run/rdp-test-password; '
                    'unset ANIMA_OS_TEST_PASSWORD', timeout=180)
    machine.command(f'{ocr} /run/anima-test-vnc/frame.png stdout 2>/dev/null | grep -F "Local session locked"; '
                    'pointer=$(DISPLAY=:99 XAUTHORITY=/run/anima-console/Xauthority xdotool getmouselocation --shell); '
                    'grep -Fx "X=173" <<< "$pointer"; grep -Fx "Y=127" <<< "$pointer"')
    machine.command(f'systemd-run --unit=rdp-test-display {shlex.quote(tools["xorg"] + "/bin/Xvfb")} :98 -screen 0 1280x800x24; sleep 2')
    client = shlex.quote(tools['freerdp'] + '/bin/xfreerdp')
    openssl = shlex.quote(tools['openssl'] + '/bin/openssl')
    machine.command('systemctl show anima-shared-session --property=MainPID --value >/run/shared-console-before; '
                    'test "$(cat /run/shared-console-before)" -gt 0')
    machine.command(f'cert=$({openssl} x509 -in /persist/rdp/cert.pem -noout -fingerprint -sha256 | cut -d= -f2 | tr -d :); '
                    'systemd-run --unit=rdp-test-client --uid=root --property=SetLoginEnvironment=yes --setenv=DISPLAY=:98 '
                    f'sh -c \'exec {client} /v:127.0.0.1 /u:admin /from-stdin:force /sec:tls '
                    '/cert:fingerprint:sha256:\'"$cert"\' /size:1280x800 < /run/rdp-test-password\'')
    observe = (f'until {capture} -display :98 -window root /run/rdp-frame.png && '
               f'{ocr} /run/rdp-frame.png stdout 2>/dev/null | grep -q "Local session locked"; '
               'do sleep 2; done')
    try:
        machine.command('timeout 90 sh -c ' + shlex.quote(observe), timeout=100)
        machine.command('systemctl show anima-shared-session --property=MainPID --value | cmp /run/shared-console-before -; '
                        'test "$(pgrep -u admin -f anima-local-console-source/app.py | wc -l)" -eq 1')
    finally:
        machine.command('systemctl stop rdp-test-client rdp-test-display; rm -f /run/rdp-test-password')
    machine.command("runuser -u anima-daemon -- anima-link ports -state /var/lib/anima-link off 3389 | jq -e '.open == [22,5900]'")
    machine.command("runuser -u anima-daemon -- anima-link ports -state /var/lib/anima-link off 5900 | jq -e '.open == [22]'")
    from update_test import login
    machine.send('systemctl reboot'); login(machine, password)
    machine.command("sha256sum -c /persist/remote-identity.sha256; "
                    "for unit in sshd xrdp anima-vnc; do systemctl is-active \"$unit\"; done; "
                    "runuser -u anima-daemon -- anima-link ports -state /var/lib/anima-link | jq -e '.open == [22]'; "
                    "runuser -u anima-daemon -- anima-link ports -state /var/lib/anima-link on 5900 | jq -e '.open == [22,5900]'; "
                    "runuser -u anima-daemon -- anima-link ports -state /var/lib/anima-link on 3389 | jq -e '.open == [22,3389,5900]'")
