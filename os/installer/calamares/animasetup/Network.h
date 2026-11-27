#pragma once
#include <QWidget>
#include <QStringList>

class QLabel;
class QVBoxLayout;
class QPushButton;
class QProcess;
class QScrollArea;

struct WifiNetwork {
    QString device;
    QString bssid;
    QString name;
    QString signal;
    QString security;
    bool connected = false;
};

QStringList networkFields(const QString& line);
class NetworkPage : public QWidget {
public:
    NetworkPage();
private:
    QLabel* ethernet;
    QLabel* message;
    QPushButton* refresh;
    QPushButton* enableWifi;
    QScrollArea* networks;
    QVBoxLayout* rows;
    QProcess* devices;
    QProcess* radio;
    QProcess* scan;
    QProcess* connection;
    QString wifiSignature;
    bool wifiAvailable = false;
    bool radioEnabled = false;
    bool connecting = false;
    bool scanned = false;
    unsigned generation = 0;
    unsigned scanGeneration = 0;
    void buildPage();
    void watchDevices();
    void readDevices(int code);
    void readRadio(int code);
    void scanNetworks();
    void readNetworks(int code);
    void addNetwork(const WifiNetwork& network);
    void turnOnWifi();
    void connectNetwork(const WifiNetwork& network);
    void connectionFinished(int code);
    void clearNetworks();
    void showMessage(const QString& text);
};
