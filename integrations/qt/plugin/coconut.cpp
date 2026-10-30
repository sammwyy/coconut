// SPDX-License-Identifier: Apache-2.0
// QPA plugins must be built against the Qt version shipped by the host.
#include <QApplication>
#include <QDBusConnection>
#include <QFile>
#include <QFileSystemWatcher>
#include <QFont>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMetaEnum>
#include <QPalette>
#include <QStandardPaths>
#include <QTimer>
#include <QDir>
#include <qpa/qplatformtheme.h>
#include <qpa/qplatformthemeplugin.h>
#include <qpa/qplatformthemefactory_p.h>
#include <qpa/qplatformintegration.h>
#include <private/qguiapplication_p.h>
#include <qpa/qwindowsysteminterface.h>
#include <memory>

class CoconutTheme final : public QObject, public QPlatformTheme {
    Q_OBJECT
public:
    CoconutTheme() {
        // The session can select Qt's xdgdesktopportal wrapper around this
        // plugin for native dialogs. Never construct that wrapper from here:
        // it discovers "coconut" through XDG_CURRENT_DESKTOP and would recurse.
        fallback.reset(QGuiApplicationPrivate::platformIntegration()->createPlatformTheme(QStringLiteral("generic")));
        if (!fallback) fallback.reset(new QPlatformTheme);
        const auto custom = qEnvironmentVariable("COCONUT_QT_APPEARANCE");
        source = custom.isEmpty() ? QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation)
            + QStringLiteral("/coconut/appearance/qt.json") : custom;
        connect(&watcher, &QFileSystemWatcher::fileChanged, this, &CoconutTheme::scheduleReload);
        connect(&watcher, &QFileSystemWatcher::directoryChanged, this, &CoconutTheme::scheduleReload);
        debounce.setSingleShot(true);
        debounce.setInterval(80);
        connect(&debounce, &QTimer::timeout, this, [this] { reload(); });
        QDBusConnection::sessionBus().connect(QStringLiteral("org.freedesktop.impl.portal.desktop.coconut"),
            QStringLiteral("/org/coconut/Appearance"), QStringLiteral("org.coconut.Appearance"),
            QStringLiteral("Changed"), this, SLOT(scheduleReload()));
        // Covers startup before the directory exists and atomic replacements.
        poll.setInterval(2000);
        connect(&poll, &QTimer::timeout, this, [this] { reload(); });
        poll.start();
        reload(false);
        QTimer::singleShot(0, this, [this] { applyStylesheet(); });
    }

    const QPalette *palette(Palette type = SystemPalette) const override {
        return ready ? &colors : fallback->palette(type);
    }
    const QFont *font(Font type = SystemFont) const override {
        if (!ready || type == FixedFont) return fallback->font(type);
        return &systemFont;
    }
    QVariant themeHint(ThemeHint hint) const override {
        if (ready && hint == StyleNames) return QStringList{QStringLiteral("Fusion")};
        return fallback->themeHint(hint);
    }
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
    Qt::ColorScheme colorScheme() const override {
        return ready ? (dark ? Qt::ColorScheme::Dark : Qt::ColorScheme::Light) : fallback->colorScheme();
    }
#endif
#if QT_VERSION >= QT_VERSION_CHECK(6, 10, 0)
    Qt::ContrastPreference contrastPreference() const override {
        return ready && highContrast ? Qt::ContrastPreference::HighContrast : fallback->contrastPreference();
    }
#endif
    bool usePlatformNativeDialog(DialogType type) const override { return fallback->usePlatformNativeDialog(type); }
    QPlatformDialogHelper *createPlatformDialogHelper(DialogType type) const override { return fallback->createPlatformDialogHelper(type); }
    QPlatformMenuItem *createPlatformMenuItem() const override { return fallback->createPlatformMenuItem(); }
    QPlatformMenu *createPlatformMenu() const override { return fallback->createPlatformMenu(); }
    QPlatformMenuBar *createPlatformMenuBar() const override { return fallback->createPlatformMenuBar(); }
#ifndef QT_NO_SYSTEMTRAYICON
    QPlatformSystemTrayIcon *createPlatformSystemTrayIcon() const override { return fallback->createPlatformSystemTrayIcon(); }
