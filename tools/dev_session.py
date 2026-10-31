#!/usr/bin/env python3
"""Run staged Coconut components with private D-Bus activation and XDG paths."""

import argparse
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
BINARIES = (
    "coconut", "coconut-bg", "coconut-greeter", "coconut-settings",
    "coconut-appearance", "cterm", "cfiles", "cmonitor", "cplayer", "cedit",
    "ccalc", "cviewer", "coconut-screenshots", "coconut-permission-portal", "coconut-clipboard",
)
SERVICES = (
    ("integrations/appearance/data", "org.freedesktop.impl.portal.desktop.coconut",
     "coconut.portal", ("coconut-appearance",)),
    ("apps/files/data", "org.freedesktop.impl.portal.desktop.cfiles",
     "cfiles.portal", ("cfiles", "--portal")),
    ("apps/permission-portal/data", "org.coconut.Permissions",
     "coconut-permissions.portal", ("coconut-permission-portal", "--service")),
    ("integrations/clipboard/data", "org.coconut.Clipboard", None, ("coconut-clipboard",)),
)


def executable(path):
    return path.is_file() and os.access(path, os.X_OK)


def dbus_quote(value):
    return '"' + str(value).replace("\\", "\\\\").replace('"', '\\"') + '"'


def find_executable(override, candidates, description):
    paths = [Path(override).expanduser()] if override else candidates
    for path in paths:
        if executable(path):
            return path.resolve()
    raise RuntimeError(f"{description} not found; checked {', '.join(map(str, paths))}")


def prepare(build):
    missing = [name for name in BINARIES if not executable(build / name)]
    if missing:
        raise RuntimeError(f"missing staged binaries: {', '.join(missing)}; run ./scripts/dev.sh --build")
    if not list((build / "qt/plugins/platformthemes").glob("libcoconut-qt*.so")):
        raise RuntimeError("staged Qt plugin missing; run ./scripts/dev.sh --build")
    for command in ("dbus-run-session", "dbus-update-activation-environment", "gdbus"):
        if not shutil.which(command):
            raise RuntimeError(f"{command} is required for the development session")

    blair = find_executable(os.environ.get("COCONUT_DEV_BLAIR"), [
        build / "blair", ROOT.parent / "blair/.build/blair",
        ROOT.parent / "blair/target/release/blair",
        Path(shutil.which("blair") or "/nonexistent/blair"),
    ], "Blair (COCONUT_DEV_BLAIR)")
    broker = find_executable(os.environ.get("COCONUT_DEV_PORTAL"), [
        Path(shutil.which("xdg-desktop-portal") or "/nonexistent/xdg-desktop-portal"),
        Path("/usr/libexec/xdg-desktop-portal"), Path("/usr/lib/xdg-desktop-portal"),
    ], "xdg-desktop-portal (COCONUT_DEV_PORTAL)")

    (build / "dev").mkdir(exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix="session-", dir=build / "dev"))
    env = os.environ.copy()
    original_data = env.get("XDG_DATA_HOME", str(Path.home() / ".local/share"))
    for kind in ("CONFIG", "DATA", "STATE", "CACHE"):
        path = run / kind.lower()
        path.mkdir()
        env[f"XDG_{kind}_HOME"] = str(path)
    runtime = env.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
    if env.get("WAYLAND_DISPLAY") and not Path(env["WAYLAND_DISPLAY"]).is_absolute():
        env["WAYLAND_DISPLAY"] = str(Path(runtime) / env["WAYLAND_DISPLAY"])
    (run / "runtime").mkdir(mode=0o700)
    env["XDG_RUNTIME_DIR"] = str(run / "runtime")
    env.pop("WAYLAND_SOCKET", None)
    if (Path(runtime) / "pipewire-0").exists():
        env.setdefault("PIPEWIRE_REMOTE", str(Path(runtime) / "pipewire-0"))
    if (Path(runtime) / "pulse/native").exists():
        env.setdefault("PULSE_SERVER", f"unix:{runtime}/pulse/native")
    env.update(
        PATH=f"{build}:{env.get('PATH', os.defpath)}",
        XDG_DATA_DIRS=f"{original_data}:{env.get('XDG_DATA_DIRS', '/usr/local/share:/usr/share')}",
        XDG_CURRENT_DESKTOP="Coconut", XDG_SESSION_DESKTOP="Coconut",
        XDG_SESSION_TYPE="wayland", COCONUT_DEV_SESSION_DIR=str(run),
        COCONUT_DEV_BLAIR=str(blair), COCONUT_DEV_PORTAL=str(broker),
        CREAMUI_APPEARANCE=str(run / "data/coconut/appearance/cream.toml"),
        COCONUT_QT_APPEARANCE=str(run / "data/coconut/appearance/qt.json"),
        QT_QPA_PLATFORMTHEME="xdgdesktopportal:coconut",
        QT_PLUGIN_PATH=f"{build / 'qt/plugins'}:{env.get('QT_PLUGIN_PATH', '')}".rstrip(":"),
        GSETTINGS_BACKEND="keyfile",
        RUST_LOG=env.get("RUST_LOG", "warn"), RUST_BACKTRACE=env.get("RUST_BACKTRACE", "1"),
    )
    data = run / "data"
    services = data / "dbus-1/services"
    portals = data / "xdg-desktop-portal/portals"
    for path in (services, portals, data / "applications", data / "coconut", data / "fonts"):
        path.mkdir(parents=True, exist_ok=True)
    (data / "coconut/themes").symlink_to(ROOT / "themes", target_is_directory=True)
    (data / "fonts/coconut").symlink_to(
        ROOT / "assets/fonts/plus-jakarta-sans", target_is_directory=True)
    activation = [sys.executable, str(Path(__file__).resolve()), "--phase", "activate",
                  "--build-dir", str(build), "--"]
    for directory, name, portal, command in SERVICES:
        source = ROOT / directory
        definition = (source / f"{name}.service").read_text()
        command = [str(build / command[0]), *command[1:]]
        definition = "\n".join(
            "Exec=" + " ".join(map(dbus_quote, activation + command))
            if line.startswith("Exec=") else line
            for line in definition.splitlines() if not line.startswith("SystemdService="))
        (services / f"{name}.service").write_text(definition + "\n")
        if portal:
            shutil.copyfile(source / portal, portals / portal)
    (services / "org.freedesktop.portal.Desktop.service").write_text(
        "[D-BUS Service]\nName=org.freedesktop.portal.Desktop\nExec="
        + " ".join(map(dbus_quote, activation + [str(broker)])) + "\n")
    for desktop in (ROOT / "apps").glob("*/data/*.desktop"):
        shutil.copyfile(desktop, data / "applications" / desktop.name)
    for directory in env["XDG_DATA_DIRS"].split(":"):
        for portal in (Path(directory) / "xdg-desktop-portal/portals").glob("*.portal"):
            if not (portals / portal.name).exists():
                (portals / portal.name).symlink_to(portal)
    env["XDG_DESKTOP_PORTAL_DIR"] = str(portals)
    config = run / "config/xdg-desktop-portal"
    config.mkdir()
    shutil.copyfile(ROOT / "integrations/appearance/data/coconut-portals.conf",
                    config / "coconut-portals.conf")
    shutil.copyfile(config / "coconut-portals.conf", config / "portals.conf")
    shutil.copyfile(config / "coconut-portals.conf", config / "blair-portals.conf")
    # Older brokers restrict configuration lookup to this override directory.
    shutil.copyfile(config / "coconut-portals.conf", portals / "portals.conf")
    if env.get("WAYLAND_DISPLAY") or env.get("DISPLAY"):
        config = run / "config/blair"
        config.mkdir()
        (config / "config.toml").write_text('[general]\nbackend = "winit"\n')
    return run, env


