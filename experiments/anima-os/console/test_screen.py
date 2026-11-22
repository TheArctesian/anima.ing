"""Real Qt input/cover lifecycle with a fake xset; never changes the host display."""
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
from PySide6.QtCore import QEvent, QPointF, Qt, QTimer
from PySide6.QtGui import QEventPoint, QInputDevice, QKeyEvent, QTouchEvent
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication, QLineEdit, QPushButton, QVBoxLayout, QWidget
from screen import ScreenOff

XSET = r'''
import json, os, sys, time
from pathlib import Path
args = sys.argv[1:]
log = Path(os.environ['FAKE_XSET_LOG'])
def record(phase):
    with log.open('a') as stream:
        stream.write(json.dumps([phase, args]) + '\n')
record('start')
if args[-1] == 'off':
    if os.environ.get('FAKE_XSET_MODE') == 'slow':
        time.sleep(.6)
    if os.environ.get('FAKE_XSET_MODE') == 'fail':
        sys.exit(1)
record('done')
'''


class TouchTarget(QWidget):
    def __init__(self):
        super().__init__()
        self.received = []
        self.setAttribute(Qt.WidgetAttribute.WA_AcceptTouchEvents)

    def event(self, event):
        if event.type() in (QEvent.Type.TouchBegin, QEvent.Type.TouchUpdate, QEvent.Type.TouchEnd):
            self.received.append(event.type())
            event.accept()
            return True
        return super().event(event)


class ScreenLifecycle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.folder = Path(self.temp.name)
        self.log = self.folder / 'xset.jsonl'
        fake = self.folder / 'xset'
        fake.write_text('#!' + sys.executable + '\n' + XSET)
        fake.chmod(0o700)
        self.environment = patch.dict(os.environ, {'PATH': str(self.folder), 'FAKE_XSET_LOG': str(self.log), 'FAKE_XSET_MODE': ''})
        self.environment.start()
        self.window = QWidget()
        layout = QVBoxLayout(self.window)
        self.entry = QLineEdit()
        self.button = QPushButton('Example action')
        layout.addWidget(self.entry)
        layout.addWidget(self.button)
        self.window.show()
        self.controller = ScreenOff()
        QTest.qWait(20)

    def tearDown(self):
        self.controller.close()
        self.controller.deleteLater()
        self.window.close()
        self.window.deleteLater()
        self.app.processEvents()
        self.environment.stop()
        self.temp.cleanup()

    def records(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def covers(self):
        return [window for window in self.app.topLevelWidgets() if window.isVisible() and window.windowTitle() == 'Anima — screen off']

    def until(self, predicate):
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            if predicate():
                return
            QTest.qWait(10)
        self.fail('Timed out waiting for display state')

    def test_black_cover_keyboard_wake_swallows_repeat_and_release(self):
        self.controller.blank()
        self.until(lambda: len(self.covers()) == len(self.app.screens()))
        for cover in self.covers():
            self.assertEqual(cover.geometry(), cover.screen().geometry())
            self.assertEqual(cover.grab().toImage().pixelColor(10, 10).name(), '#000000')
        cover = self.covers()[0]
        QTest.keyPress(cover, Qt.Key.Key_A)
        self.assertTrue(self.covers(), "Retain the input grab until the waking key is released")
        repeat = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_A, Qt.KeyboardModifier.NoModifier, 'a', True)
        self.app.sendEvent(cover, repeat)
        QTest.keyRelease(cover, Qt.Key.Key_A)
        self.assertFalse(self.covers())
        self.assertEqual(self.entry.text(), '')
        QTest.keyClick(self.entry, Qt.Key.Key_B)
        self.assertEqual(self.entry.text(), 'b')
        QTest.qWait(350)
        self.assertFalse(any(args[-1] == 'off' for _, args in self.records()), 'Wake cancels delayed DPMS off')

    def test_mouse_wake_does_not_click_underlying_action(self):
        clicks = []
        self.button.clicked.connect(lambda: clicks.append(True))
        self.controller.blank()
        QTest.mouseClick(self.covers()[0], Qt.MouseButton.LeftButton)
        self.assertFalse(self.covers())
        self.assertEqual(clicks, [])
        QTest.mouseClick(self.button, Qt.MouseButton.LeftButton)
        self.assertEqual(clicks, [True])

    def test_touch_wake_consumes_the_whole_contact(self):
        target = TouchTarget()
        device = QTest.createTouchDevice(QInputDevice.DeviceType.TouchScreen)
        target.show()
        self.controller.blank()
        cover = self.covers()[0]
        for kind, state in ((QEvent.Type.TouchBegin, QEventPoint.State.Pressed), (QEvent.Type.TouchUpdate, QEventPoint.State.Updated), (QEvent.Type.TouchEnd, QEventPoint.State.Released)):
            point = QEventPoint(0, state, QPointF(5, 5), QPointF(5, 5))
            self.app.sendEvent(cover, QTouchEvent(kind, device, Qt.KeyboardModifier.NoModifier, [point]))
        self.assertFalse(self.covers())
        self.assertEqual(target.received, [])
        self.app.sendEvent(target, QTouchEvent(QEvent.Type.TouchBegin, device, Qt.KeyboardModifier.NoModifier, []))
        self.assertEqual(target.received, [QEvent.Type.TouchBegin])
        target.close()
        target.deleteLater()

    def test_wake_cancels_inflight_off_before_restoring_power(self):
        os.environ['FAKE_XSET_MODE'] = 'slow'
        ticks = []
        timer = QTimer()
        timer.timeout.connect(lambda: ticks.append(True))
        timer.start(20)
        self.controller.blank()
        self.until(lambda: any(phase == 'start' and args[-1] == 'off' for phase, args in self.records()))
        QTest.keyClick(self.covers()[0], Qt.Key.Key_A)
        self.until(lambda: any(phase == 'done' and 'on' in args for phase, args in self.records()))
        QTest.qWait(700)
        self.assertFalse(any(phase == 'done' and args[-1] == 'off' for phase, args in self.records()), 'A killed off command must never finish after wake')
        self.assertGreater(len(ticks), 10, 'Display commands must not block the event loop')
        timer.stop()
        self.assertFalse(self.covers())

    def test_failed_or_missing_dpms_still_blacks_out_and_wakes(self):
        for mode in ('fail', 'missing'):
            with self.subTest(mode=mode):
                os.environ['FAKE_XSET_MODE'] = mode
                if mode == 'missing':
                    (self.folder / 'xset').unlink()
                self.controller.blank()
                QTest.qWait(400)
                self.assertTrue(self.covers())
                QTest.keyClick(self.covers()[0], Qt.Key.Key_A)
                self.assertFalse(self.covers())
                QTest.qWait(30)

    def test_close_cancels_off_and_leaves_no_cover_or_delayed_command(self):
        os.environ['FAKE_XSET_MODE'] = 'slow'
        self.controller.blank()
        self.until(lambda: any(phase == 'start' and args[-1] == 'off' for phase, args in self.records()))
        self.controller.close()
        self.assertFalse(self.covers())
        QTest.qWait(700)
        self.assertFalse(any(phase == 'done' and args[-1] == 'off' for phase, args in self.records()))
        self.assertTrue(any(phase == 'done' and 'on' in args for phase, args in self.records()))


if __name__ == '__main__':
    unittest.main()
