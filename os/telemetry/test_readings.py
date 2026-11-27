"""Live installer battery readings through hotplug and missing charge."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import readings


class Telemetry(unittest.TestCase):
    def test_battery_hotplug_and_unknown_capacity(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            def write(name, value):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(value)
            def path(name):
                return root / str(name).lstrip('/')
            write('sys/class/power_supply/BAT0/type', 'Battery')
            write('sys/class/power_supply/BAT0/capacity', '73')
            write('sys/class/power_supply/BAT0/status', 'Charging')
            write('sys/class/power_supply/mouse/type', 'Battery')
            write('sys/class/power_supply/mouse/scope', 'Device')
            with patch.object(readings, 'Path', path):
                self.assertEqual(readings.batteries(), [{'name': 'BAT0', 'percent': 73, 'state': 'Charging'}])
                (root/'sys/class/power_supply/BAT0/capacity').unlink()
                self.assertIsNone(readings.batteries()[0]['percent'])
                write('sys/class/power_supply/BAT0/present', '0')
                self.assertEqual(readings.batteries(), [])


if __name__ == '__main__':
    unittest.main()
