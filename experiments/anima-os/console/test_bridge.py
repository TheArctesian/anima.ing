"""Qt bridge lifecycle against a synthetic asynchronous child process, without sudo."""
import json
from pathlib import Path
import sys
import time
import unittest
from PySide6.QtCore import QCoreApplication, QProcess
from PySide6.QtTest import QTest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'telemetry'))
from app import Bridge

CHILD = r'''
import json, sys, threading, time
lock = threading.Lock()
def answer(request):
    if request['op'] == 'update-download':
        time.sleep(.5)
    with lock:
        expired = request['op'] == 'sign-in-poll'
        print(json.dumps(dict(id=request['id'], ok=not expired, value={}, error='Sign-in expired.' if expired else '')), flush=True)
for line in sys.stdin:
    threading.Thread(target=answer, args=(json.loads(line),)).start()
'''


class BridgeLifecycle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def setUp(self):
        self.bridge = Bridge()
        self.bridge.telemetry_timer.stop()
        self.answers = []
        self.closed = []
        self.bridge.answer.connect(lambda text: self.answers.append(json.loads(text)))
        self.bridge.closeBrowser.connect(lambda: self.closed.append(True))
        self.process = QProcess()
        self.process.readyReadStandardOutput.connect(lambda: self.bridge.read(self.process))
        self.process.start(sys.executable, ['-u', '-c', CHILD])
        self.assertTrue(self.process.waitForStarted())
        self.bridge.process = self.process
        self.bridge.unlocked = True

    def tearDown(self):
        self.process.closeWriteChannel()
        if not self.process.waitForFinished(2000):
            self.process.kill()
            self.process.waitForFinished()
        self.bridge.deleteLater()

    def send(self, identity, op):
        self.bridge.request(json.dumps(dict(id=identity, op=op, args={})))

    def wait_for(self, identity):
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            for answer in self.answers:
                if answer['id'] == identity:
                    return answer
            QTest.qWait(10)
        self.fail('No reply for ' + identity)

    def test_reads_complete_during_mutation_and_cancel_closes_provider(self):
        self.send('download', 'update-download')
        self.send('observe', 'update-status')
        self.send('conflict', 'restart')
        self.assertFalse(self.wait_for('conflict')['ok'])
        self.assertTrue(self.wait_for('observe')['ok'])
        self.assertIn('download', self.bridge.pending)
        self.assertTrue(self.wait_for('download')['ok'])
        self.assertEqual(self.bridge.pending, {})
        self.send('cancel', 'cancel-sign-in')
        self.assertTrue(self.wait_for('cancel')['ok'])
        self.assertEqual(len(self.closed), 1)
        self.bridge.lock()
        self.assertFalse(self.bridge.unlocked)
        self.send('locked', 'status')
        self.assertFalse(self.wait_for('locked')['ok'])

    def test_expired_signin_closes_the_qr_and_browser_window(self):
        self.send('expired', 'sign-in-poll')
        self.assertFalse(self.wait_for('expired')['ok'])
        self.assertEqual(len(self.closed), 1)


if __name__ == '__main__':
    unittest.main()
