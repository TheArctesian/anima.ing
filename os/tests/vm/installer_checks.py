"""Disposable VM installation request; production setup remains graphical."""
import shlex
import shutil
import subprocess

import vm_host


def prepare_serial_disk(iso, output):
    if not iso.is_file():
        raise ValueError('ISO file is missing')
    for command in ('qemu-img', 'qemu-system-x86_64'):
        if shutil.which(command) is None:
            raise ValueError(f'{command} must be on PATH')
    output.mkdir(parents=True, exist_ok=False)
    vm_host.require_storage(output, 64)
    disk = output / 'installed.qcow2'
    subprocess.run(['qemu-img', 'create', '-f', 'qcow2', str(disk), '128G'], check=True)
    vm_host.copy_firmware(output)
    with (output / 'sentinel.raw').open('wb') as stream:
        stream.write(b'ANIMA_ISO_UNSELECTED_DISK')
        stream.truncate(32*1024**3)
    return disk


def install_request(machine, password):
    machine.command("umask 077; anima-os-install inventory | jq -e '.[] | select(.path == \"/dev/vda\" and .eligible)' > /run/install-disk.json")
    machine.command(f"printf '%s\\n' {shlex.quote(password)} | mkpasswd --method=yescrypt --stdin > /run/install-password-hash; ckbcomp -layout us > /run/install-keymap")
    request = '''{system:($system|rtrimstr("\\n")),selected:.[0],data:[],confirmed:[],hashed_password:($password|rtrimstr("\\n")),keymap:$keymap}'''
    machine.command("jq -s --rawfile system /etc/anima-install-system --rawfile password /run/install-password-hash --rawfile keymap /run/install-keymap " + shlex.quote(request) + " /run/install-disk.json > /run/install-unconfirmed.json")
    confirm = '.confirmed = [.selected | {path, "maj:min", size, model, serial}]'
    machine.command("jq " + shlex.quote(confirm) + " /run/install-unconfirmed.json > /run/install.json; rm /run/install-password-hash")


def install_selected(machine):
    machine.command('anima-install < /run/install.json > /run/install-progress.jsonl',
                    timeout=vm_host.install_copy_timeout())
    machine.command("grep -q ANIMA_INSTALL_COMPLETE /run/install-progress.jsonl")
