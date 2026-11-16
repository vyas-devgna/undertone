#include "tuner.h"

#include <QColor>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QLockFile>
#include <QPalette>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QStandardPaths>
#include <QTextStream>

int main(int argc, char **argv) {
    const bool check = argc == 2 && QByteArray(argv[1]) == "--audio-check";
    if (check)
        qputenv("QT_QPA_PLATFORM", "offscreen");

    QGuiApplication app(argc, argv);
    app.setOrganizationName("Undertone");
    app.setApplicationName("Undertone");
    app.setDesktopFileName("undertone");

    QLockFile lock(QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation) + "/undertone.lock");
    if (!lock.tryLock()) {
        QTextStream(stderr) << "Undertone is already running.\n";
        return check ? 1 : 0;
    }

    Tuner tuner;
    if (check) {
        QObject::connect(&tuner, &Tuner::checkFinished, &app, [&](bool ok) {
            QTextStream(stdout) << tuner.message() << Qt::endl;
            app.exit(ok ? 0 : 1);
        });
        QTimer::singleShot(180, &tuner, &Tuner::checkAudio);
        return app.exec();
    }

    QQuickStyle::setStyle("Material");
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("tuner", &tuner);
    const QString monoFont = QFontDatabase::hasFamily("JetBrainsMono Nerd Font")
        ? QStringLiteral("JetBrainsMono Nerd Font")
        : QFontDatabase::systemFont(QFontDatabase::FixedFont).family();
    engine.rootContext()->setContextProperty("monoFont", monoFont);

    QPalette palette;
    palette.setColor(QPalette::Accent, QColor("#7daea3"));
    palette.setColor(QPalette::Window, QColor("#282828"));
    palette.setColor(QPalette::WindowText, QColor("#d4be98"));
    QGuiApplication::setPalette(palette);

    engine.load(QUrl("qrc:/Main.qml"));
    if (engine.rootObjects().isEmpty())
        return 1;
    return app.exec();
}
