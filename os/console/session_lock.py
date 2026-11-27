"""Session-local lock entry point for the shared desktop's global shortcut."""
from PySide6.QtCore import ClassInfo, QObject, Slot
from PySide6.QtDBus import QDBusConnection


@ClassInfo(**{'D-Bus Interface': 'ing.anima.Console'})
class SessionLock(QObject):
    def __init__(self, bridge):
        super().__init__(bridge)
        self.bridge = bridge
        self.bus = QDBusConnection.sessionBus()
        if not self.bus.registerObject('/ing/anima/Console', self,
                                       QDBusConnection.RegisterOption.ExportAllSlots):
            raise RuntimeError('Cannot register the local console lock action')
        if not self.bus.registerService('ing.anima.Console'):
            raise RuntimeError('Another console owns this desktop session')

    @Slot()
    def Lock(self):
        self.bridge.lock()
