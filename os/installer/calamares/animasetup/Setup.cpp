#include "Setup.h"
#include "Review.h"
#include <QJsonDocument>
#include "GlobalStorage.h"
#include "JobQueue.h"
#include <QPushButton>
#include <QProcess>
#include <QFile>
#include <QScrollArea>
#include <QApplication>

QLabel* description(const QString& text, QWidget* parent) {
    auto* label = new QLabel(text, parent);
    label->setWordWrap(true);
    label->setTextFormat(Qt::PlainText);
    return label;
}
Setup::Setup(QObject* parent) : Calamares::ViewStep(parent) {
    connect(Calamares::JobQueue::instance(), &Calamares::JobQueue::failed, this, [this] { failed = true; });
}
QString Setup::prettyName() const { return mode; }
bool Setup::isNextEnabled() const {
    if (mode == "Ready") return false;
    if (mode == "Storage") return systemDisk && systemDisk->currentIndex() > 0;
    if (mode == "Review") {
        const auto selected = Calamares::JobQueue::instance()->globalStorage()->value("animaSelectedDisks").toList();
        return confirmation && confirmation->isEnabled() && confirmation->isChecked() && selected == reviewSelection;
    }
    return true;
}
void Setup::setConfigurationMap(const QVariantMap& config) {
    mode = config.value("page").toString();
    if (mode == "Welcome") {
        page = heroPage("Install ANIMA OS", "A home for your applications and agents.");
        return;
    }
    if (mode == "Network") { page = networkPage(); return; }
    if (mode == "Ready") {
        page = heroPage("Installation complete", "Your node is ready to start.\nRestart into Anima OS, then unlock with the admin password you chose. Sign in to connect this node to your Anima apps.\nTo remove the installation USB first, choose Shut down.");
        page->setProperty("animaCompletion",true);
        return;
    }
    page = new QWidget;
    layout = new QVBoxLayout(page);
    layout->setContentsMargins(32, 28, 32, 24);
    auto* title = description(mode == "Review" ? "Review installation" : "Storage", page);
    title->setObjectName("animaHeading"); layout->addWidget(title);
    if (mode == "Storage") storage();
    if (mode == "Review") {
        review = new QScrollArea(page); review->setWidgetResizable(true);
        review->setFrameShape(QFrame::NoFrame); review->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        layout->addWidget(review);
        auto* gs = Calamares::JobQueue::instance()->globalStorage();
        // GlobalStorage emits changed while holding its write lock.
        connect(gs, &Calamares::GlobalStorage::changed, this, [this,gs] {
            if (confirmation && confirmation->isEnabled() && gs->value("animaSelectedDisks").toList() != reviewSelection) {
                confirmation->setChecked(false); confirmation->setEnabled(false); gs->remove("animaAuthorizedDisks");
            }
        }, Qt::QueuedConnection);
    }
}
void Setup::onActivate() {
    if (mode == "Review") refreshReview();
    if (mode == "Ready" && failed) {
        for (auto* label : page->findChildren<QLabel*>()) label->setText("Installation failed. Open recovery or use Ctrl+Alt+F2 for diagnostics. Selected disks may be partially written; do not assume the system is ready.");
    }
    emit nextStatusChanged(isNextEnabled());
}
void Setup::onLeave() {
    if (mode == "Review" && isNextEnabled()) {
        auto* gs = Calamares::JobQueue::instance()->globalStorage();
        gs->insert("animaAuthorizedDisks", gs->value("animaSelectedDisks"));
    }
}
void Setup::refreshReview() {
    auto* gs = Calamares::JobQueue::instance()->globalStorage();
    if (confirmation) confirmation->setChecked(false);
    reviewSelection = gs->value("animaSelectedDisks").toList(); gs->remove("animaAuthorizedDisks");
    QFile file("/run/anima-installer-disks.json"); disks = {};
    if (file.open(QIODevice::ReadOnly)) disks = QJsonDocument::fromJson(file.readAll()).array();
    auto* content = installationReview(disks,reviewSelection,gs->value("keyboardLayout").toString());
    confirmation = new QCheckBox("I authorize erasing these disks.", content);
    confirmation->setAccessibleName("I authorize erasing every selected disk shown above.");
    confirmation->setEnabled(!reviewSelection.isEmpty()); content->layout()->addWidget(confirmation);
    connect(confirmation, &QCheckBox::toggled, this, [this] { emit nextStatusChanged(isNextEnabled()); });
    review->setWidget(content);
}
CALAMARES_PLUGIN_FACTORY_DEFINITION(SetupFactory, registerPlugin<Setup>();)
