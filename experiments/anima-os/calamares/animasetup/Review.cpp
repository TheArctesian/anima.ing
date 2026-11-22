#include "Review.h"
#include <QFormLayout>
#include <QFrame>
#include <QJsonObject>
#include <QLabel>
#include <QToolButton>
#include <QVBoxLayout>
#include <QPainter>
#include <QTextLayout>
#include <QtMath>

class ReviewLabel : public QLabel {
public:
    using QLabel::QLabel;
    int heightForWidth(int width) const override {
        QTextLayout content(text(),font()); return arrange(content,width);
    }
    QSize minimumSizeHint() const override { return {0,fontMetrics().height()}; }
protected:
    void paintEvent(QPaintEvent*) override {
        QPainter painter(this); painter.setPen(palette().color(foregroundRole()));
        QTextLayout content(text(),font()); arrange(content,contentsRect().width());
        content.draw(&painter,contentsRect().topLeft());
    }
private:
    static int arrange(QTextLayout& content, int width) {
        QTextOption options; options.setWrapMode(QTextOption::WrapAtWordBoundaryOrAnywhere); content.setTextOption(options);
        qreal height = 0; content.beginLayout();
        while (true) {
            auto line = content.createLine(); if (!line.isValid()) break;
            line.setLineWidth(qMax(1,width)); line.setPosition(QPointF(0,height)); height += line.height();
        }
        content.endLayout(); return qCeil(height);
    }
};
static QLabel* text(const QString& value, QWidget* parent) {
    auto* label = new ReviewLabel(value, parent);
    label->setTextFormat(Qt::PlainText);
    label->setWordWrap(true);
    label->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    return label;
}
static QWidget* diskCard(const QJsonObject& disk, const QString& role, QWidget* parent) {
    auto* card = new QFrame(parent); card->setObjectName("animaReviewDisk");
    auto* box = new QVBoxLayout(card); box->setContentsMargins(16,12,16,12); box->setSpacing(4);
    auto* action = text(role, card); action->setObjectName("animaReviewAction"); box->addWidget(action);
    const auto model = disk["model"].toString().trimmed(), path = disk["path"].toString();
    auto* name = text(model.isEmpty() ? path : model, card); name->setObjectName("animaReviewName"); box->addWidget(name);
    const auto bytes = disk["size"].toVariant().toLongLong();
    const auto capacity = bytes > 0 ? QString::number(bytes / (1024.0*1024*1024), 'f', 1) + " GiB" : "Capacity unavailable";
    auto* location = text(model.isEmpty() ? capacity : capacity + " · " + path, card);
    location->setObjectName("animaMuted"); box->addWidget(location);
    const auto serial = disk["serial"].toString().trimmed();
    auto* identity = text("Serial: " + (serial.isEmpty() ? "Not reported" : serial), card);
    identity->setObjectName("animaMuted"); box->addWidget(identity);
    return card;
}
static QWidget* summary(const QString& keyboard, QWidget* parent) {
    auto* widget = new QWidget(parent); auto* form = new QFormLayout(widget);
    form->setContentsMargins(0,8,0,8); form->setHorizontalSpacing(28); form->setVerticalSpacing(8);
    form->setLabelAlignment(Qt::AlignLeft | Qt::AlignTop);
    form->setRowWrapPolicy(QFormLayout::WrapLongRows);
    form->setFieldGrowthPolicy(QFormLayout::AllNonFixedFieldsGrow);
    form->addRow("Local account", text("admin", widget));
    form->addRow("Keyboard", text(keyboard.isEmpty() ? "Default layout" : keyboard, widget));
    form->addRow("Disk encryption", text("Off · Not available", widget));
    return widget;
}
static QWidget* untouched(const QJsonArray& disks, const QVariantList& selected, QWidget* parent) {
    auto* group = new QWidget(parent); auto* box = new QVBoxLayout(group);
    box->setContentsMargins(0,0,0,0); box->setSpacing(8);
    auto* toggle = new QToolButton(group); toggle->setObjectName("animaReviewDisclosure");
    toggle->setCheckable(true); toggle->setToolButtonStyle(Qt::ToolButtonTextBesideIcon); toggle->setArrowType(Qt::RightArrow);
    auto* content = new QWidget(group); auto* list = new QVBoxLayout(content);
    list->setContentsMargins(12,0,0,0); list->setSpacing(8); int count = 0;
    for (const auto& value : disks) {
        const auto disk = value.toObject(); const auto type = disk["type"].toString();
        if ((type != "disk" && type != "rom") || selected.contains(disk["path"].toString())) continue;
        const auto model = disk["model"].toString().trimmed(), path = disk["path"].toString();
        auto* label = text(model.isEmpty() ? path : model + " · " + path, content);
        label->setObjectName("animaMuted"); list->addWidget(label); ++count;
    }
    toggle->setText(QString("Untouched drives (%1)").arg(count));
    box->addWidget(toggle,0,Qt::AlignLeft); box->addWidget(content); content->hide();
    QObject::connect(toggle,&QToolButton::toggled,content,[content,toggle](bool open) {
        content->setVisible(open); toggle->setArrowType(open ? Qt::DownArrow : Qt::RightArrow);
    });
    if (!count) group->hide();
    return group;
}
QWidget* installationReview(const QJsonArray& disks, const QVariantList& selected, const QString& keyboard) {
    auto* content = new QWidget; content->setObjectName("animaReview");
    content->setStyleSheet("QFrame#animaReviewDisk { background:#23262B; border:1px solid #353941; border-radius:4px; }"
                          "QLabel#animaReviewAction { color:#D8B47B; font-weight:500; }"
                          "QLabel#animaReviewName { font-weight:500; }"
                          "QToolButton#animaReviewDisclosure { background:transparent; border:none; text-align:left; padding:8px 0; }"
                          "QToolButton#animaReviewDisclosure:focus { border:1px solid #88C0D0; }");
    auto* box = new QVBoxLayout(content); box->setContentsMargins(0,0,0,0); box->setSpacing(12); box->setAlignment(Qt::AlignTop);
    box->addWidget(text(selected.isEmpty() ? "No installation disk selected. Go back to Storage." : "All data on the selected disks below will be erased.",content));
    for (int i = 0; i < selected.size(); ++i) {
        const auto path = selected[i].toString(); QJsonObject disk{{"path",path}};
        for (const auto& value : disks) if (value.toObject()["path"].toString() == path) { disk = value.toObject(); break; }
        box->addWidget(diskCard(disk, i == 0 ? "Erase and install Anima OS" : "Erase and add data volume", content));
    }
    box->addWidget(summary(keyboard,content));
    box->addWidget(untouched(disks,selected,content));
    return content;
}
