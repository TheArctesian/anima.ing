#pragma once
#include "ViewManager.h"
#include <QDialog>
#include <QLabel>
#include <QComboBox>
#include <QPushButton>
#include <QCheckBox>
#include <QVBoxLayout>
#include <QGridLayout>
#include <QProcess>
#include <QFile>
#include <QTimer>
#include <QGuiApplication>
#include <QScreen>
#include <QScrollArea>
#include <QShowEvent>
#include <QWindow>
#include <QPainter>

class AnimaSettings : public QDialog {
    QComboBox* screens;
    QComboBox* rotations;
    QLabel* status;
    QPushButton* apply;
    QPushButton* keep;
    QTimer timer;
    QString output;
    int remaining = 0;
    bool editable;
    static QIcon icon(const QString& name) {
        QPixmap image(40,40); image.setDevicePixelRatio(2); image.fill(Qt::transparent);
        QPainter pen(&image); pen.setRenderHint(QPainter::Antialiasing);
        pen.setPen(QPen(QColor(name == "check" ? "#16181B" : "#A4A8B0"),1.5,Qt::SolidLine,Qt::RoundCap,Qt::RoundJoin));
        if (name == "display") {
            pen.drawRoundedRect(QRectF(2,3,16,11),1,1); pen.drawLine(10,14,10,18); pen.drawLine(6,18,14,18);
        } else if (name == "motion") {
            pen.drawEllipse(QRectF(8,5,10,10)); pen.drawLine(2,6,5,6); pen.drawLine(1,10,4,10); pen.drawLine(2,14,5,14);
        } else if (name == "terminal") {
            pen.drawRoundedRect(QRectF(2,3,16,14),1,1); pen.drawLine(5,7,8,10); pen.drawLine(8,10,5,13); pen.drawLine(11,13,15,13);
        } else if (name == "power") {
            pen.drawArc(QRectF(3,3,14,14),125*16,290*16); pen.drawLine(10,1,10,9);
        } else if (name == "check") {
            pen.drawLine(4,10,8,14); pen.drawLine(8,14,16,5);
        } else {
            pen.drawArc(QRectF(4,4,12,12),35*16,290*16); pen.drawLine(16,3,16,8); pen.drawLine(16,8,11,8);
        }
        return QIcon(image);
    }
    QWidget* section(const QString& title, const QString& symbol) {
        auto* heading=new QWidget(this); auto* row=new QHBoxLayout(heading);
        row->setContentsMargins(0,0,0,0); row->setSpacing(10);
        auto* glyph=new QLabel(heading); glyph->setPixmap(icon(symbol).pixmap(20,20)); glyph->setFixedSize(20,20);
        auto* label=new QLabel(title,heading); label->setObjectName("animaSettingsSection");
        row->addWidget(glyph); row->addWidget(label); row->addStretch(); return heading;
    }
    void fitToScreen() {
        auto* display=screen(); if (!display) return;
        const QRect available=display->availableGeometry().adjusted(16,16,-16,-16);
        setMaximumSize(available.size()); resize(QSize(480,600).boundedTo(available.size()));
        move(available.center()-QPoint(width()/2,height()/2));
    }
    void observeScreen(QScreen* display) {
        connect(display,&QScreen::geometryChanged,this,[this]{QTimer::singleShot(0,this,&AnimaSettings::fitToScreen);});
        connect(display,&QScreen::availableGeometryChanged,this,[this]{QTimer::singleShot(0,this,&AnimaSettings::fitToScreen);});
    }
    void showEvent(QShowEvent* event) override {
        QDialog::showEvent(event);
        connect(windowHandle(),&QWindow::screenChanged,this,&AnimaSettings::fitToScreen,Qt::UniqueConnection);
        QTimer::singleShot(0,this,&AnimaSettings::fitToScreen);
    }
    bool change(const QString& rotation) {
        QProcess command; command.start("anima-display", {"--set",output,rotation});
        if (!command.waitForFinished(5000)) { command.kill(); command.waitForFinished(); return false; }
        return command.exitStatus()==QProcess::NormalExit && command.exitCode()==0;
    }
    void revert() {
        if (remaining<=0) return;
        timer.stop(); QProcess command; command.start("anima-display", {"--revert"});
        const bool restored=command.waitForFinished(7000) && command.exitStatus()==QProcess::NormalExit && command.exitCode()==0;
        remaining=0; screens->setEnabled(editable); rotations->setEnabled(editable);
        if (restored) rotations->setCurrentIndex(qMax(0,rotations->findData(screens->currentData())));
        status->setText(restored ? "Previous orientation restored." : "Restore failed. Use Ctrl+Alt+F2 for recovery.");
        keep->hide(); apply->setEnabled(editable && screens->count()>0);
    }
    void done(int result) override { revert(); QDialog::done(result); }
    void applyRotation() {
        output=screens->currentText(); if (output.isEmpty() || !editable) return;
        if (!change(rotations->currentData().toString())) { status->setText("Unable to change this display."); return; }
        remaining=15; screens->setEnabled(false); rotations->setEnabled(false); apply->setEnabled(false); keep->show(); timer.start(1000);
        status->setText("Keep this orientation? Reverting in 15 seconds."); keep->setFocus();
    }
    void keepRotation() {
        QProcess save; save.start("anima-display",{"--keep",output});
        if (!save.waitForFinished(5000) || save.exitStatus()!=QProcess::NormalExit || save.exitCode()!=0) {
            save.kill(); save.waitForFinished(); status->setText("Could not save orientation. The preview will revert."); return;
        }
        timer.stop(); remaining=0; screens->setEnabled(editable); rotations->setEnabled(editable);
        screens->setItemData(screens->currentIndex(),rotations->currentData()); keep->hide(); apply->setEnabled(editable);
        status->setText("Orientation saved for the installed system."); apply->setFocus();
    }
    void displaySettings(QVBoxLayout* box) {
        box->addWidget(section("Display","display"));
        auto* form=new QGridLayout; form->setHorizontalSpacing(16); form->setVerticalSpacing(10); form->setColumnStretch(1,1);
        screens=new QComboBox(this); rotations=new QComboBox(this);
        screens->setAccessibleName("Display"); rotations->setAccessibleName("Orientation");
        QProcess query; query.start("anima-display",{"--list"}); query.waitForFinished(5000);
        for(const auto& line:QString::fromUtf8(query.readAllStandardOutput()).split('\n',Qt::SkipEmptyParts)) {
            const auto columns=line.split('\t'); if(columns.size()==2) screens->addItem(columns[0],columns[1]);
        }
        rotations->addItem("Landscape","normal"); rotations->addItem("Portrait left","left");
        rotations->addItem("Portrait right","right"); rotations->addItem("Landscape flipped","inverted");
        auto selectRotation=[this]{ rotations->setCurrentIndex(qMax(0,rotations->findData(screens->currentData()))); };
        connect(screens,&QComboBox::currentIndexChanged,this,selectRotation); selectRotation();
        form->addWidget(new QLabel("Display",this),0,0); form->addWidget(screens,0,1);
        form->addWidget(new QLabel("Orientation",this),1,0); form->addWidget(rotations,1,1); box->addLayout(form);
        screens->setEnabled(editable); rotations->setEnabled(editable);
        auto* actions=new QHBoxLayout; actions->setSpacing(8); actions->addStretch();
        apply=new QPushButton(icon("rotate"),"Apply",this); apply->setEnabled(editable && screens->count()>0); actions->addWidget(apply);
        keep=new QPushButton(icon("check"),"Keep changes",this); keep->setObjectName("view-button-next"); actions->addWidget(keep); keep->hide(); box->addLayout(actions);
        status=new QLabel(editable ? "Changes revert after 15 seconds unless kept." : "Display changes are unavailable during installation.",this);
        status->setObjectName("animaMuted"); status->setWordWrap(true); box->addWidget(status);
        connect(apply,&QPushButton::clicked,this,&AnimaSettings::applyRotation);
        connect(keep,&QPushButton::clicked,this,&AnimaSettings::keepRotation);
        connect(&timer,&QTimer::timeout,this,[this]{if(remaining==1){revert();return;} --remaining;status->setText(QString("Reverting in %1 seconds…").arg(remaining));});
    }
    void sessionSettings(QVBoxLayout* box) {
        box->addSpacing(12); box->addWidget(section("Appearance","motion"));
        auto* motion=new QCheckBox("Reduce background motion",this); motion->setChecked(QFile::exists("/run/anima-reduced-motion")); box->addWidget(motion);
        connect(motion,&QCheckBox::toggled,this,[](bool off){QFile f("/run/anima-reduced-motion");if(off) f.open(QIODevice::WriteOnly); else f.remove();});
        box->addSpacing(12); box->addWidget(section("Advanced","terminal"));
        auto* recovery=new QPushButton(icon("terminal"),"Recovery terminal",this); box->addWidget(recovery,0,Qt::AlignLeft);
        connect(recovery,&QPushButton::clicked,this,[]{QProcess::startDetached("xfce4-terminal",{"--disable-server"});});
        auto* power=new QWidget(this); power->setObjectName("animaSettingsPower");
        auto* powerRow=new QHBoxLayout(power); powerRow->setContentsMargins(0,16,0,0);
        auto* shutdown=new QPushButton(icon("power"),"Shut down",this); shutdown->setToolTip("Shut down this live session");
        shutdown->setEnabled(editable); powerRow->addWidget(shutdown); powerRow->addStretch(); box->addWidget(power);
        connect(shutdown,&QPushButton::clicked,this,[]{QProcess::startDetached("systemctl",{"poweroff"});});
    }
public:
    explicit AnimaSettings(QWidget* parent) : QDialog(parent,Qt::Dialog|Qt::FramelessWindowHint) {
        setObjectName("animaSettings"); editable=Calamares::ViewManager::instance()->currentStepIndex()<6;
        auto* frame=new QVBoxLayout(this); frame->setContentsMargins(24,20,24,20); frame->setSpacing(16);
        auto* heading=new QLabel("Settings",this); heading->setObjectName("animaDialogHeading"); frame->addWidget(heading);
        auto* scroll=new QScrollArea(this); scroll->setWidgetResizable(true); scroll->setFrameShape(QFrame::NoFrame);
        scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        auto* content=new QWidget(scroll); scroll->setWidget(content); frame->addWidget(scroll,1);
        auto* box=new QVBoxLayout(content); box->setContentsMargins(0,0,8,0); box->setSpacing(12);
        displaySettings(box); sessionSettings(box); box->addStretch();
        auto* footer=new QHBoxLayout; footer->addStretch();
        auto* close=new QPushButton("Done",this); close->setObjectName("view-button-next"); footer->addWidget(close); frame->addLayout(footer);
        connect(close,&QPushButton::clicked,this,&QDialog::accept);
        for (auto* display:QGuiApplication::screens()) observeScreen(display);
        connect(qGuiApp,&QGuiApplication::screenAdded,this,&AnimaSettings::observeScreen);
    }
    ~AnimaSettings() override { revert(); }
};
