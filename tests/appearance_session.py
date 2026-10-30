#!/usr/bin/env python3
"""Real GTK/Qt live updates on an isolated bus, display and XDG directories.

cargo build -p coconut-integration-appearance
cmake -S integrations/qt/plugin -B .build/qt6 -DBUILD_TESTING=ON
cmake --build .build/qt6
dbus-run-session -- xvfb-run -a /usr/bin/python3 tests/appearance_session.py
"""
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]


def wait_for(check, description, timeout=12):
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            result = check()
            if result:
                return result
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            last_error = error
        time.sleep(0.05)
    raise AssertionError(f"Timed out waiting for {description}: {last_error}")


def main():
    assert os.environ.get("DBUS_SESSION_BUS_ADDRESS"), "run under dbus-run-session"
    assert os.environ.get("DISPLAY"), "run under xvfb-run"
    with tempfile.TemporaryDirectory(prefix="coconut-appearance-") as directory:
        root = Path(directory)
        env = os.environ.copy()
        env.update(HOME=str(root), XDG_CONFIG_HOME=str(root / "config"),
                   XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"),
                   XDG_RUNTIME_DIR=str(root / "runtime"), XDG_CURRENT_DESKTOP="Coconut",
                   XDG_DESKTOP_PORTAL_DIR=str(root / "portals"), GSETTINGS_BACKEND="keyfile",
                   GIO_USE_VFS="local", NO_AT_BRIDGE="1",
                   QT_QPA_PLATFORM="xcb", QT_QPA_PLATFORMTHEME="xdgdesktopportal:coconut",
                   QT_PLUGIN_PATH=str(ROOT / ".build/qt6/plugins"),
                   CREAMUI_THEME="invalid-process-local-theme", CREAMUI_VARIANT="invalid")
        env.pop("WAYLAND_DISPLAY", None)
        env.pop("GTK_THEME", None)
        env.pop("GSETTINGS_SCHEMA_DIR", None)
        (root / "runtime").mkdir(mode=0o700)
        (root / "portals").mkdir()
        shutil.copy(ROOT / "integrations/appearance/data/coconut.portal", root / "portals")
        config = root / "config/coconut/appearance.toml"
        config.parent.mkdir(parents=True)
        gtk_config = root / "config/gtk-3.0/settings.ini"
        gtk_config.parent.mkdir(parents=True)
        gtk_config.write_text("# keep this\n[Settings]\ngtk-icon-theme-name=Adwaita\n")
        portal_config = root / "config/xdg-desktop-portal/coconut-portals.conf"
        portal_config.parent.mkdir(parents=True)
        portal_config.write_text("[preferred]\ndefault=none\norg.freedesktop.impl.portal.Settings=coconut\n")
        generated = root / "data/coconut/appearance"
        env["CREAMUI_APPEARANCE"] = str(generated / "cream.toml")
        env["COCONUT_QT_APPEARANCE"] = str(generated / "qt.json")
        subprocess.run(["dbus-update-activation-environment", "HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME",
                        "XDG_CACHE_HOME", "XDG_RUNTIME_DIR", "XDG_CURRENT_DESKTOP", "XDG_DESKTOP_PORTAL_DIR",
                        "GSETTINGS_BACKEND", "GIO_USE_VFS", "NO_AT_BRIDGE", "DISPLAY"], env=env, check=True)

        def select(variant, accent="#0099d3", font="Sans", theme="coconut"):
            temporary = config.with_suffix(".tmp")
            temporary.write_text(f'theme = "{theme}"\nvariant = "{variant}"\n[overrides]\naccent = "{accent}"\nfont_family = "{font}"\n')
            temporary.replace(config)

        def call(namespace="org.freedesktop.appearance", key="color-scheme", public=False):
            service = "org.freedesktop.portal.Desktop" if public else "org.freedesktop.impl.portal.desktop.coconut"
            interface = "org.freedesktop.portal.Settings" if public else "org.freedesktop.impl.portal.Settings"
            return subprocess.check_output(["gdbus", "call", "--session", "--dest", service,
                "--object-path", "/org/freedesktop/portal/desktop", "--method", interface + ".Read", namespace, key], env=env, text=True, stderr=subprocess.DEVNULL, timeout=4)

        children = []
        logs = []
        def start(args):
            log = tempfile.TemporaryFile(mode="w+")
            logs.append(log)
            child = subprocess.Popen(args, env=env, stdout=subprocess.PIPE, stderr=log, text=True, bufsize=1)
            children.append(child)
            return child

        try:
            select("light")
            bridge = start([str(ROOT / "target/debug/coconut-appearance")])
            wait_for(lambda: "uint32 2" in call(), "backend startup")
            wait_for(lambda: (generated / "cream.toml").is_file(), "CreamUI native export")
            broker = start(["/usr/libexec/xdg-desktop-portal", "--replace"])
            def broker_ready():
                return "true" in subprocess.check_output(["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
                    "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.NameHasOwner",
                    "org.freedesktop.portal.Desktop"], env=env, text=True, timeout=3)
            wait_for(broker_ready, "portal broker name")
            wait_for(lambda: "uint32 2" in call(public=True), "public Settings portal")
            assert "light" in call("org.creamui.appearance", "variant", public=True)

            monitor = start(["gdbus", "monitor", "--session", "--dest", "org.freedesktop.portal.Desktop",
                             "--object-path", "/org/freedesktop/portal/desktop"])
            signals = []
            threading.Thread(target=lambda: signals.extend(iter(monitor.stdout.readline, "")), daemon=True).start()

            probe = start([str(ROOT / ".build/qt6/coconut-qt-probe")])
            updates = queue.Queue()
            qt_seen = []
            def collect():
                for line in probe.stdout:
                    qt_seen.append(json.loads(line))
                    updates.put(qt_seen[-1])
            threading.Thread(target=collect, daemon=True).start()
            def qt_state(accent):
                while True:
                    try:
                        state = updates.get_nowait()
                    except queue.Empty:
                        return False
                    if state["accent"] == accent:
                        return state
            initial = wait_for(lambda: qt_state("#0099d3"), "Qt initial palette")
            assert initial["window"] != "#141414"

            # GtkSettings and the actual generated styles are consumed by a
            # GTK process, rather than checking only our generated strings.
            gtk_probe = start(["/usr/bin/python3", "-u", "-c", """
import gi, json
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, GLib
settings = Gtk.Settings.get_default()
window = Gtk.Window()
button = Gtk.Button(label='Probe')
button.get_style_context().add_class('suggested-action')
window.add(button)
window.show_all()
previous = None
def report():
    global previous
    name = settings.get_property('gtk-theme-name')
    bg = button.get_style_context().get_background_color(Gtk.StateFlags.NORMAL)
    state = {'theme': name, 'accent': '#%02x%02x%02x' % (round(bg.red * 255), round(bg.green * 255), round(bg.blue * 255))}
    if state != previous:
        print(json.dumps(state), flush=True)
        previous = state
    return True
GLib.timeout_add(50, report)
Gtk.main()
"""])
            gtk_updates = queue.Queue()
            def collect_gtk():
                for line in gtk_probe.stdout:
                    gtk_updates.put(json.loads(line))
            threading.Thread(target=collect_gtk, daemon=True).start()
            def gtk_state(accent):
                while not gtk_updates.empty():
                    state = gtk_updates.get_nowait()
                    if state["accent"] == accent:
                        return state
                return False
            first_gtk = wait_for(lambda: gtk_state("#0099d3"), "GTK startup")
            assert first_gtk["theme"].startswith("Coconut")
            assert first_gtk["accent"] == "#0099d3", first_gtk

            select("midnight", "#ff00ff", "Serif")
            wait_for(lambda: "uint32 1" in call(public=True), "public dark preference")
            wait_for(lambda: "midnight" in call("org.creamui.appearance", "variant", public=True), "public named variant")
            changed = wait_for(lambda: qt_state("#ff00ff"), "live Qt accent")
            assert changed["window"] != initial["window"]
            assert changed["font"] == "Serif", changed
            wait_for(lambda: "#ff00ff" in (generated / "cream.toml").read_text(), "native CreamUI change")
            second_gtk = wait_for(lambda: gtk_state("#ff00ff"), "live GTK theme change")
            assert first_gtk != second_gtk
            assert second_gtk["accent"] == "#ff00ff", second_gtk
            wait_for(lambda: any("SettingChanged" in line and "color-scheme" in line for line in signals), "public change signal")
            wait_for(lambda: any("SettingChanged" in line and "accent-color" in line for line in signals), "public accent signal")
            wait_for(lambda: any("SettingChanged" in line and "org.creamui.appearance" in line and "midnight" in line for line in signals), "public named variant signal")

            # An accent-only edit must also reload GTK and Qt.
            select("midnight", "#00ff00", "Serif")
            wait_for(lambda: qt_state("#00ff00"), "Qt accent-only change")
            third_gtk = wait_for(lambda: gtk_state("#00ff00"), "GTK accent-only change")
            assert third_gtk != second_gtk
            assert third_gtk["accent"] == "#00ff00", third_gtk

            before = (generated / "qt.json").read_bytes()
            config.write_text("not a valid theme selection = [")
            time.sleep(1.3)
            assert (generated / "qt.json").read_bytes() == before
            assert "uint32 1" in call(public=True)
            select("light")
            wait_for(lambda: "uint32 2" in call(public=True), "recovery after invalid selection")
            assert "# keep this" in gtk_config.read_text()
            assert "gtk-icon-theme-name=Adwaita" in gtk_config.read_text()

            # A disk-backed theme can replace GTK's entire stylesheet while
            # supplying only selected Qt roles and a dynamic Widgets sheet.
            custom = root / "data/coconut/themes/manual"
            shutil.copytree(ROOT / "themes/coconut", custom)
            (custom / "gtk").mkdir()
            (custom / "qt").mkdir()
            manifest = (custom / "theme.toml").read_text().replace('id = "coconut"', 'id = "manual"')
            manifest += "\n[adapters.gtk]\nbase = 'manual'\ngtk3 = 'gtk/gtk3.css.in'\ngtk4 = 'gtk/gtk4.css.in'\n"
            manifest += "\n[adapters.qt]\nbase = 'auto'\npalette = 'qt/palette.toml'\nstylesheet = 'qt/extra.qss.in'\n"
            (custom / "theme.toml").write_text(manifest)
            for major, resource in [(3, "Adwaita/gtk-contained-dark.css"), (4, "Default/gtk-dark.css")]:
                (custom / f"gtk/gtk{major}.css.in").write_text(
                    '@import url("resource:///org/gtk/libgtk/theme/' + resource + '");\n'
                    'button.suggested-action { background-image: none; background-color: ${tokens.colors.accent}; '
                    'color: ${tokens.colors.on_accent}; transition: none; }\n')
            (custom / "qt/palette.toml").write_text('[active]\nHighlight = "${tokens.colors.accent}"\nHighlightedText = "${tokens.colors.on_accent}"\n')
            sheet = custom / "qt/extra.qss.in"
            sheet.write_text('QPushButton { border-radius: ${tokens.geometry.control_radius}px; }')
            select("midnight", "#1256ab", "Serif", theme="manual")
            wait_for(lambda: qt_state("#1256ab"), "custom Qt role mapping")
            wait_for(lambda: gtk_state("#1256ab"), "manual dynamic GTK stylesheet")
            wait_for(lambda: any("border-radius: 10px" in s["stylesheet"] for s in qt_seen), "Qt stylesheet template")
            sheet.write_text('QPushButton { border-radius: 3px; color: ${tokens.colors.accent}; }')
            wait_for(lambda: any("border-radius: 3px" in s["stylesheet"] for s in qt_seen), "live native template edit")
            subprocess.run([str(ROOT / "target/debug/coconut-appearance"), "--check"], env=env, check=True)

            # The current generated CSS must parse in both native GTK versions.
            theme_name = (root / "config/coconut/gtk-theme").read_text().strip()
            for major in (3, 4):
                css = root / f"data/themes/{theme_name}/gtk-{major}.0/gtk.css"
                subprocess.run(["/usr/bin/python3", "-c", """
import gi, sys
gi.require_version('Gtk', sys.argv[1] + '.0')
from gi.repository import Gtk
provider = Gtk.CssProvider()
errors = []
provider.connect('parsing-error', lambda provider, section, error: errors.append(str(error)))
provider.load_from_path(sys.argv[2])
assert not errors, errors
""", str(major), str(css)], env=env, check=True, timeout=5)
            print("PASS: public portal, GTK live colors, Qt live palette/fonts, dynamic accent, native CreamUI export, manual/hybrid templates, invalid-config recovery")
        except Exception:
            print("Qt observations:", locals().get("qt_seen", []))
            for log in logs:
                log.seek(0)
                print(log.read())
            raise
        finally:
            for child in reversed(children):
                child.terminate()
            for child in reversed(children):
                try:
                    child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
            for log in logs:
                log.close()
            # The real broker activates the document portal, whose FUSE mount
            # outlives the broker. Unmount only this fixture's private path.
            subprocess.run(["fusermount3", "-u", str(root / "runtime/doc")], env=env,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


if __name__ == "__main__":
    main()
