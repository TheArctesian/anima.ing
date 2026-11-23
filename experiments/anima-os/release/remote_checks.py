"""Exercise real private SSH, RDP TLS, desktop login and persistent owner choices."""
import json
from pathlib import Path
import shlex
from update_test import login
import update_fixture


def check(machine, password, package):
    tools = json.loads((package / 'test-tools.json').read_text())
    machine.command('systemctl is-active sshd xrdp xrdp-sesman; '
                    '! ss -ltnH | grep -E "(0.0.0.0|\\*|\\[::\\]):(22|3389) "; '
                    '! ss -ltnH | grep -q ":5900 "')
    update_fixture.serve(machine)
    machine.command('cp /persist/ssh/ssh_host_ed25519_key.pub /run/anima-test-ssh.pub; '
                    'cp /persist/rdp/cert.pem /run/anima-test-rdp.pem; chmod 644 /run/anima-test-*; '
                    'cp /run/update-origin/os/preview/x86_64-linux/private-access.test /tmp/private-access.test; '
                    'chmod 755 /tmp/private-access.test; '
                    'sha256sum /persist/ssh/ssh_host_ed25519_key.pub /persist/rdp/cert.pem >/persist/remote-identity.sha256')
    machine.send("read -rsp 'FIXTURE_PASSWORD> ' ANIMA_OS_TEST_PASSWORD")
    machine.expect('FIXTURE_PASSWORD> '); machine.send(password)
    machine.expect(r'root@anima(?:-node-[a-z0-9]{4})?:[^\r\n]*#')
    machine.command('export ANIMA_OS_TEST_PASSWORD; runuser -u anima-daemon --preserve-environment -- '
                    '/tmp/private-access.test -test.run "^TestOSPrivateSSHAndRDPTLS$" -test.v; '
                    'umask 077; printf "%s\\n" "$ANIMA_OS_TEST_PASSWORD" >/run/rdp-test-password; '
                    'unset ANIMA_OS_TEST_PASSWORD', timeout=180)
    machine.command('nix-store --import </run/update-origin/os/preview/x86_64-linux/test-tools.nar >/tmp/tools.log', timeout=180)
    machine.command(f'systemd-run --unit=rdp-test-display {shlex.quote(tools["xorg"] + "/bin/Xvfb")} :99 -screen 0 1280x800x24; sleep 2')
    client = shlex.quote(tools['freerdp'] + '/bin/xfreerdp')
    openssl = shlex.quote(tools['openssl'] + '/bin/openssl')
    machine.command(f'cert=$({openssl} x509 -in /persist/rdp/cert.pem -noout -fingerprint -sha256 | cut -d= -f2 | tr -d :); '
                    'systemd-run --unit=rdp-test-client --uid=root --property=SetLoginEnvironment=yes --setenv=DISPLAY=:99 '
                    f'sh -c \'exec {client} /v:127.0.0.1 /u:admin /from-stdin:force /sec:tls '
                    '/cert:fingerprint:sha256:\'"$cert"\' /size:1280x800 < /run/rdp-test-password\'; '
                    'timeout 90 sh -c \'until test "$(pgrep -u admin -f anima-local-console-source/app.py | wc -l)" -ge 2; do sleep 2; done\'', timeout=100)
    machine.command("runuser -u anima-daemon -- anima-link ports -state /var/lib/anima-link off 3389 | jq -e '.open == [22]'")
    machine.send('systemctl reboot'); login(machine, password)
    machine.command("sha256sum -c /persist/remote-identity.sha256; systemctl is-active sshd xrdp; "
                    "runuser -u anima-daemon -- anima-link ports -state /var/lib/anima-link | jq -e '.open == [22]'; "
                    "runuser -u anima-daemon -- anima-link ports -state /var/lib/anima-link on 3389 | jq -e '.open == [22,3389]'")
