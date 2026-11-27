"""A stopped service or a replaced snapshot cannot leave current-looking metrics."""
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'console'))
from telemetry_snapshot import read_frame


class Snapshot(unittest.TestCase):
    def test_unavailable_publication_and_recovery(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'console.json'
            frame = {'schema': 1, 'cpu': 42, 'batteries': [], 'remoteConnections': {'rdp': 0, 'vnc': 0}}
            self.assertEqual(read_frame(path), 'null')
            path.write_text(json.dumps(frame))
            self.assertEqual(json.loads(read_frame(path)), frame)
            previous = time.time() - 20
            os.utime(path, (previous, previous))
            self.assertEqual(read_frame(path), 'null')
            pending = path.with_suffix('.new')
            pending.write_text(json.dumps(frame))
            pending.replace(path)
            self.assertEqual(json.loads(read_frame(path)), frame)
            for invalid in (' ' * 17_000, '{"schema":1,"batteries":42}', '{"schema":1,"cpu":NaN}'):
                path.write_text(invalid)
                self.assertEqual(read_frame(path), 'null')
            path.unlink()
            pending.write_text(json.dumps(frame))
            path.symlink_to(pending.name)
            self.assertEqual(read_frame(path), 'null')
