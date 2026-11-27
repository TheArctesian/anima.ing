"""Unprivileged fullscreen web UI. Only packaged local content receives the native bridge."""
import json
import os
import re
from pathlib import Path
import sys
from PySide6.QtCore import QObject, QProcess, QProcessEnvironment, Qt, QTimer, QUrl, Signal, Slot
from PySide6.QtGui import QColor
from PySide6.QtWidgets import QApplication, QMainWindow
from PySide6.QtWebChannel import QWebChannel
from PySide6.QtWebEngineCore import QWebEnginePage, QWebEngineProfile, QWebEngineScript, QWebEngineSettings
from PySide6.QtWebEngineWidgets import QWebEngineView

from signin import SigninWindow
from screen import ScreenOff
from physical_control import PhysicalScreenOff, environment as physical_environment
from terminal import AdminTerminal
from session_lock import SessionLock
from telemetry_snapshot import read_frame as local_observation

ROOT = Path(__file__).resolve().parent


class LocalPage(QWebEnginePage):
    def __init__(self, profile, parent):
        super().__init__(profile, parent)
        if Path('/persist/anima-reduced-motion').is_file():
            script = QWebEngineScript()
            script.setInjectionPoint(QWebEngineScript.InjectionPoint.DocumentCreation)
            script.setWorldId(QWebEngineScript.ScriptWorldId.MainWorld.value)
            script.setSourceCode('window.animaInstallerReducedMotion = true;')
            self.scripts().insert(script)

    def acceptNavigationRequest(self, url, kind, main_frame):
        return url.isLocalFile() and Path(url.toLocalFile()).resolve() == ROOT / 'index.html'


class Bridge(QObject):
    telemetryChanged = Signal(str)
    answer = Signal(str)
    locked = Signal()
    openBrowser = Signal(str, str)
    closeBrowser = Signal()
    blankScreen = Signal()
    openTerminal = Signal()
    openApplicationTerminal = Signal(str, str, str)

    def __init__(self):
        super().__init__()
        self.process = None
        self.buffer = b''
        self.pending = {}
        self.unlocked = False
        self.telemetry_timer = QTimer(self)
        self.telemetry_timer.timeout.connect(self.telemetry)
        self.telemetry_timer.start(5000)

    @Slot()
    def telemetry(self):
        self.telemetryChanged.emit(local_observation())

    @Slot()
    def screenOff(self):
        self.lock()
        self.blankScreen.emit()

    @Slot()
    def terminal(self):
        if self.unlocked:
            self.openTerminal.emit()

    @Slot(str, str, str)
    def applicationTerminal(self, container, name, user):
        if (self.unlocked and re.fullmatch(r'[0-9a-f]{64}', container)
                and re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]{0,127}', name)
                and re.fullmatch(r'[1-9][0-9]{0,9}:[1-9][0-9]{0,9}', user)):
            self.openApplicationTerminal.emit(container, name, user)

    def emit_error(self, identity, message):
        self.answer.emit(json.dumps({'id': identity, 'ok': False, 'error': message}))

    @Slot(str)
    def request(self, text):
        try:
            request = json.loads(text)
            identity, op = request['id'], request['op']
            if op == 'lock':
                self.lock()
                return
            if op == 'unlock':
                if self.process:
                    self.emit_error(identity, 'Authentication is already in progress.')
                    return
                self.unlock(request['args']['password'])
                return
            if not self.unlocked:
                self.emit_error(identity, 'Unlock the local session first.')
                return
            reads = ('status', 'diagnostics', 'update-status', 'displays', 'activity', 'code-attention', 'data-volumes',
                     'workloads', 'work-admission', 'workload-details', 'workload-logs', 'workload-build-logs', 'workload-terminal-target',
                     'remote-access-status',
                     'maintenance-status', 'maintenance-impact', 'migration-accounts', 'migration-inventory')
            if identity in self.pending:
                self.emit_error(identity, 'This operation is already running.')
                return
            if op not in reads and any(active not in reads for active in self.pending.values()):
                self.emit_error(identity, 'Wait for the current operation to finish.')
                return
            self.pending[identity] = op
            self.process.write((json.dumps(request)+'\n').encode())
        except (ValueError, KeyError, TypeError):
            self.emit_error('invalid', 'Invalid local request.')

    def unlock(self, password):
        if not isinstance(password, str) or '\n' in password or '\r' in password:
            self.emit_error('unlock', 'Invalid password.')
            return
        self.buffer = b''
        process = QProcess(self)
        self.process = process
        process.readyReadStandardOutput.connect(lambda: self.read(process))
        process.finished.connect(lambda: self.ended(process))
        process.errorOccurred.connect(lambda: self.ended(process))
        if os.environ.get('ANIMA_SHARED_CONSOLE') == '1':
            environment = QProcessEnvironment.systemEnvironment()
            physical = physical_environment()
            environment.insert('DISPLAY', physical.get('DISPLAY', ':0'))
            environment.insert('XAUTHORITY', physical.get('XAUTHORITY', '/run/anima-console/no-physical-authority'))
            process.setProcessEnvironment(environment)
        process.start('/run/wrappers/bin/sudo', ['-k', '-S', '-p', '', '--preserve-env=DISPLAY,XAUTHORITY', '/run/current-system/sw/bin/anima-console-privileged'])
        process.write((password+'\n').encode())
        QTimer.singleShot(15000, lambda: self.auth_timeout(process))

    def auth_timeout(self, process):
        if process is self.process and not self.unlocked:
            self.emit_error('unlock', 'Authentication timed out. Try again.')
            self.lock()

    def read(self, process):
        if process is not self.process:
            process.readAllStandardOutput()
            return
        self.buffer += bytes(process.readAllStandardOutput())
        while b'\n' in self.buffer:
            line, self.buffer = self.buffer.split(b'\n', 1)
            try:
                answer = json.loads(line)
            except ValueError:
                continue
            operation = self.pending.pop(answer.get('id'), None)
            if answer['id'] == 'unlock' and answer['ok']:
                self.unlocked = True
            value = answer.get('value', {})
            if isinstance(value, dict):
                if value.get('url'):
                    self.openBrowser.emit(value['url'], value.get('userCode', ''))
                if (answer.get('ok') and (value.get('signedIn') or operation == 'cancel-sign-in')) or (operation == 'sign-in-poll' and not answer.get('ok')):
                    self.closeBrowser.emit()
            self.answer.emit(json.dumps(answer))

    def ended(self, process):
        if process is not self.process:
            process.deleteLater()
            return
        self.process = None
        self.pending = {}
        if not self.unlocked:
            self.emit_error('unlock', 'Authentication failed. Enter the local admin password.')
        self.unlocked = False
        self.locked.emit()
        process.deleteLater()

    @Slot()
    def lock(self):
        process = self.process
        self.process = None
        self.unlocked = False
        self.pending = {}
        self.buffer = b''
        if process:
            process.closeWriteChannel()
        self.closeBrowser.emit()
        self.locked.emit()


