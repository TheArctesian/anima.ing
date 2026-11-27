#include "Network.h"
#include <QLabel>
#include <QVBoxLayout>
#include <QPushButton>
#include <QScrollArea>
#include <QProcess>
#include <QProcessEnvironment>
#include <QTimer>

QLabel* description(const QString& text, QWidget* parent);
QStringList networkFields(const QString& line) {
    QStringList result; QString word; bool escape = false;
    for (const auto c : line) {
        if (escape) { word += c; escape = false; }
        else if (c == '\\') escape = true;
        else if (c == ':') { result.append(word); word.clear(); }
        else word += c;
    }
    result.append(word); return result;
}
NetworkPage::NetworkPage() {
    buildPage();
    devices = new QProcess(this); radio = new QProcess(this);
    scan = new QProcess(this); connection = new QProcess(this);
    auto environment = QProcessEnvironment::systemEnvironment();
    environment.insert("LC_ALL", "C");
    for (auto* process : {devices, radio, scan, connection}) {
        process->setProcessEnvironment(environment);
        auto* timeout = new QTimer(process); timeout->setSingleShot(true);
        connect(process, &QProcess::started, timeout, [timeout] { timeout->start(45000); });
        connect(timeout, &QTimer::timeout, process, &QProcess::kill);
        connect(process, qOverload<int,QProcess::ExitStatus>(&QProcess::finished), timeout, &QTimer::stop);
    }
    connect(devices, qOverload<int,QProcess::ExitStatus>(&QProcess::finished), this, [this](int code, auto) { readDevices(code); });
    connect(radio, qOverload<int,QProcess::ExitStatus>(&QProcess::finished), this, [this](int code, auto) { readRadio(code); });
    connect(scan, qOverload<int,QProcess::ExitStatus>(&QProcess::finished), this, [this](int code, auto) { readNetworks(code); });
    connect(connection, qOverload<int,QProcess::ExitStatus>(&QProcess::finished), this, [this](int code, auto) { connectionFinished(code); });
    connect(devices, &QProcess::errorOccurred, this, [this](auto error) { if (error == QProcess::FailedToStart) readDevices(1); });
    connect(radio, &QProcess::errorOccurred, this, [this](auto error) { if (error == QProcess::FailedToStart) readRadio(1); });
    connect(scan, &QProcess::errorOccurred, this, [this](auto error) { if (error == QProcess::FailedToStart) readNetworks(1); });
    connect(connection, &QProcess::errorOccurred, this, [this](auto error) { if (error == QProcess::FailedToStart) connectionFinished(1); });
    connect(refresh, &QPushButton::clicked, this, &NetworkPage::scanNetworks);
    auto* timer = new QTimer(this); timer->setInterval(4000);
    connect(timer, &QTimer::timeout, this, &NetworkPage::watchDevices);
    timer->start(); watchDevices();
}
void NetworkPage::buildPage() {
    setObjectName("animaNetwork");
    setStyleSheet("QWidget#animaNetworkRow { border-bottom: 1px solid #2C3036; }"
                  "QLabel#animaNetworkSection { font-weight: 500; }"
                  "QPushButton#animaAdvanced { background: transparent; border: none; text-align: left; padding-left: 0; }"
                  "QPushButton#animaAdvanced:focus { border: 1px solid #88C0D0; }");
    auto* box = new QVBoxLayout(this); box->setContentsMargins(32,28,32,24); box->setSpacing(16);
    auto* title = description("Network", this); title->setObjectName("animaHeading"); box->addWidget(title);
    box->addWidget(description("Connect to a network, or continue offline.", this));
    auto* wired = new QWidget(this); wired->setObjectName("animaNetworkRow");
    auto* wiredBox = new QVBoxLayout(wired); wiredBox->setContentsMargins(0,12,0,16); wiredBox->setSpacing(4);
    auto* wiredTitle = description("Ethernet", wired); wiredTitle->setObjectName("animaNetworkSection"); wiredBox->addWidget(wiredTitle);
    ethernet = description("Checking connection…", wired); ethernet->setObjectName("animaMuted"); wiredBox->addWidget(ethernet); box->addWidget(wired);
    auto* heading = new QHBoxLayout; auto* wifi = description("Wi-Fi", this); wifi->setObjectName("animaNetworkSection");
    heading->addWidget(wifi); heading->addStretch(); refresh = new QPushButton("Refresh", this);
    refresh->setAccessibleName("Refresh Wi-Fi networks"); refresh->setVisible(false); heading->addWidget(refresh); box->addLayout(heading);
    message = description("Checking Wi-Fi…", this); message->setObjectName("animaMuted"); box->addWidget(message);
    enableWifi = new QPushButton("Turn on Wi-Fi", this); enableWifi->hide();
    box->addWidget(enableWifi,0,Qt::AlignLeft);
    connect(enableWifi,&QPushButton::clicked,this,&NetworkPage::turnOnWifi);
    networks = new QScrollArea(this); networks->setWidgetResizable(true); networks->setFrameShape(QFrame::NoFrame);
    networks->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff); networks->setMinimumHeight(120);
    auto* content = new QWidget; rows = new QVBoxLayout(content); rows->setContentsMargins(0,0,0,0); rows->setSpacing(0);
    rows->setAlignment(Qt::AlignTop); networks->setWidget(content); networks->hide(); box->addWidget(networks,1);
    auto* advanced = new QPushButton("Advanced settings…", this); advanced->setObjectName("animaAdvanced");
    advanced->setToolTip("Hidden networks, enterprise Wi-Fi and static addresses");
    auto* advancedBox = new QHBoxLayout; advancedBox->addWidget(advanced); advancedBox->addStretch(); box->addLayout(advancedBox);
    auto* note = description("Network settings carry over to your installed system.", this); note->setObjectName("animaMuted"); box->addWidget(note);
    box->addStretch();
    connect(advanced,&QPushButton::clicked,this,[] { QProcess::startDetached("nm-connection-editor", {}); });
}
void NetworkPage::watchDevices() {
    if (devices->state() == QProcess::NotRunning && radio->state() == QProcess::NotRunning)
        devices->start("nmcli", {"-t","-f","DEVICE,TYPE,STATE","device","status"});
}
void NetworkPage::readDevices(int code) {
    const auto output = QString::fromUtf8(devices->readAllStandardOutput()); devices->readAllStandardError();
    if (code != 0) {
        wifiAvailable = false; radioEnabled = false; scanned = false; ++generation;
        ethernet->setText("Connection status unavailable"); refresh->hide(); enableWifi->hide();
        showMessage("Couldn't check Wi-Fi. Retrying…"); return;
    }
    QStringList wired; QStringList wifiDevices;
    for (const auto& line : output.split('\n',Qt::SkipEmptyParts)) {
        const auto values = networkFields(line); if (values.size() != 3) continue;
        if (values[1] == "wifi") wifiDevices.append(values[0] + ":" + values[2]);
        if (values[1] == "ethernet") {
            const auto state = values[2] == "connected" ? "Connected" : values[2] == "unavailable" ? "No cable connected" : "Not connected";
            wired.append(state);
        }
    }
    ethernet->setText(wired.isEmpty() ? "No Ethernet adapter" : wired.join('\n'));
    wifiDevices.sort();
    const auto signature = wifiDevices.join("\n");
    if (wifiSignature != signature) { ++generation; scanned = false; }
    wifiSignature = signature; wifiAvailable = !wifiDevices.isEmpty();
    if (!wifiAvailable) {
        scanned = false; radioEnabled = false; refresh->hide(); enableWifi->hide();
        showMessage("Wi-Fi unavailable\nNo Wi-Fi adapter detected."); return;
    }
    radio->start("nmcli", {"radio","wifi"});
}
void NetworkPage::readRadio(int code) {
    const auto state = QString::fromUtf8(radio->readAllStandardOutput()).trimmed(); radio->readAllStandardError();
    const bool enabled = code == 0 && state == "enabled";
    if (radioEnabled != enabled) ++generation;
    radioEnabled = enabled;
    enableWifi->setVisible(code == 0 && !radioEnabled);
    refresh->setVisible(radioEnabled); refresh->setEnabled(!connecting && scan->state() == QProcess::NotRunning);
    if (!radioEnabled) {
        scanned = false;
        showMessage(code == 0 ? "Wi-Fi is turned off." : "Couldn't check Wi-Fi. Retrying…");
        return;
    }
    if (!scanned && !connecting) scanNetworks();
}
void NetworkPage::clearNetworks() {
    while (auto* row = rows->takeAt(0)) { delete row->widget(); delete row; }
}
void NetworkPage::showMessage(const QString& text) {
    networks->hide(); message->setText(text); message->show();
}
QWidget* networkPage() { return new NetworkPage; }
