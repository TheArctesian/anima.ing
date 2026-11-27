"""Read the observer's boot-local projection without sampling hardware."""
import json
import os
from pathlib import Path
import stat
import time

SOURCE = Path('/run/anima-observation/console.json')
FRAME_LIMIT = 16_384
MAX_AGE = 15


def read_frame(path=SOURCE):
    try:
        flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
        with os.fdopen(os.open(path, flags), 'rb') as source:
            info = os.fstat(source.fileno())
            age = time.time() - info.st_mtime
            if not stat.S_ISREG(info.st_mode) or not 0 <= age <= MAX_AGE or info.st_size > FRAME_LIMIT:
                return 'null'
            frame = source.read(FRAME_LIMIT + 1)
        if len(frame) > FRAME_LIMIT:
            return 'null'
        value = json.loads(frame)
        if isinstance(value, dict) and value.get('schema') == 1:
            batteries = value.get('batteries')
            if batteries is None or (isinstance(batteries, list) and all(isinstance(row, dict) for row in batteries)):
                return json.dumps(value, allow_nan=False)
    except (OSError, ValueError, RecursionError):
        pass
    return 'null'
