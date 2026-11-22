"""Local display blanking; the console locks separately and workloads keep running."""
import os
from PySide6.QtCore import QEvent, QObject, QProcess, Qt, QTimer
from PySide6.QtGui import QColor, QCursor, QPalette
from PySide6.QtWidgets import QApplication, QWidget

OFF = ['+dpms', 'dpms', 'force', 'off']
ON = ['dpms', 'force', 'on', '-dpms']
KEYS = (QEvent.Type.KeyPress, QEvent.Type.KeyRelease)
BUTTONS = (QEvent.Type.MouseButtonPress, QEvent.Type.MouseButtonRelease, QEvent.Type.MouseButtonDblClick)
TOUCH = (QEvent.Type.TouchBegin, QEvent.Type.TouchUpdate, QEvent.Type.TouchEnd, QEvent.Type.TouchCancel)
WAKE = (*KEYS, *BUTTONS, *TOUCH, QEvent.Type.MouseMove, QEvent.Type.Wheel, QEvent.Type.NativeGesture)


class ScreenOff(QObject):
    def __init__(self, parent=None):
        super().__init__(parent)
        self._app = QApplication.instance()
        self._covers = {}
        self._active = False
        self._closed = False
        self._keys = set()
        self._buttons = Qt.MouseButton.NoButton
        self._touch = False
        self._running = None
        self._pending = None
        self._needs_restore = False
        self._pointer = QCursor.pos()
        self._process = QProcess(self)
        self._process.setStandardOutputFile(os.devnull)
        self._process.setStandardErrorFile(os.devnull)
        self._process.started.connect(self._started)
        self._process.finished.connect(self._finished)
        self._process.errorOccurred.connect(self._failed)
        self._deadline = QTimer(self)
        self._deadline.setSingleShot(True)
        self._deadline.timeout.connect(self._process.kill)
        self._off_timer = QTimer(self)
        self._off_timer.setSingleShot(True)
        self._off_timer.timeout.connect(self._turn_off)
        self._app.installEventFilter(self)
        self._app.screenAdded.connect(self._added)
        self._app.screenRemoved.connect(self._removed)
        self._app.aboutToQuit.connect(self.close)

    def blank(self):
        if self._closed or self._active:
            return
        self._active = True
        self._pointer = QCursor.pos()
        for screen in self._app.screens():
            self._added(screen)
        screen = self._app.screenAt(self._pointer) or self._app.primaryScreen()
        cover = self._covers.get(screen)
        if cover:
            cover.activateWindow()
            cover.grabKeyboard()
            cover.grabMouse(Qt.CursorShape.BlankCursor)
        self._off_timer.start(250)

    def _added(self, screen):
        if not self._active or screen in self._covers:
            return
        flags = Qt.WindowType.Tool | Qt.WindowType.FramelessWindowHint | Qt.WindowType.WindowStaysOnTopHint | Qt.WindowType.BypassWindowManagerHint
        cover = QWidget(None, flags)
        cover.setWindowTitle('Anima — screen off')
        cover.setScreen(screen)
        palette = cover.palette()
        palette.setColor(QPalette.ColorRole.Window, QColor('black'))
        cover.setPalette(palette)
        cover.setAutoFillBackground(True)
        cover.setAttribute(Qt.WidgetAttribute.WA_AcceptTouchEvents)
        cover.setCursor(Qt.CursorShape.BlankCursor)
        cover.setMouseTracking(True)
        cover.setGeometry(screen.geometry())
        screen.geometryChanged.connect(cover.setGeometry)
        self._covers[screen] = cover
        cover.showFullScreen()
        cover.raise_()
        self._off_timer.start(250)

    def _removed(self, screen):
        cover = self._covers.pop(screen, None)
        if cover:
            cover.close()
            cover.deleteLater()

    def _turn_off(self):
        if self._active:
            self._request('off')

    def _request(self, target):
        self._pending = target
        if self._running is None:
            self._drain()
        elif target == 'on' and self._running == 'off':
            self._process.kill()

    def _drain(self):
        if self._closed or self._pending is None or self._running is not None:
            return
        self._running, self._pending = self._pending, None
        if self._running == 'off':
            self._needs_restore = True
        self._process.start('xset', OFF if self._running == 'off' else ON)
        self._deadline.start(1500)

    def _started(self):
        if self._running == 'off' and not self._active:
            self._process.kill()

    def _finished(self, code=1, status=QProcess.ExitStatus.CrashExit):
        if self._running == 'on' and code == 0 and status == QProcess.ExitStatus.NormalExit:
            self._needs_restore = False
        self._deadline.stop()
        self._running = None
        self._drain()

    def _failed(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self._finished()

    def _wake(self):
        self._active = False
        self._off_timer.stop()
        for screen in tuple(self._covers):
            self._removed(screen)
        if self._needs_restore or self._running is not None:
            self._request('on')

    def _consume_sequence(self, event):
        kind = event.type()
        if kind in KEYS and event.key() in self._keys:
            if kind == QEvent.Type.KeyRelease and not event.isAutoRepeat():
                self._keys.discard(event.key())
            return True
        if kind in BUTTONS and event.button() & self._buttons:
            if kind == QEvent.Type.MouseButtonRelease:
                self._buttons &= ~event.button()
            return True
        if kind in TOUCH and self._touch:
            if kind in (QEvent.Type.TouchEnd, QEvent.Type.TouchCancel):
                self._touch = False
            return True
        return False

    def eventFilter(self, watched, event):
        if self._closed:
            return False
        if self._consume_sequence(event):
            event.accept()
            return True
        if not self._active:
            return False
        kind = event.type()
        if kind == QEvent.Type.ShortcutOverride:
            event.accept()
            return True
        if kind not in WAKE:
            return False
        if kind == QEvent.Type.MouseMove and event.globalPosition().toPoint() == self._pointer:
            return True
        if kind == QEvent.Type.KeyPress:
            self._keys.add(event.key())
        if kind in BUTTONS or kind == QEvent.Type.MouseMove:
            self._buttons = event.buttons()
        if kind in (QEvent.Type.TouchBegin, QEvent.Type.TouchUpdate):
            self._touch = True
        self._wake()
        event.accept()
        return True

    def close(self):
        if self._closed:
            return
        self._closed = True
        self._active = False
        self._off_timer.stop()
        self._deadline.stop()
        self._pending = None
        self._app.removeEventFilter(self)
        for screen in tuple(self._covers):
            self._removed(screen)
        # Only teardown waits: cancel off before restoring the kiosk's no-idle-DPMS policy.
        if self._process.state() != QProcess.ProcessState.NotRunning:
            self._process.kill()
            self._process.waitForFinished(250)
        if self._needs_restore:
            self._process.start('xset', ON)
            if not self._process.waitForFinished(500):
                self._process.kill()
                self._process.waitForFinished(250)