class Processes:
    def __init__(self):
        self.children = []

    def start(self, command, log=None, env=None):
        output = log.open("ab") if log else None
        try:
            child = subprocess.Popen(command, env=env, stdout=output, stderr=output,
                                     start_new_session=True)
        finally:
            if output:
                output.close()
        self.children.append(child)
        return child

    def close(self):
        for child in reversed(self.children):
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                continue
        deadline = time.monotonic() + 3
        for child in reversed(self.children):
            try:
                child.wait(timeout=max(0, deadline - time.monotonic()))
            except subprocess.TimeoutExpired:
                pass
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass


def name_has_owner(name):
    result = subprocess.run([
        "gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
        "--object-path", "/org/freedesktop/DBus", "--method",
        "org.freedesktop.DBus.NameHasOwner", name,
    ], capture_output=True, text=True, timeout=2)
    return result.returncode == 0 and "true" in result.stdout


def start_service(name):
    result = subprocess.run([
        "gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
        "--object-path", "/org/freedesktop/DBus", "--method",
        "org.freedesktop.DBus.StartServiceByName", name, "0",
    ], capture_output=True, text=True, timeout=30)
    if result.returncode:
        raise RuntimeError(f"could not start {name}: {result.stderr.strip()}")
    if not name_has_owner(name):
        raise RuntimeError(f"{name} did not acquire its bus name")


def stop_activated(name):
    if not name_has_owner(name):
        return
    result = subprocess.run([
        "gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
        "--object-path", "/org/freedesktop/DBus", "--method",
        "org.freedesktop.DBus.GetConnectionUnixProcessID", name,
    ], capture_output=True, text=True, check=True, timeout=2)
    pid = int(result.stdout.split("uint32 ")[1].split(",")[0].rstrip(")"))
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    deadline = time.monotonic() + 5
    while name_has_owner(name):
        if time.monotonic() > deadline:
            raise RuntimeError(f"could not stop early activation of {name}")
        time.sleep(0.1)


