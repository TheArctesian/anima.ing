"""An authenticated console owns one local administration PTY, never a coding run."""
import os
import pwd
from pathlib import Path
from PySide6.QtCore import QProcess, QProcessEnvironment, Qt, QTimer
from PySide6.QtGui import QFontDatabase
from PySide6.QtWidgets import QHBoxLayout, QLabel, QPushButton, QVBoxLayout, QWidget
from remote_status import RemoteConnections

POWERLINE_FONT = 'JetBrainsMono Nerd Font'
POWERLINE_CONFIG = '/etc/anima/starship-powerline.toml'


def host_environment(environment, font):
    shell = pwd.getpwuid(os.getuid()).pw_shell
    if shell:
        environment.insert('SHELL', shell)
    else:
        environment.remove('SHELL')
    if environment.value('STARSHIP_CONFIG'):
        return
    home_config = Path.home() / '.config'
    user_config = Path(environment.value('XDG_CONFIG_HOME') or home_config)
    paths = (folder / 'starship.toml' for folder in (user_config, home_config) if folder.is_absolute())
    preferred = next((path for path in paths if path.is_file()), None)
    if preferred:
        environment.insert('STARSHIP_CONFIG', str(preferred))
    elif font == POWERLINE_FONT and not environment.value('NO_COLOR'):
        environment.insert('STARSHIP_CONFIG', POWERLINE_CONFIG)


class AdminTerminal(QWidget):
    def __init__(self):
        super().__init__()
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint)
        self.setWindowTitle('Anima · node administration')
        self.setStyleSheet('QWidget{background:#1B1D21;color:#E6E7E9;font:14px "IBM Plex Sans";}QPushButton{padding:8px 14px;border:1px solid #353941;border-radius:6px;}QPushButton:hover,QPushButton:focus{border-color:#88C0D0;}')
        layout = QVBoxLayout(self)
        header = QHBoxLayout()
        self.label = QLabel('Node administration · admin')
        self.label.setTextFormat(Qt.TextFormat.PlainText)
        self.label.setWordWrap(True)
        header.addWidget(self.label, 1)
        self.remote_connections = RemoteConnections()
        header.addWidget(self.remote_connections)
        close = QPushButton('×')
        close.setToolTip('Close terminal and end this shell')
        close.setAccessibleName('Close administration terminal')
        close.clicked.connect(self.close_session)
        header.addWidget(close)
        layout.addLayout(header)
        self.surface = QWidget(self)
        self.surface.setAttribute(Qt.WidgetAttribute.WA_NativeWindow)
        self.surface.setAccessibleName('Administration terminal')
        layout.addWidget(self.surface, 1)
        self.process = None
        self.closing = False
        self.next_session = None

    def open_session(self):
        self.open('Node administration · admin')

    def open_application(self, container, name, user):
        self.open(f'Application · {name} · {user}', [
            '/run/wrappers/bin/sudo', '-k', '--',
            '/run/current-system/sw/bin/anima-application-terminal', 'open', container,
        ])

    def open(self, target, command=None):
        if self.closing:
            self.next_session = (target, command)
            return
        if self.process is not None:
            self.showFullScreen()
            self.raise_()
            self.activateWindow()
            return
        self.label.setText(target + ' · closes when locked')
        self.showFullScreen()
        self.raise_()
        self.activateWindow()
        process = QProcess(self)
        self.process = process
        environment = QProcessEnvironment.systemEnvironment()
        # Shells should not inherit the workspace runner's owner ingress or the
        # graphical application's private runtime configuration.
        for key in environment.keys():
            if key.startswith(('ANIMA_', 'QTWEBENGINE', 'PYTHON')):
                environment.remove(key)
        font = POWERLINE_FONT if POWERLINE_FONT in QFontDatabase.families() else 'Monospace'
        terminal_args = ['-ls'] if command is None else ['-e', *command]
        if command is None:
            host_environment(environment, font)
        process.setProcessEnvironment(environment)
        process.setWorkingDirectory(os.path.expanduser('~'))
        process.setStandardOutputFile(os.devnull)
        process.setStandardErrorFile(os.devnull)
        process.setUnixProcessParameters(QProcess.UnixProcessFlag.CloseFileDescriptors)
        process.finished.connect(lambda *_: self.ended(process))
        process.errorOccurred.connect(lambda *_: self.failed(process))
        process.start('/run/current-system/sw/bin/xterm', [
            '-into', str(int(self.surface.winId())), '-fa', font, '-fs', '12', '-tn', 'xterm-256color',
            '-bg', '#1B1D21', '-fg', '#E6E7E9', '-cr', '#88C0D0', '-sl', '10000',
            '-xrm', 'XTerm*allowWindowOps: false', '-xrm', 'XTerm*allowTitleOps: false',
            '-xrm', 'XTerm*allowFontOps: false', '-xrm', 'XTerm*allowTcapOps: false',
            '-xrm', 'XTerm*disallowedWindowOps: SetSelection,GetSelection,SetXprop',
            *terminal_args,
        ])

    def ended(self, process):
        process.deleteLater()
        if self.process is not process:
            return
        self.process = None
        self.closing = False
        self.hide()
        requested, self.next_session = self.next_session, None
        if requested is not None:
            self.open(*requested)

    def failed(self, process):
        if self.process is not process:
            return
        if self.closing:
            if process.state() == QProcess.ProcessState.NotRunning:
                self.ended(process)
        else:
            self.label.setText('Terminal unavailable. Close this window and retry.')
            if process.state() == QProcess.ProcessState.NotRunning:
                self.process = None
                process.deleteLater()

    def close_session(self):
        self.hide()
        self.next_session = None
        process = self.process
        if process is not None and not self.closing:
            self.closing = True
            # Closing xterm closes its PTY. It never attaches a persistent tmux
            # session; services deliberately started by the admin remain services.
            timer = QTimer(process)
            timer.setSingleShot(True)
            timer.timeout.connect(process.kill)
            timer.start(750)
            process.terminate()

    def closeEvent(self, event):
        self.close_session()
        event.accept()