#endif
    QIconEngine *createIconEngine(const QString &name) const override { return fallback->createIconEngine(name); }
    QIcon fileIcon(const QFileInfo &info, IconOptions options = {}) const override { return fallback->fileIcon(info, options); }
#if QT_CONFIG(shortcut)
    QList<QKeySequence> keyBindings(QKeySequence::StandardKey key) const override { return fallback->keyBindings(key); }
#endif

public slots:
    void scheduleReload() { debounce.start(); }
    void reload() { reload(true); }

private:
    void reload(bool notify) {
        const auto directory = QFileInfo(source).absolutePath();
        if (QDir(directory).exists() && !watcher.directories().contains(directory)) watcher.addPath(directory);
        if (QFile::exists(source) && !watcher.files().contains(source)) watcher.addPath(source);
        QFile file(source);
        if (!file.open(QIODevice::ReadOnly)) return;
        const auto bytes = file.readAll();
        if (bytes == lastSource) return;
        const auto document = QJsonDocument::fromJson(bytes);
        if (!document.isObject()) { qWarning("Coconut: invalid appearance JSON"); return; }
        const auto object = document.object();
        const auto scheme = object.value(QStringLiteral("color_scheme")).toString();
        const auto family = object.value(QStringLiteral("font_family")).toString();
        const auto pointSize = object.value(QStringLiteral("font_point_size")).toDouble();
        if (object.value(QStringLiteral("schema")).toInt() != 1 || family.isEmpty() || pointSize <= 0
            || (scheme != QStringLiteral("light") && scheme != QStringLiteral("dark"))) return;
        QPalette next;
        const auto groups = object.value(QStringLiteral("palette")).toObject();
        const auto roles = QMetaEnum::fromType<QPalette::ColorRole>();
        for (const auto group : {QPalette::Active, QPalette::Inactive, QPalette::Disabled}) {
            const auto name = group == QPalette::Active ? "Active" : group == QPalette::Inactive ? "Inactive" : "Disabled";
            const auto entries = groups.value(QLatin1String(name)).toObject();
            if (entries.isEmpty()) return;
            for (auto entry = entries.begin(); entry != entries.end(); ++entry) {
                bool known = false;
                const auto role = roles.keyToValue(entry.key().toLatin1().constData(), &known);
                // Accent was added in Qt 6.6; older builds use Highlight.
                if (!known && entry.key() == QStringLiteral("Accent")) continue;
                const QColor value(entry.value().toString());
                if (!known || !value.isValid()) return;
                next.setColor(group, static_cast<QPalette::ColorRole>(role), value);
            }
        }
        colors = next;
        systemFont = QFont(family);
        systemFont.setPointSizeF(pointSize);
        dark = scheme == QStringLiteral("dark");
        highContrast = object.value(QStringLiteral("high_contrast")).toBool();
        stylesheet = object.value(QStringLiteral("stylesheet")).toString();
        lastSource = bytes;
        ready = true;
        if (notify) {
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
            QWindowSystemInterface::handleThemeChange();
#else
            QWindowSystemInterface::handleThemeChange(nullptr);
#endif
            applyStylesheet();
        }
    }
    void applyStylesheet() {
        auto *app = qobject_cast<QApplication *>(QCoreApplication::instance());
        if (app && ready && (app->styleSheet().isEmpty() || app->styleSheet() == appliedStylesheet)) {
            if (app->styleSheet() != stylesheet) app->setStyleSheet(stylesheet);
            appliedStylesheet = stylesheet;
        }
    }
    std::unique_ptr<QPlatformTheme> fallback;
    QFileSystemWatcher watcher;
    QTimer debounce, poll;
    QString source, stylesheet, appliedStylesheet;
    QByteArray lastSource;
    QPalette colors;
    QFont systemFont;
    bool ready = false, dark = false, highContrast = false;
};

class CoconutPlugin final : public QPlatformThemePlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QPlatformThemeFactoryInterface_iid FILE "coconut.json")
public:
    QPlatformTheme *create(const QString &key, const QStringList &) override {
        return key.compare(QStringLiteral("coconut"), Qt::CaseInsensitive) == 0 ? new CoconutTheme : nullptr;
    }
};

#include "coconut.moc"
