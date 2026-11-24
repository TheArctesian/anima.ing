#pragma once
#include <QJsonArray>
#include <QVariantList>
#include <QWidget>

QWidget* installationReview(const QJsonArray& disks, const QVariantList& selected, const QString& keyboard);
