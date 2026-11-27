"""Non-publishing source checks; Linux Nix/VM qualification is separate."""
import ast
from pathlib import Path
import subprocess
import shutil
import sys

ROOT = Path(__file__).resolve().parent.parent


def run(*arguments):
    subprocess.run(arguments, cwd=ROOT, check=True)


def main():
    paths = subprocess.check_output(
        ['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard', 'os'],
        cwd=ROOT).decode().split('\0')
    for name in sorted(set(filter(None, paths))):
        path = ROOT / name
        if not path.is_file():
            continue
        if path.suffix == '.py':
            ast.parse(path.read_text(), filename=name)
        elif path.suffix == '.nix' and shutil.which('nix-instantiate'):
            subprocess.run(['nix-instantiate', '--parse', name], cwd=ROOT,
                           stdout=subprocess.DEVNULL, check=True)
        elif path.suffix == '.sh':
            run('bash', '-n', name)
        elif path.suffix in {'.js', '.mjs'}:
            run('node', '--check', name)
    for folder in ['os/tests/vm', 'os/display', 'os/telemetry', 'os/updates', 'os/release']:
        run(sys.executable, '-m', 'unittest', 'discover', '-s', folder, '-p', 'test_*.py')
    if shutil.which('nix-instantiate') is None:
        print('Nix syntax checks skipped: nix-instantiate is unavailable.')
    print('OS source checks passed; Nix, Qt and installed-VM qualification remain separate.')


if __name__ == '__main__':
    main()
