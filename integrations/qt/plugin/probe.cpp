// A real QApplication used by the isolated session tests.
#include <QApplication>
#include <QFont>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPalette>
#include <QTimer>
#include <iostream>

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    QByteArray previous;
    auto report = [&] {
        QJsonObject values;
        values["window"] = app.palette().color(QPalette::Window).name();
        values["accent"] = app.palette().color(QPalette::Highlight).name();
        values["font"] = app.font().family();
        values["font_size"] = app.font().pointSizeF();
        values["stylesheet"] = app.styleSheet();
        auto json = QJsonDocument(values).toJson(QJsonDocument::Compact);
        if (json != previous) { std::cout << json.constData() << std::endl; previous = json; }
    };
    QTimer timer;
    QObject::connect(&timer, &QTimer::timeout, &app, report);
    timer.start(50);
    QTimer::singleShot(0, &app, report);
    QTimer::singleShot(60000, &app, &QApplication::quit);
    return app.exec();
}
