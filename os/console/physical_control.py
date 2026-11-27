"""The virtual console requests physical blanking without covering its framebuffer."""
import json
from pathlib import Path
from PySide6.QtCore import QObject, QTimer
from PySide6.QtNetwork import QLocalSocket


class PhysicalScreenOff(QObject):
    def blank(self):
        peer = QLocalSocket(self)
        peer.connected.connect(lambda: peer.write(b'blank\n'))
        peer.disconnected.connect(peer.deleteLater)
        peer.errorOccurred.connect(lambda *_: peer.deleteLater())
        QTimer.singleShot(1500, peer.disconnectFromServer)
        peer.connectToServer('/run/anima-console/physical.sock')


def environment():
    try:
        data = json.loads(Path('/run/anima-console/physical.json').read_text())
        return {key: value for key, value in data.items()
                if key in ('DISPLAY', 'XAUTHORITY') and isinstance(value, str)}
    except (OSError, ValueError, TypeError):
        return {}
