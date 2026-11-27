"""Real embedded terminal lifecycle on an isolated Linux X display."""
import os
import pwd
from pathlib import Path
import signal
import subprocess
import time
import unittest
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication, QPushButton
from terminal import AdminTerminal


def wait_until(predicate, description):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        QApplication.processEvents()
        observed = predicate()
        if observed:
            return observed
        QTest.qWait(50)
    raise AssertionError(description)


def descendants(pid):
    found = set()
    pending = [pid]
    while pending:
        parent = pending.pop()
        try:
            children = Path(f'/proc/{parent}/task/{parent}/children').read_text().split()
        except FileNotFoundError:
            continue
        for child in map(int, children):
            if child not in found:
                found.add(child)
                pending.append(child)
    return found


def named(pids, name):
    for pid in pids:
        try:
            if Path(f'/proc/{pid}/comm').read_text().strip() == name:
                return pid
        except FileNotFoundError:
            continue
    return None


def login_shell(pids, name):
    expected = os.fsencode('-' + name)
    for pid in pids:
        try:
            invocation = Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0', 1)[0]
        except FileNotFoundError:
            continue
        if invocation == expected:
            return pid
    return None


class TerminalLifecycle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])
        cls.app.setQuitOnLastWindowClosed(False)

    def setUp(self):
        self.terminal = AdminTerminal()
        self.addCleanup(self.terminal.close_session)
        self.terminal.open_session()
        wait_until(lambda: self.terminal.process is not None
                   and self.terminal.process.processId() > 0, 'Terminal did not start')
        self.pid = self.terminal.process.processId()
        shell = Path(pwd.getpwuid(os.getuid()).pw_shell).name
        wait_until(lambda: login_shell(descendants(self.pid), shell), 'Account login shell did not start')
        self.window = subprocess.check_output([
            '/run/current-system/sw/bin/xdotool', 'search', '--pid', str(self.pid),
            '--class', 'XTerm',
        ], text=True).splitlines()[0]

    def tearDown(self):
        current = self.terminal.process
        pids = {self.pid}
        if current and current.processId():
            pids.add(current.processId())
        self.terminal.close_session()
        wait_until(lambda: all(not Path(f'/proc/{pid}').exists() for pid in pids),
                   'Terminal process survived close')
        self.terminal.deleteLater()
        QApplication.processEvents()

    def enter(self, command):
        program = '/run/current-system/sw/bin/xdotool'
        subprocess.run([program, 'windowfocus', '--sync', self.window], check=True)
        subprocess.run([program, 'type', '--clearmodifiers', '--delay', '30', command], check=True)
        subprocess.run([program, 'key', 'Return'], check=True)

    def test_close_ends_shell_and_foreground_process(self):
        self.enter('sleep 120')
        wait_until(lambda: named(descendants(self.pid), 'sleep'), 'Foreground command did not start')
        processes = descendants(self.pid) | {self.pid}
        close = next(button for button in self.terminal.findChildren(QPushButton)
                     if button.accessibleName() == 'Close administration terminal')
        close.click()
        wait_until(lambda: not self.terminal.isVisible(), 'Terminal remained visible after close')
        wait_until(lambda: all(not Path(f'/proc/{pid}').exists() for pid in processes),
                   'Closing the terminal left its shell or foreground command alive')

    def test_shell_exit_dismisses_terminal(self):
        self.enter('exit')
        wait_until(lambda: not self.terminal.isVisible(), 'Shell exit left an empty terminal window')
        wait_until(lambda: not Path(f'/proc/{self.pid}').exists(), 'Shell exit left xterm alive')

    def pause_and_reopen(self):
        os.kill(self.pid, signal.SIGSTOP)
        wait_until(lambda: Path(f'/proc/{self.pid}/stat').read_text().rsplit(')', 1)[1].startswith(' T'),
                   'The old terminal did not stop')
        self.terminal.close_session()
        self.terminal.open_session()
        self.assertTrue(Path(f'/proc/{self.pid}').exists())
        self.assertFalse(self.terminal.isVisible(), 'Reopening exposed the terminal before the old xterm exited')

    def test_reopen_waits_for_previous_xterm(self):
        self.pause_and_reopen()
        wait_until(lambda: not Path(f'/proc/{self.pid}').exists(), 'The old xterm did not exit')
        wait_until(lambda: self.terminal.isVisible() and self.terminal.process
                   and self.terminal.process.processId() not in (0, self.pid),
                   'The terminal did not open a fresh session after cleanup')

    def test_close_cancels_a_queued_reopen(self):
        self.pause_and_reopen()
        self.terminal.close_session()
        wait_until(lambda: not Path(f'/proc/{self.pid}').exists(), 'The old xterm did not exit')
        QTest.qWait(250)
        self.assertFalse(self.terminal.isVisible(), 'The terminal reopened after the close request')


if __name__ == '__main__':
    unittest.main()
