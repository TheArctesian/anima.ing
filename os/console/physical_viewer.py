"""The primary physical monitor owns shared-display geometry; remote clients observe it."""
import os
from PySide6.QtCore import QObject, QProcess, QTimer


class PhysicalViewer(QObject):
    def __init__(self, app):
        super().__init__(app)
        self.app = app
        self.closing = False
        self.current = None
        self.generation = 0
        self.process = QProcess(self)
        self.process.setStandardOutputFile(os.devnull)
        self.process.setStandardErrorFile(os.devnull)
        self.process.finished.connect(self.ended)
        self.process.errorOccurred.connect(self.ended)
        self.retry = QTimer(self)
        self.retry.setSingleShot(True)
        self.retry.timeout.connect(self.refresh)
        self.changed = QTimer(self)
        self.changed.setSingleShot(True)
        self.changed.timeout.connect(self.refresh)
        for screen in app.screens():
            self.watch(screen)
        app.screenAdded.connect(self.added)
        app.screenRemoved.connect(self.schedule)
        app.primaryScreenChanged.connect(self.schedule)
        self.refresh()

    def watch(self, screen):
        screen.geometryChanged.connect(self.schedule)

    def added(self, screen):
        self.watch(screen)
        self.schedule()

    def schedule(self, *_):
        if not self.closing:
            self.changed.start(250)

    def desired(self):
        screen = self.app.primaryScreen()
        if screen is None:
            return None
        geometry = screen.geometry()
        if geometry.width() <= 0 or geometry.height() <= 0:
            return None
        return (screen.name(), max(320, geometry.width()), max(240, geometry.height()))

    def refresh(self):
        if self.closing:
            return
        desired = self.desired()
        if self.process.state() != QProcess.ProcessState.NotRunning:
            if desired != self.current:
                self.process.terminate()
                generation = self.generation
                timer = QTimer(self.process)
                timer.setSingleShot(True)
                timer.timeout.connect(lambda: self.force_stop(generation))
                timer.timeout.connect(timer.deleteLater)
                timer.start(1000)
            return
        if desired is None:
            self.current = None
            return
        self.current = desired
        self.generation += 1
        _, width, height = desired
        self.process.start('vncviewer', [
            '-FullScreen', '-Shared', '-RemoteResize=1', f'-DesktopSize={width}x{height}',
            '-ShortcutModifiers=', '-SecurityTypes=None', '-AcceptClipboard=0', '-SendClipboard=0',
            '-AlertOnFatalError=0', '-ReconnectOnError=0', '/run/anima-console/framebuffer.sock',
        ])

    def force_stop(self, generation):
        if self.generation == generation and self.process.state() != QProcess.ProcessState.NotRunning:
            self.process.kill()

    def ended(self, *_):
        if not self.closing:
            self.retry.start(1000)

    def close(self):
        self.closing = True
        self.retry.stop()
        self.changed.stop()
        self.process.kill()
