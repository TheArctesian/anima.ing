"""Skip source trees already released; reject sources older than the public release."""
import json
import os
from pathlib import Path
import subprocess
import urllib.error
from channel import public_json
from receipt import fingerprint


def select():
    current = public_json('latest-iso.json')
    try:
        previous = public_json(current['version'] + '/release.json')
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        previous = None
    needed = previous is None or previous['sourceFingerprint'] != fingerprint()
    if previous:
        subprocess.run(['git', 'merge-base', '--is-ancestor', previous['sourceRevision'], 'HEAD'], check=True)
    with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
        output.write('needed=' + str(needed).lower() + '\n')


if __name__ == '__main__':
    select()
