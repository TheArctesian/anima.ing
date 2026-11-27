"""Show graphical transport observations in native console headers."""

import json

from PySide6.QtCore import Qt, Slot
from PySide6.QtWidgets import QLabel, QSizePolicy


class RemoteConnections(QLabel):
    def __init__(self):
        super().__init__()
        self.setTextFormat(Qt.TextFormat.PlainText)
        self.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Preferred)
        self.setAccessibleName('Remote desktop connections')
        self.setToolTip('Established RDP and VNC transports to this shared console. Counts include clients signing in.')
        self.update_frame('null')

    @Slot(str)
    def update_frame(self, frame):
        sample = json.loads(frame)
        observation = sample.get('remoteConnections') if isinstance(sample, dict) else None
        protocols = ('RDP', 'VNC')
        counts = [observation.get(name.lower()) for name in protocols] if isinstance(observation, dict) else []
        available = len(counts) == len(protocols) and all(type(count) is int and 0 <= count < 2**32 for count in counts)
        text = (' · '.join(f'{count} {name}' for name, count in zip(protocols, counts) if count)
                if available else 'Remote status unavailable')
        if text == self.text():
            return
        self.setText(text)
        self.setAccessibleDescription(text)
        self.setStyleSheet('color: ' + ('#b7d59b' if available else '#a4a8b0'))
        self.setHidden(not text)
