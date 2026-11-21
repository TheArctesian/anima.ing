"""Calamares adapter: use the same fail-closed offline installer as the CLI."""
import json
from pathlib import Path
import subprocess
import libcalamares


def pretty_name():
    return 'Installing Anima.ing'


def run():
    gs = libcalamares.globalstorage
    try:
        if gs.value('username') != 'admin':
            raise ValueError('The local owner account must be admin.')
        paths = gs.value('animaSelectedDisks') or []
        if not paths or paths != gs.value('animaAuthorizedDisks') or len(paths) != len(set(paths)):
            raise ValueError('Disk selections changed or were not authorized. Review again.')
        devices = json.loads(Path('/run/anima-installer-disks.json').read_text())
        selected = []
        for path in paths:
            matches = [d for d in devices if d['path'] == path]
            if len(matches) != 1:
                raise ValueError('Selected disk was not present when setup started.')
            selected.append(matches[0])
        password = libcalamares.utils.obscure(gs.value('password'))
        if len(password) < 12:
            raise ValueError('The owner password must contain at least 12 characters.')
        hashed = subprocess.run(['mkpasswd', '--method=yescrypt', '--stdin'],
                                input=password+'\n', text=True, capture_output=True, check=True).stdout.strip()
        del password
        gs.remove('password')
        keymap = subprocess.run(['ckbcomp', '-layout', gs.value('keyboardLayout') or 'us',
                                 '-variant', gs.value('keyboardVariant') or ''],
                                check=True, capture_output=True, text=True).stdout
        libcalamares.job.setprogress(0.05)
        request = {'system': '@installedSystem@', 'selected': selected[0],
                   'data': selected[1:], 'hashed_password': hashed, 'keymap': keymap,
                   'confirmed': [{key: disk.get(key) for key in
                                  ('path', 'maj:min', 'size', 'model', 'serial')}
                                 for disk in selected]}
        native_install(request)
        del request, hashed
        libcalamares.job.setprogress(1.0)
        libcalamares.utils.debug('ANIMA_INSTALL_COMPLETE')
    except subprocess.CalledProcessError as error:
        # Report the operation and status, never its arguments or captured output.
        operation = Path(error.cmd[0]).name
        return ('Installation stopped', f'{operation} failed (exit {error.returncode}). Check the local system journal.')
    except (ValueError, OSError) as error:
        return ('Installation stopped', str(error))
    return None


def native_install(request):
    with subprocess.Popen(['@installerProgram@', 'install', '--system', '@installedSystem@'],
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          stderr=subprocess.DEVNULL, text=True) as process:
        process.stdin.write(json.dumps(request))
        process.stdin.close()
        succeeded = False
        failure = 'The native installer stopped. Inspect the local system journal.'
        while True:
            line = process.stdout.readline(65537)
            if not line:
                break
            if len(line) > 65536:
                raise ValueError('The installer returned an invalid progress event.')
            event = json.loads(line)
            if event.get('state') == 'failed':
                failure = event.get('error', failure)
            progress = event.get('progress')
            if isinstance(progress, (int, float)) and 0 <= progress <= 1:
                libcalamares.job.setprogress(progress)
            succeeded = event.get('state') == 'succeeded' or succeeded
        if process.wait() != 0 or not succeeded:
            raise ValueError(failure)
