#include "tuner.h"

#include <QColor>
#include <QDir>
#include <QFile>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QLockFile>
#include <QPalette>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QTextStream>

static void followOmarchyTheme(QTimer &poll) {
    QPalette fallback;
    fallback.setColor(QPalette::Accent, QColor("#7daea3"));
    fallback.setColor(QPalette::Window, QColor("#282828"));
    fallback.setColor(QPalette::WindowText, QColor("#d4be98"));
    // QPalette has no success role; Link carries the theme's green for the in-tune state.
    fallback.setColor(QPalette::Link, QColor("#a9b665"));
    QGuiApplication::setPalette(fallback);

    const auto state = qEnvironmentVariableIsEmpty("XDG_STATE_HOME")
        ? QDir::homePath() + "/.local/state" : qEnvironmentVariable("XDG_STATE_HOME");
    const auto omarchy = state + "/omarchy";
    if (!QDir(omarchy).exists())
        return;

    const auto reload = [fallback, path = omarchy + "/current/theme/colors.toml"] {
        QPalette palette = fallback;
        QFile file(path);
        if (file.open(QIODevice::ReadOnly)) {
            static const QRegularExpression color(
                R"re(^\s*(accent|background|foreground|green)\s*=\s*"(#[0-9a-fA-F]{6})")re",
                QRegularExpression::MultilineOption);
            auto values = color.globalMatch(QString::fromUtf8(file.read(8192)));
            while (values.hasNext()) {
                const auto match = values.next();
                const auto key = match.captured(1);
                const auto role = key == "accent" ? QPalette::Accent
                    : key == "green" ? QPalette::Link
                    : key == "background" ? QPalette::Window : QPalette::WindowText;
                palette.setColor(role, QColor(match.captured(2)));
            }
        }
        if (palette != QGuiApplication::palette())
            QGuiApplication::setPalette(palette);
    };
    // The active theme can be replaced by swapping its symlink.
    reload();
    QObject::connect(&poll, &QTimer::timeout, &poll, reload);
    poll.start(1000);
}

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

    QTimer themePoll;
    followOmarchyTheme(themePoll);

    engine.load(QUrl("qrc:/Main.qml"));
    if (engine.rootObjects().isEmpty())
        return 1;
    return app.exec();
}
