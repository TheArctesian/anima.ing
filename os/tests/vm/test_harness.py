"""The runner must not report success after detecting an unsafe test result."""
import json
import os
from pathlib import Path
import pty
import pwd
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import vm


class ShellTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        master, slave = pty.openpty()
        process = subprocess.Popen(
            ['/bin/bash', '--noprofile', '--norc', '-i'],
            stdin=slave, stdout=slave, stderr=slave,
            env={'PATH': '/usr/bin:/bin', 'PS1': '\n ANIMA ❯ ', 'TERM': 'dumb',
                 'HISTFILE': '/dev/null'})
        os.close(slave)
        self.machine = vm.Machine.__new__(vm.Machine)
        self.machine.master = master
        self.machine.process = process
        self.machine.buffer = ''
        self.machine.offset = 0
        self.machine.log = (Path(self.folder.name) / 'serial.log').open('w')
        self.addCleanup(self.machine.close)

    def test_shell_handshake_ignores_theme_and_checks_actual_user(self):
        self.machine.shell(pwd.getpwuid(os.getuid()).pw_name, timeout=5)
        self.machine.command('test "$((2+3))" = 5', timeout=5)
        with self.assertRaisesRegex(RuntimeError, 'Unexpected guest user'):
            self.machine.shell('not-the-fixture-user', timeout=5)

    def test_shell_handshake_recovers_when_login_discards_early_input(self):
        self.machine.send("printf '\\nLOGIN_READY\\n'; read -r discarded")
        self.machine.expect(r'\nLOGIN_READY\r*\n', timeout=5)
        self.machine.shell(pwd.getpwuid(os.getuid()).pw_name, timeout=5)
        self.machine.command('test "$((2+3))" = 5', timeout=5)


class ResultTests(unittest.TestCase):
    def test_drive_option_syntax_in_a_path_is_refused_before_starting_qemu(self):
        with tempfile.TemporaryDirectory(prefix='anima,') as directory:
            output = Path(directory)
            with self.assertRaisesRegex(ValueError, 'commas'):
                vm.Machine(output, output / 'disk')
            self.assertFalse((output / 'serial.log').exists())

    def test_install_media_cannot_inject_qemu_drive_options(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            with self.assertRaisesRegex(ValueError, 'commas'):
                vm.Machine(output, output / 'disk', boot_iso=output / 'installer,format=raw.iso')
            self.assertFalse((output / 'serial.log').exists())

    def test_changed_unselected_disk_fails_the_run_and_keeps_evidence(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            with patch.object(vm, 'prepare', return_value=(output / 'disk', 'before')), \
                 patch.object(vm, 'Machine'), patch.object(vm, 'digest', return_value='changed'):
                with self.assertRaisesRegex(RuntimeError, 'VM checks failed'):
                    vm.run(output / 'source', output)
            result = json.loads((output / 'results.json').read_text())[-1]
            self.assertEqual(result['case'], 'unselected-disk')
            self.assertEqual(result['result'], 'Failed')

    def test_zero_iterations_is_an_error_before_creating_output(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder) / 'unused'
            result = subprocess.run([
                sys.executable, str(Path(__file__).with_name('faults.py')),
                'missing.qcow2', str(output), '--scenario', 'shutdown-hang',
                '--iterations', '0',
            ], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('--iterations must be positive', result.stderr)
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
