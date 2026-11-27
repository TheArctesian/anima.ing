"""Global lock with a real terminal focused, on an isolated X display/session bus."""
import os
from pathlib import Path
import subprocess
import unittest
from PySide6.QtWidgets import QApplication

from app import Bridge
from session_lock import SessionLock
from terminal import AdminTerminal
from test_terminal import wait_until, descendants, named


class DesktopLock(unittest.TestCase):
    def test_shortcut_locks_with_terminal_focused(self):
        app = QApplication.instance() or QApplication([])
        app.setQuitOnLastWindowClosed(False)
        bridge = Bridge()
        bridge.telemetry_timer.stop()
        endpoint = SessionLock(bridge)
        terminal = AdminTerminal()
        bridge.locked.connect(terminal.close_session)
        desktop = subprocess.Popen([os.environ['ANIMA_TEST_OPENBOX'], '--config-file',
                                    os.environ['ANIMA_TEST_DESKTOP']])
        try:
            tool = '/run/current-system/sw/bin/xdotool'
            # Wait for the window manager to publish its root-window property.
            wait_until(lambda: subprocess.run(
                [tool, 'get_desktop'], stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL).returncode == 0, 'Desktop did not start')
            bridge.unlocked = True
            terminal.open_session()
            wait_until(lambda: terminal.process and terminal.process.processId() > 0,
                       'Terminal did not start')
            pid = terminal.process.processId()
            wait_until(lambda: named(descendants(pid), 'bash'), 'Shell did not start')
            window = subprocess.check_output([tool, 'search', '--pid', str(pid),
                                              '--class', 'XTerm'], text=True).splitlines()[0]
            subprocess.run([tool, 'windowfocus', '--sync', window], check=True)
            subprocess.run([tool, 'key', '--clearmodifiers', 'ctrl+alt+l'], check=True)
            wait_until(lambda: not bridge.unlocked, 'Global shortcut did not lock the session')
            wait_until(lambda: not terminal.isVisible() and not Path(f'/proc/{pid}').exists(),
                       'Lock left the administration terminal active')
        finally:
            terminal.close_session()
            endpoint.bus.unregisterService('ing.anima.Console')
            endpoint.bus.unregisterObject('/ing/anima/Console')
            desktop.terminate()
            desktop.wait(timeout=5)
            bridge.deleteLater()
            terminal.deleteLater()
            app.processEvents()


if __name__ == '__main__':
    unittest.main()
