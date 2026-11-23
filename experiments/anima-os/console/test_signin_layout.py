"""Exercise the actual sign-in window without contacting an account provider."""
import argparse
import json
import importlib.util
from pathlib import Path
import sys
import subprocess
import os
import tempfile
from unittest.mock import patch
from screen import ScreenOff
from PySide6.QtCore import Qt
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication, QPushButton
from PySide6.QtWebEngineWidgets import QWebEngineView

URL = 'https://auth.example/device?user_code=ABCD-EFGH'


def check_code(browser, output):
    browser.qr.set_url(URL)
    browser.code.setText('ABCD-EFGH')
    browser.link.setText(URL)
    browser.confirmation.setText('Confirm code: ABCD-EFGH')
    browser.show()
    QTest.mouseClick(browser.alternative, Qt.MouseButton.LeftButton)
    for width, height in ((1280, 800), (800, 1280), (640, 480)):
        browser.resize(width, height)
        QTest.qWait(150)
        browser.grab().save(str(output / f'qr-{width}x{height}.png'))
        qr = output / f'code-{width}x{height}.png'
        browser.qr.grab().save(str(qr))
        decoded = subprocess.check_output(['zbarimg', '--quiet', '--raw', str(qr)], text=True).strip()
        assert decoded == URL, 'Phone-readable QR must encode the complete verification link'
        assert browser.pages.currentWidget().horizontalScrollBar().maximum() == 0
    for label, expected in (('Copy code', 'ABCD-EFGH'), ('Copy link', URL)):
        button = next(widget for widget in browser.findChildren(QPushButton) if widget.text() == label)
        QTest.mouseClick(button, Qt.MouseButton.LeftButton)
        assert QApplication.clipboard().text() == expected
    QTest.mouseClick(browser.alternative, Qt.MouseButton.LeftButton)
    assert browser.pages.currentWidget() is browser.web, 'Local browser sign-in remains available'
    browser.clear()
    assert not browser.isVisible() and not browser.code.text() and not browser.link.text()
    assert not browser.qr.matrix, 'Completion/cancellation must remove the previous QR'


def check_screen(output):
    local = next(widget for widget in QApplication.allWidgets()
                 if isinstance(widget, QWebEngineView) and widget.page().webChannel() is not None)
    bridge = local.page().webChannel().registeredObjects()['console']
    blanker = QApplication.instance().findChild(ScreenOff)
    assert blanker is not None, 'Exercise the controller wired by the real application'
    with tempfile.TemporaryDirectory() as folder:
        command = Path(folder) / 'xset'
        command.write_text('#!' + sys.executable + '\nimport sys\nsys.exit(0)\n')
        command.chmod(0o700)
        with patch.dict(os.environ, {'PATH': folder}):
            try:
                bridge.screenOff()
                QTest.qWait(350)
                covers = [window for window in QApplication.topLevelWidgets()
                          if window.isVisible() and window.windowTitle() == 'Anima — screen off']
                assert len(covers) == len(QApplication.screens()), 'Every display must be covered'
                for cover in covers:
                    assert cover.grab().toImage().pixelColor(10, 10).name() == '#000000'
                covers[0].grab().save(str(output / 'screen-off.png'))
                QTest.keyPress(covers[0], Qt.Key.Key_A)
                QTest.qWait(30)
                QTest.keyRelease(covers[0], Qt.Key.Key_A)
                QTest.qWait(150)
                assert not any(window.isVisible() and window.windowTitle() == 'Anima — screen off' for window in QApplication.topLevelWidgets()), 'Wake must dismiss the covers'
                state = []
                local.page().runJavaScript("JSON.stringify({locked:!!document.querySelector('#unlock-form'),password:document.querySelector('#unlock-form input')?.value})", state.append)
                for _ in range(100):
                    if state:
                        break
                    QTest.qWait(10)
                assert state and json.loads(state[0]) == {'locked': True, 'password': ''}, f'Wake must leave the real web console locked without typing the wake key: {state!r}'
                local.window().grab().save(str(output / 'screen-wake-locked.png'))
                print('screen-off: real bridge, black cover, keyboard wake and locked WebEngine page passed', flush=True)
            finally:
                blanker.close()


def check_windows(output):
    back = next(widget for widget in QApplication.allWidgets()
                if isinstance(widget, QPushButton) and widget.text() == '← Back to Anima')
    browser = back.window()
    web = browser.findChild(QWebEngineView)
    assert web.page().webChannel() is None, 'Remote sign-in must not receive the native bridge'
    assert web.page().profile().isOffTheRecord(), 'Sign-in must not keep a persistent browser profile'
    browser.show()
    for width, height in ((1280, 800), (1920, 1200), (800, 1280), (640, 480)):
        browser.resize(width, height)
        QTest.qWait(150)
        browser.grab().save(str(output / f'signin-{width}x{height}.png'))
        print(f'{width}x{height}: page {web.geometry().getRect()}', flush=True)
        assert web.y() <= 96, 'Sign-in header consumes too much vertical space'
        assert web.height() >= height - 96, 'Sign-in form does not fill remaining screen space'
        assert web.width() >= width - 32, 'Sign-in form is unnecessarily narrow'
        assert browser.rect().contains(back.geometry()) or browser.rect().contains(back.mapTo(browser, back.rect().center()))
    QTest.mouseClick(back, Qt.MouseButton.LeftButton)
    assert not browser.isVisible(), 'Back must return to the local console'
    check_code(browser, output)
    check_screen(output)
    return 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, default=Path(__file__).with_name('app.py'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location('console_under_test', args.source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    QApplication.exec = staticmethod(lambda: check_windows(args.output))
    return module.main()


if __name__ == '__main__':
    sys.exit(main())