def session(build, run):
    processes = Processes()
    status = 1
    services = [entry[1] for entry in SERVICES] + ["org.freedesktop.portal.Desktop"]
    try:
        (run / "session-pid").write_text(str(os.getpid()))
        os.environ.pop("DISPLAY", None)
        os.environ.update(QT_QPA_PLATFORM="wayland", GDK_BACKEND="wayland",
                          XDG_CURRENT_DESKTOP="Coconut", XDG_SESSION_DESKTOP="Coconut")
        variables = [name for name in os.environ if name.startswith((
            "XDG_", "COCONUT_", "CREAMUI_", "QT_", "RUST_"))]
        subprocess.run(["dbus-update-activation-environment", "PATH", "DISPLAY=",
                        "WAYLAND_DISPLAY", "GSETTINGS_BACKEND", "GDK_BACKEND", *variables], check=True)
        exported = variables + ["PATH", "DBUS_SESSION_BUS_ADDRESS", "WAYLAND_DISPLAY",
                                "GSETTINGS_BACKEND", "GDK_BACKEND", "PIPEWIRE_REMOTE", "PULSE_SERVER"]
        (run / "environment").write_text("unset DISPLAY WAYLAND_SOCKET\n" + "\n".join(
            f"export {name}={shlex.quote(os.environ[name])}"
            for name in sorted(set(exported)) if name in os.environ) + "\n")
        # Winit can activate portals before Blair publishes its Wayland display.
        for name in ("org.freedesktop.portal.Desktop", *(entry[1] for entry in SERVICES),
                     "org.freedesktop.impl.portal.desktop.gtk"):
            stop_activated(name)
        subprocess.run([str(build / "coconut-appearance"), "--once"], check=True)
        for name in services:
            start_service(name)
        processes.start([str(build / "coconut-bg")], run / "coconut-bg.log")
        shell = processes.start([str(build / "coconut")], run / "coconut.log")
        (run / "ready").touch()
        while shell.poll() is None:
            for child in processes.children[:-1]:
                if child.poll() is not None:
                    raise RuntimeError(
                        f"session component {child.args[0]} exited with status {child.returncode}")
            for name in services:
                if not name_has_owner(name):
                    raise RuntimeError(f"session service {name} lost its bus name")
            time.sleep(1)
        status = shell.returncode
        return status
    finally:
        processes.close()
        for name in reversed(services):
            try:
                stop_activated(name)
            except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                print(f"dev.sh: service cleanup: {error}", file=sys.stderr)
        (run / "session-exit").write_text(str(status))


def compositor(build, run, arguments):
    processes = Processes()
    try:
        (run / "compositor-pid").write_text(str(os.getpid()))
        hook = "exec " + shlex.join([sys.executable, str(Path(__file__).resolve()),
                                    "--phase", "session", "--build-dir", str(build)])
        child = processes.start([os.environ["COCONUT_DEV_BLAIR"], *arguments, "--run", hook],
                                run / "blair.log")
        deadline = time.monotonic() + 90
        ready = False
        while child.poll() is None:
            if (run / "session-exit").exists():
                return int((run / "session-exit").read_text())
            if not ready and (run / "ready").exists():
                print("Coconut development session ready (private D-Bus and staged portals).", flush=True)
                ready = True
            if not ready and time.monotonic() > deadline:
                raise RuntimeError(f"session startup timed out; see {run}")
            time.sleep(0.2)
        return child.returncode
    finally:
        try:
            if (run / "session-pid").exists() and not (run / "session-exit").exists():
                try:
                    os.kill(int((run / "session-pid").read_text()), signal.SIGTERM)
                except ProcessLookupError:
                    pass
                deadline = time.monotonic() + 5
                while not (run / "session-exit").exists() and time.monotonic() < deadline:
                    time.sleep(0.1)
        finally:
            processes.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-dir", required=True, type=Path)
    parser.add_argument("--phase", choices=("launch", "compositor", "session", "activate"),
                        default="launch", help=argparse.SUPPRESS)
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    arguments = args.arguments[1:] if args.arguments[:1] == ["--"] else args.arguments
    build = args.build_dir.expanduser().resolve()
    if args.phase != "launch":
        run = Path(os.environ["COCONUT_DEV_SESSION_DIR"])
        if args.phase == "activate":
            os.environ.pop("DISPLAY", None)
            with (run / f"{Path(arguments[0]).name}.log").open("ab") as output:
                os.dup2(output.fileno(), 1)
                os.dup2(output.fileno(), 2)
            os.execvp(arguments[0], arguments)
        if args.phase == "session":
            return session(build, run)
        return compositor(build, run, arguments)
    run, env = prepare(build)
    print(f"Staged binaries: {build}\nBlair: {env['COCONUT_DEV_BLAIR']}\n"
          f"Session files and logs: {run}", flush=True)
    processes = Processes()
    child = None
    try:
        child = processes.start([
            "dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()),
            "--phase", "compositor", "--build-dir", str(build), "--", *arguments,
        ], env=env)
        return child.wait()
    finally:
        if child is not None and child.poll() is None and (run / "compositor-pid").exists():
            try:
                os.kill(int((run / "compositor-pid").read_text()), signal.SIGTERM)
                child.wait(timeout=12)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                pass
        processes.close()


def interrupt(_signum, _frame):
    raise KeyboardInterrupt


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, interrupt)
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"dev.sh: {error}", file=sys.stderr)
        sys.exit(1)
