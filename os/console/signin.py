"""Separate, off-record sign-in browser with compact native navigation."""
from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QColor, QPainter
from PySide6.QtWidgets import QApplication, QHBoxLayout, QLabel, QLineEdit, QPushButton, QScrollArea, QSizePolicy, QStackedWidget, QVBoxLayout, QWidget
from PySide6.QtWebEngineCore import QWebEnginePage, QWebEngineProfile
from PySide6.QtWebEngineWidgets import QWebEngineView
import qrcode
from remote_status import RemoteConnections


class SigninCode(QWidget):
    def __init__(self):
        super().__init__()
        self.matrix = []
        self.setFixedSize(240, 240)
        self.setAccessibleName('Scan to sign in on another device')

    def set_url(self, url):
        self.matrix = []
        if url:
            code = qrcode.QRCode(error_correction=qrcode.constants.ERROR_CORRECT_M, border=4)
            code.add_data(url)
            code.make(fit=True)
            self.matrix = code.get_matrix()
        self.update()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.fillRect(self.rect(), Qt.GlobalColor.white)
        if not self.matrix:
            return
        unit = self.width() // len(self.matrix)
        offset = (self.width() - unit * len(self.matrix)) // 2
        for y, row in enumerate(self.matrix):
            for x, filled in enumerate(row):
                if filled:
                    painter.fillRect(offset+x*unit, offset+y*unit, unit, unit, Qt.GlobalColor.black)


class BrowserPage(QWebEnginePage):
    def acceptNavigationRequest(self, url, kind, main_frame):
        return url.scheme() == 'https' or url.toString() == 'about:blank'


class HostLabel(QLabel):
    def __init__(self):
        super().__init__()
        self.host = ''
        self.setTextFormat(Qt.TextFormat.PlainText)
        self.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        self.setSizePolicy(QSizePolicy.Policy.Ignored, QSizePolicy.Policy.Preferred)

    def show_host(self, url):
        self.host = url.host()
        self.setToolTip(self.host)
        self.setText(self.fontMetrics().elidedText(self.host, Qt.TextElideMode.ElideMiddle, self.width()))

    def resizeEvent(self, event):
        super().resizeEvent(event)
        self.setText(self.fontMetrics().elidedText(self.host, Qt.TextElideMode.ElideMiddle, self.width()))


class SigninWindow(QWidget):
    def __init__(self):
        super().__init__()
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint)
        self.setStyleSheet('QWidget{background:#1B1D21;color:#E6E7E9;font:14px "IBM Plex Sans";}QWidget#signinHeader{border-bottom:1px solid #2C3036;}QLabel{border:0;}QPushButton{padding:8px 14px;border:1px solid #353941;border-radius:4px;}QPushButton:hover,QPushButton:focus{border-color:#88C0D0;}')
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        header = QWidget(self)
        header.setObjectName('signinHeader')
        header.setFixedHeight(64)
        chrome = QVBoxLayout(header)
        chrome.setContentsMargins(24, 4, 24, 4)
        chrome.setSpacing(0)
        row = QHBoxLayout()
        row.setSpacing(16)
        back = QPushButton('← Back to Anima', header)
        self.confirmation = QLabel(header)
        self.confirmation.setTextFormat(Qt.TextFormat.PlainText)
        self.confirmation.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.origin = HostLabel()
        self.remote_connections = RemoteConnections()
        self.alternative = QPushButton('QR / code', header)
        self.alternative.setAccessibleName('Sign in with a phone or another computer')
        row.addWidget(back)
        row.addWidget(self.alternative)
        row.addWidget(self.confirmation, 1)
        row.addWidget(self.origin, 1)
        chrome.addLayout(row, 1)
        chrome.addWidget(self.remote_connections, 0, Qt.AlignmentFlag.AlignRight)
        self.web = QWebEngineView(self)
        self.profile = QWebEngineProfile(self.web)
        page = BrowserPage(self.profile, self.web)
        page.setBackgroundColor(QColor('#1B1D21'))
        self.web.setPage(page)
        layout.addWidget(header, 0)
        self.pages = QStackedWidget(self)
        self.pages.addWidget(self.web)
        self.pages.addWidget(self.code_page())
        layout.addWidget(self.pages, 1)
        back.clicked.connect(self.hide)
        self.alternative.clicked.connect(self.toggle_method)
        self.web.urlChanged.connect(self.origin.show_host)

    def code_page(self):
        scroll = QScrollArea(self)
        scroll.setWidgetResizable(True)
        content = QWidget()
        outer = QVBoxLayout(content)
        outer.setContentsMargins(24, 24, 24, 24)
        self.code_content = QWidget()
        column = QVBoxLayout(self.code_content)
        column.setContentsMargins(0, 0, 0, 0)
        column.setSpacing(12)
        title = QLabel('Sign in on another device')
        title.setStyleSheet('font-size:24px;')
        title.setWordWrap(True)
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        note = QLabel('Scan with your phone, or open the sign-in link on another computer.\nConfirm the code below when asked. This computer will continue automatically.')
        note.setWordWrap(True)
        note.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.qr = SigninCode()
        self.code = QLineEdit()
        self.code.setReadOnly(True)
        self.code.setAccessibleName('Sign-in confirmation code')
        self.code.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.code.setStyleSheet('font-size:24px;letter-spacing:3px;padding:8px;')
        self.code.setMinimumWidth(280)
        self.code.setMaximumWidth(360)
        self.link = QLineEdit()
        self.link.setReadOnly(True)
        self.link.setAccessibleName('Sign-in link for another device')
        self.link.setCursorPosition(0)
        actions = QHBoxLayout()
        actions.addStretch()
        for label, field in (('Copy code', self.code), ('Copy link', self.link)):
            button = QPushButton(label)
            button.clicked.connect(lambda checked=False, field=field: QApplication.clipboard().setText(field.text()))
            actions.addWidget(button)
        actions.addStretch()
        column.addWidget(title)
        column.addWidget(note)
        column.addWidget(self.qr, 0, Qt.AlignmentFlag.AlignHCenter)
        column.addWidget(self.code, 0, Qt.AlignmentFlag.AlignHCenter)
        column.addWidget(self.link)
        column.addLayout(actions)
        outer.addStretch()
        outer.addWidget(self.code_content, 0, Qt.AlignmentFlag.AlignHCenter)
        outer.addStretch()
        scroll.setWidget(content)
        return scroll

    def resizeEvent(self, event):
        super().resizeEvent(event)
        self.code_content.setFixedWidth(max(280, min(560, self.width()-64)))

    def toggle_method(self):
        remote = self.pages.currentIndex() == 0
        self.pages.setCurrentIndex(1 if remote else 0)
        self.alternative.setText('Sign in here' if remote else 'QR / code')
        self.alternative.setAccessibleName('Sign in on this computer' if remote else 'Sign in with a phone or another computer')
        if remote:
            self.code.setFocus()

    def open_signin(self, url, code):
        self.confirmation.setText('Confirm code: '+code)
        self.qr.set_url(url)
        self.code.setText(code)
        self.link.setText(url)
        self.link.setCursorPosition(0)
        self.pages.setCurrentIndex(0)
        self.alternative.setText('QR / code')
        self.alternative.setAccessibleName('Sign in with a phone or another computer')
        self.web.load(QUrl(url))
        self.showFullScreen()

    def clear(self):
        self.hide()
        self.qr.set_url('')
        self.code.clear()
        self.link.clear()
        self.confirmation.clear()
        self.web.load(QUrl('about:blank'))
        self.profile.cookieStore().deleteAllCookies()
