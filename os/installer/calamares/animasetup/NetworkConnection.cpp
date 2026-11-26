#include "Network.h"
#include <QLabel>
#include <QVBoxLayout>
#include <QPushButton>
#include <QScrollArea>
#include <QProcess>
#include <QDialog>
#include <QDialogButtonBox>
#include <QLineEdit>
#include <QTimer>
#include <algorithm>

QLabel* description(const QString& text, QWidget* parent);
class NetworkName : public QLabel {
public:
    NetworkName(const QString& name, QWidget* parent) : QLabel(name,parent), fullName(name) {
        setTextFormat(Qt::PlainText); setSizePolicy(QSizePolicy::Ignored,QSizePolicy::Preferred);
        setAccessibleName(name); setToolTip("<qt>" + name.toHtmlEscaped() + "</qt>");
    }
protected:
    void resizeEvent(QResizeEvent* event) override {
        QLabel::resizeEvent(event);
        setText(fontMetrics().elidedText(fullName,Qt::ElideRight,width()));
    }
private:
    QString fullName;
};
static QString askPassword(QWidget* page, const QString& name, bool& accepted) {
    QDialog dialog(page,Qt::Dialog|Qt::FramelessWindowHint); dialog.setObjectName("animaNetworkDialog");
    dialog.resize(qMin(440,page->width()),240);
    auto* form = new QVBoxLayout(&dialog); form->setContentsMargins(24,24,24,24); form->setSpacing(16);
    auto* heading = description("Connect to Wi-Fi",&dialog); heading->setObjectName("animaDialogHeading"); form->addWidget(heading);
    form->addWidget(description(name,&dialog));
    auto* label = description("Password",&dialog); form->addWidget(label);
    auto* password = new QLineEdit(&dialog); password->setEchoMode(QLineEdit::Password);
    password->setAccessibleName("Wi-Fi password"); label->setBuddy(password); form->addWidget(password);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Cancel|QDialogButtonBox::Ok,&dialog);
    buttons->button(QDialogButtonBox::Ok)->setText("Connect"); buttons->button(QDialogButtonBox::Ok)->setObjectName("view-button-next"); form->addWidget(buttons);
    QObject::connect(buttons,&QDialogButtonBox::accepted,&dialog,&QDialog::accept);
    QObject::connect(buttons,&QDialogButtonBox::rejected,&dialog,&QDialog::reject);
    password->setFocus(); accepted = dialog.exec() == QDialog::Accepted; return password->text();
}
void NetworkPage::scanNetworks() {
    if (!wifiAvailable || !radioEnabled || connecting || scan->state() != QProcess::NotRunning) return;
    scanned = true; scanGeneration = generation; refresh->setEnabled(false); refresh->setText("Refreshing…");
    showMessage("Looking for Wi-Fi networks…");
    scan->start("nmcli", {"--wait","15","-t","-f","DEVICE,BSSID,SSID,SIGNAL,SECURITY,IN-USE","device","wifi","list","--rescan","yes"});
}
void NetworkPage::readNetworks(int code) {
    const auto output = QString::fromUtf8(scan->readAllStandardOutput()); scan->readAllStandardError();
    refresh->setText("Refresh"); refresh->setEnabled(!connecting);
    if (!wifiAvailable || !radioEnabled) return;
    if (scanGeneration != generation) { scanNetworks(); return; }
    clearNetworks();
    if (code != 0) { showMessage("Couldn't refresh networks. Try again."); return; }
    QList<WifiNetwork> available;
    for (const auto& line : output.split('\n',Qt::SkipEmptyParts)) {
        const auto values = networkFields(line);
        if (values.size() != 6 || values[2].isEmpty()) continue;
        available.append({values[0],values[1],values[2],values[3],values[4],values[5] == "*"});
    }
    std::stable_sort(available.begin(),available.end(),[](const auto& a, const auto& b) {
        return a.connected != b.connected ? a.connected : a.signal.toInt() > b.signal.toInt();
    });
    QStringList shown;
    for (const auto& network : available) {
        const auto key = network.device + '\n' + network.name + '\n' + network.security;
        if (shown.contains(key)) continue;
        shown.append(key); addNetwork(network);
    }
    if (available.isEmpty()) { showMessage("No Wi-Fi networks found. Refresh to try again, or use advanced settings for a hidden network."); return; }
    message->hide(); networks->show();
}
void NetworkPage::addNetwork(const WifiNetwork& network) {
    auto* row = new QWidget(networks); row->setObjectName("animaNetworkRow");
    auto* box = new QHBoxLayout(row); box->setContentsMargins(0,12,0,12); box->setSpacing(12);
    auto* labels = new QVBoxLayout; labels->setSpacing(4);
    auto* name = new NetworkName(network.name,row); labels->addWidget(name);
    const auto security = network.security.isEmpty() || network.security == "--" ? "Open network" : "Secured";
    auto* detail = description(QString("%1% signal · %2").arg(network.signal,security),row);
    detail->setObjectName("animaMuted"); detail->setSizePolicy(QSizePolicy::Ignored,QSizePolicy::Preferred); labels->addWidget(detail); box->addLayout(labels,1);
    if (network.connected) {
        auto* connected = description("Connected",row); connected->setObjectName("animaNetworkSection"); box->addWidget(connected);
    } else {
        auto* button = new QPushButton("Connect",row); button->setAccessibleName("Connect to " + network.name);
        box->addWidget(button); connect(button,&QPushButton::clicked,this,[this,network] { connectNetwork(network); });
    }
    rows->addWidget(row);
}
void NetworkPage::connectNetwork(const WifiNetwork& network) {
    if (connecting) return;
    const bool secured = !network.security.isEmpty() && network.security != "--";
    bool accepted = !secured; QString secret;
    if (secured) secret = askPassword(this,network.name,accepted);
    if (!accepted) { secret.fill(QChar(0)); return; }
    connecting = true; refresh->setEnabled(false); networks->setEnabled(false);
    message->setText("Connecting to " + network.name + "…"); message->show();
    connection->start("nmcli", {"--wait","30","--ask","device","wifi","connect",network.bssid,"ifname",network.device});
    // Keep the password out of process arguments and installer logs.
    connection->write(secret.toUtf8()+"\n"); connection->closeWriteChannel(); secret.fill(QChar(0));
}
void NetworkPage::connectionFinished(int code) {
    connection->readAllStandardOutput(); connection->readAllStandardError();
    connecting = false; networks->setEnabled(true); refresh->setEnabled(true);
    if (code != 0) {
        message->setText("Couldn't connect. Check the password and try again. Enterprise networks use advanced settings."); message->show();
        return;
    }
    watchDevices(); scanNetworks();
}
void NetworkPage::turnOnWifi() {
    enableWifi->setEnabled(false);
    auto* process = new QProcess(this);
    auto finish = [this,process](int code) {
        process->readAllStandardOutput(); process->readAllStandardError();
        enableWifi->setEnabled(true);
        if (code == 0) watchDevices();
        else showMessage("Couldn't turn on Wi-Fi. Check your computer's Wi-Fi switch and try again.");
        process->deleteLater();
    };
    connect(process,qOverload<int,QProcess::ExitStatus>(&QProcess::finished),this,[finish](int code, auto) { finish(code); });
    connect(process,&QProcess::errorOccurred,this,[finish](auto error) { if (error == QProcess::FailedToStart) finish(1); });
    QTimer::singleShot(15000,process,[process] { if (process->state() != QProcess::NotRunning) process->kill(); });
    process->start("nmcli", {"radio","wifi","on"});
}
