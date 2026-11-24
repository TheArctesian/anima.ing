#!/usr/bin/env python3
"""Read-only updater discovery. Never executes an upgrade or package refresh."""
import json
import platform
import shutil


def probe():
    release = platform.freedesktop_os_release()
    distro = release.get('ID', '')
    family = release.get('ID_LIKE', '').split()
    arch = distro in ('arch', 'omarchy') or 'arch' in family
    omarchy = shutil.which('omarchy') if distro == 'omarchy' else None
    pacman = shutil.which('pacman') if arch else None
    return {
        'os': distro,
        'version': release.get('VERSION_ID'),
        'preferred_system_update': [omarchy, 'update'] if omarchy else
        ([pacman, '-Syu'] if pacman else None),
        'arch_package_update': [pacman, '-Syu'] if pacman else None,
        'executed': False,
        'unverified': ['privileged remote execution', 'reboot and reconnect',
                       'operating-system rollback'],
    }


if __name__ == '__main__':
    print(json.dumps(probe(), indent=2))
