"""Host storage, acceleration and firmware for disposable x86-64 QEMU guests."""
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess


def accelerator():
    if platform.system() == 'Darwin':
        return 'hvf' if platform.machine() == 'x86_64' else 'tcg'
    return 'kvm'


def install_copy_timeout():
    return 4200 if accelerator() == 'tcg' else 1200


def firmware():
    if platform.system() != 'Darwin':
        return (Path('/usr/share/edk2/x64/OVMF_CODE.4m.fd'),
                Path('/usr/share/edk2/x64/OVMF_VARS.4m.fd'))
    binary = shutil.which('qemu-system-x86_64')
    if binary is None:
        raise ValueError('qemu-system-x86_64 must be on PATH')
    descriptors = Path(binary).resolve().parent.parent / 'share/qemu/firmware'
    for path in sorted(descriptors.glob('*.json')):
        document = json.loads(path.read_text())
        if document.get('description') != 'UEFI firmware for x86_64':
            continue
        mapping = document['mapping']
        code = Path(mapping['executable']['filename'])
        variables = Path(mapping['nvram-template']['filename'])
        if code.is_file() and variables.is_file():
            return code, variables
    raise ValueError('QEMU x86-64 UEFI firmware descriptor is missing')


def copy_firmware(output):
    code, variables = firmware()
    if not code.is_file() or not variables.is_file():
        raise ValueError('Install the x86-64 OVMF firmware before running VM checks')
    shutil.copyfile(variables, output / 'vars.fd')
    (output / 'vars.fd').chmod(0o600)


def require_storage(output, gib):
    if platform.system() == 'Darwin':
        # Resolve the mounted device rather than passing an arbitrary folder to diskutil.
        device = subprocess.check_output(['df', '-P', str(output)], text=True).splitlines()[-1].split()[0]
        volume = plistlib.loads(subprocess.check_output(['diskutil', 'info', '-plist', device]))
        if volume.get('FilesystemType') not in ('apfs', 'hfs') or volume.get('BusProtocol') == 'Disk Image':
            raise ValueError('Use a disk-backed APFS or HFS volume for VM checks')
    else:
        filesystem = subprocess.check_output(['stat', '-f', '-c', '%T', str(output)], text=True).strip()
        if filesystem in ('tmpfs', 'ramfs'):
            raise ValueError('Use disk-backed storage for VM checks')
    if shutil.disk_usage(output).free < gib * 1024**3:
        raise ValueError(f'VM checks need at least {gib} GiB free')


def copy_disk(source, target):
    if platform.system() == 'Darwin':
        subprocess.run(['qemu-img', 'convert', '-f', 'qcow2', '-O', 'qcow2', str(source), str(target)], check=True)
    else:
        subprocess.run(['cp', '--reflink=auto', '--sparse=always', str(source), str(target)], check=True)
    os.chmod(target, 0o600)