def main():
    app = QApplication(sys.argv)
    app.setApplicationName('Anima local console')
    window = QMainWindow()
    window.setWindowFlags(Qt.WindowType.FramelessWindowHint)
    view = QWebEngineView(window)
    profile = QWebEngineProfile('AnimaConsole', view)
    page = LocalPage(profile, view)
    page.setBackgroundColor(QColor('#1B1D21'))
    page.settings().setAttribute(QWebEngineSettings.WebAttribute.LocalContentCanAccessRemoteUrls, False)
    page.settings().setAttribute(QWebEngineSettings.WebAttribute.LocalContentCanAccessFileUrls, True)
    view.setPage(page)
    view.setContextMenuPolicy(Qt.ContextMenuPolicy.NoContextMenu)
    bridge = Bridge()
    session_lock = SessionLock(bridge)
    channel = QWebChannel(page)
    channel.registerObject('console', bridge)
    page.setWebChannel(channel)
    window.setCentralWidget(view)
    browser = SigninWindow()
    terminal = AdminTerminal()
    bridge.telemetryChanged.connect(browser.remote_connections.update_frame)
    bridge.telemetryChanged.connect(terminal.remote_connections.update_frame)
    bridge.openTerminal.connect(terminal.open_session)
    bridge.openApplicationTerminal.connect(terminal.open_application)
    bridge.locked.connect(terminal.close_session)
    blanker = PhysicalScreenOff(app) if os.environ.get("ANIMA_SHARED_CONSOLE") == "1" else ScreenOff(app)
    bridge.blankScreen.connect(blanker.blank)
    bridge.openBrowser.connect(browser.open_signin)
    bridge.closeBrowser.connect(browser.clear)
    view.load(QUrl.fromLocalFile(str(ROOT / 'index.html')))
    window.showFullScreen()
    app.aboutToQuit.connect(bridge.lock)
    return app.exec()


if __name__ == '__main__':
    sys.exit(main())
