"""Disposable VM installation request; production setup remains graphical."""
import shlex


def install_request(machine, password):
    machine.command("umask 077; anima-os-install inventory | jq -e '.[] | select(.path == \"/dev/vda\" and .eligible)' > /run/install-disk.json")
    machine.command(f"printf '%s\\n' {shlex.quote(password)} | mkpasswd --method=yescrypt --stdin > /run/install-password-hash; ckbcomp -layout us > /run/install-keymap")
    request = '''{system:($system|rtrimstr("\\n")),selected:.[0],data:[],confirmed:[],hashed_password:($password|rtrimstr("\\n")),keymap:$keymap}'''
    machine.command("jq -s --rawfile system /etc/anima-install-system --rawfile password /run/install-password-hash --rawfile keymap /run/install-keymap " + shlex.quote(request) + " /run/install-disk.json > /run/install-unconfirmed.json")
    confirm = '.confirmed = [.selected | {path, "maj:min", size, model, serial}]'
    machine.command("jq " + shlex.quote(confirm) + " /run/install-unconfirmed.json > /run/install.json; rm /run/install-password-hash")
