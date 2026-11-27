"""Battery projection for the live installer."""
import json
from pathlib import Path


def text(path):
    try:
        return path.read_text().strip()
    except OSError:
        return ''


def number(path):
    try:
        return float(text(path))
    except ValueError:
        return None


def batteries():
    result = []
    for device in sorted(Path('/sys/class/power_supply').glob('*')):
        if text(device / 'type') != 'Battery' or text(device / 'scope') == 'Device' or text(device / 'present') == '0':
            continue
        percent = number(device / 'capacity')
        result.append({'name': device.name, 'percent': percent if percent is not None and 0 <= percent <= 100 else None,
                       'state': text(device / 'status') or 'Unknown'})
    return result


if __name__ == '__main__':
    print(json.dumps(batteries()))
