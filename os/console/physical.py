"""Physical attachment to the persistent console; blanking stays on this display."""
import json
import os
from pathlib import Path
import sys
from PySide6.QtCore import QTimer
from PySide6.QtNetwork import QLocalServer
from PySide6.QtWidgets import QApplication
from screen import ScreenOff
from physical_viewer import PhysicalViewer

SOCKET = '/run/anima-console/physical.sock'
ENVIRONMENT = Path('/run/anima-console/physical.json')


def main():
    app = QApplication(sys.argv)
    app.setQuitOnLastWindowClosed(False)
    blanker = ScreenOff(app)
    server = QLocalServer(app)
    server.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)
    # Display manager owns exactly one physical attachment for the admin session.
    QLocalServer.removeServer(SOCKET)
    if not server.listen(SOCKET):
        return 1
    os.umask(0o077)
    temporary = ENVIRONMENT.with_suffix('.pending')
    attachment = {key: os.environ.get(key, '') for key in ('DISPLAY', 'XAUTHORITY')}
    attachment.update(pid=os.getpid(), bootId=Path('/proc/sys/kernel/random/boot_id').read_text().strip())
    temporary.write_text(json.dumps(attachment))
    temporary.replace(ENVIRONMENT)

    def receive():
        while server.hasPendingConnections():
            peer = server.nextPendingConnection()
            peer.readyRead.connect(lambda peer=peer: consume(peer))
            peer.disconnected.connect(peer.deleteLater)
            QTimer.singleShot(1000, peer.disconnectFromServer)

    def consume(peer):
        data = (peer.property('request') or b'') + bytes(peer.readAll())
        if len(data) > 16:
            peer.disconnectFromServer()
        elif b'\n' in data:
            if data == b'blank\n':
                blanker.blank()
            peer.disconnectFromServer()
        else:
            peer.setProperty('request', data)

    server.newConnection.connect(receive)
    viewer = PhysicalViewer(app)

    def close():
        viewer.close()
        server.close()
        QLocalServer.removeServer(SOCKET)
        ENVIRONMENT.unlink(missing_ok=True)

    app.aboutToQuit.connect(close)
    return app.exec()


if __name__ == '__main__':
    sys.exit(main())
