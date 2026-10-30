#!/usr/bin/env python3
"""Exercise staged session activation, shutdown, and build options."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[1]
BINARIES = (
    "coconut", "coconut-bg", "coconut-greeter", "coconut-settings",
    "coconut-appearance", "cterm", "cfiles", "cmonitor", "cplayer", "cedit",
    "ccalc", "cviewer", "coconut-screenshots", "coconut-permission-portal",
)


def executable(path, content):
    path.write_text(content)
    path.chmod(0o755)


def wait_for(check):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        result = check()
        if result:
            return result
        time.sleep(0.05)
    raise AssertionError("development session did not reach the expected state")


COMPONENT = r'''#!/usr/bin/python3
import json
import os
from pathlib import Path
import sys
import time
from gi.repository import Gio, GLib

name = Path(sys.argv[0]).name
run = Path(os.environ["COCONUT_DEV_SESSION_DIR"])
if "--once" in sys.argv:
    sys.exit(0)
(run / (name + ".pid")).write_text(str(os.getpid()))
(run / (name + ".json")).write_text(json.dumps(dict(os.environ)))
if name == "coconut":
    if os.environ.get("COCONUT_TEST_HOLD"):
        while True:
            time.sleep(1)
    time.sleep(1)
    sys.exit(int(os.environ.get("COCONUT_TEST_STATUS", "0")))
names = {
    "coconut-appearance": "org.freedesktop.impl.portal.desktop.coconut",
    "coconut-permission-portal": "org.coconut.Permissions",
    "cfiles": "org.freedesktop.impl.portal.desktop.cfiles",
    "broker": "org.freedesktop.portal.Desktop",
}
if name in names:
    Gio.bus_own_name(Gio.BusType.SESSION, names[name], Gio.BusNameOwnerFlags.NONE,
                     None, None, None)
GLib.MainLoop().run()
'''

BLAIR = r'''#!/usr/bin/python3
import json
import os
from pathlib import Path
import subprocess
import sys
import time

run = Path(os.environ["COCONUT_DEV_SESSION_DIR"])
(run / "blair.pid").write_text(str(os.getpid()))
(run / "blair-arguments.json").write_text(json.dumps(sys.argv[1:]))
env = dict(os.environ, WAYLAND_DISPLAY="dev-test")
subprocess.Popen(["/bin/sh", "-c", sys.argv[sys.argv.index("--run") + 1]], env=env)
while True:
    time.sleep(1)
'''


@unittest.skipUnless(all(shutil.which(command) for command in (
    "dbus-run-session", "gdbus", "dbus-update-activation-environment")), "D-Bus tools required")
class DevSessionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(["/usr/bin/python3", "-c", "from gi.repository import Gio"],
                                capture_output=True)
        if result.returncode:
            raise unittest.SkipTest("system Python with GObject introspection required")

    def setUp(self):
        self.fixture = tempfile.TemporaryDirectory(prefix="coconut dev ")
        self.addCleanup(self.fixture.cleanup)
        self.build = Path(self.fixture.name) / "staged binaries"
        self.build.mkdir()
        for binary in BINARIES:
            executable(self.build / binary, COMPONENT)
        plugins = self.build / "qt/plugins/platformthemes"
        plugins.mkdir(parents=True)
        (plugins / "libcoconut-qt6.so").touch()
        executable(self.build / "blair", BLAIR)
        executable(self.build / "broker", COMPONENT)
        self.env = os.environ.copy()
        for name in ("DISPLAY", "WAYLAND_DISPLAY", "COCONUT_DEV_SESSION_DIR"):
            self.env.pop(name, None)
        self.env.update(COCONUT_BUILD_DIR=str(self.build),
                        COCONUT_DEV_BLAIR=str(self.build / "blair"),
                        COCONUT_DEV_PORTAL=str(self.build / "broker"))

    def run_directory(self):
        directories = list((self.build / "dev").glob("session-*"))
        return directories[0] if directories else None

    def assert_stopped(self, run):
        for path in run.glob("*.pid"):
            pid = int(path.read_text())
            def stopped():
                try:
                    state = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1][0]
                    return state == "Z"
                except FileNotFoundError:
                    return True
            wait_for(stopped)

    def test_private_bus_uses_staged_services_and_returns_shell_status(self):
        self.env["COCONUT_TEST_STATUS"] = "7"
        result = subprocess.run(["bash", str(ROOT / "dev.sh"), "--", "--profile", "test profile"],
                                env=self.env, capture_output=True, text=True, timeout=25)
        self.assertEqual(result.returncode, 7, result.stdout + result.stderr)
        run = self.run_directory()
        self.assertTrue((run / "ready").exists())
        self.assertEqual(json.loads((run / "blair-arguments.json").read_text())[:2],
                         ["--profile", "test profile"])
        for name in ("coconut", "coconut-bg", "coconut-appearance", "cfiles",
                     "coconut-permission-portal", "broker"):
            env = json.loads((run / f"{name}.json").read_text())
            self.assertEqual(env["PATH"].split(":")[0], str(self.build))
            self.assertEqual(env["WAYLAND_DISPLAY"], "dev-test")
            self.assertEqual(env["XDG_RUNTIME_DIR"], str(run / "runtime"))
            self.assertNotEqual(env["DBUS_SESSION_BUS_ADDRESS"],
                                self.env.get("DBUS_SESSION_BUS_ADDRESS"))
            self.assertFalse(env.get("DISPLAY"))
        self.assert_stopped(run)

    def test_interrupt_stops_compositor_and_services(self):
        self.env["COCONUT_TEST_HOLD"] = "1"
        child = subprocess.Popen(["bash", str(ROOT / "dev.sh")], env=self.env,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            run = wait_for(self.run_directory)
            wait_for(lambda: (run / "ready").exists())
            child.terminate()
            stdout, stderr = child.communicate(timeout=20)
            self.assertEqual(child.returncode, 130, stdout + stderr)
            self.assert_stopped(run)
        finally:
            if child.poll() is None:
                child.kill()
                child.communicate()

    def test_build_option_forwards_no_upx(self):
        commands = Path(self.fixture.name) / "commands"
        commands.mkdir()
        record = Path(self.fixture.name) / "build-arguments"
        executable(commands / "bash", '#!/bin/sh\nprintf "%s\\n" "$@" > "$COCONUT_TEST_BUILD_ARGS"\n')
        self.env.update(PATH=f"{commands}:{os.environ['PATH']}", COCONUT_TEST_BUILD_ARGS=str(record))
        result = subprocess.run(["/bin/bash", str(ROOT / "dev.sh"), "--build"], env=self.env,
                                capture_output=True, text=True, timeout=25)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(record.read_text().splitlines(), [str(ROOT / "build.sh"), "--no-upx"])
        self.assert_stopped(self.run_directory())


class BuildTests(unittest.TestCase):
    def test_no_upx_stages_uncompressed_binaries_even_when_upx_is_available(self):
        with tempfile.TemporaryDirectory(prefix="coconut build ") as directory:
            root = Path(directory)
            commands = root / "commands"
            commands.mkdir()
            target = root / "target/release"
            target.mkdir(parents=True)
            for binary in BINARIES:
                executable(target / binary, f"uncompressed {binary}\n")
            executable(commands / "cargo", "#!/bin/sh\nexit 0\n")
            executable(commands / "cmake", '''#!/usr/bin/python3
from pathlib import Path
import sys
if sys.argv[1] == "--build":
    plugins = Path(sys.argv[2]) / "plugins/platformthemes"
    plugins.mkdir(parents=True)
    (plugins / "libcoconut-qt6.so").write_text("plugin")
''')
            record = root / "upx-called"
            executable(commands / "upx", '#!/bin/sh\ntouch "$COCONUT_TEST_UPX_RECORD"\nexit 1\n')
            staged = root / "staged"
            env = dict(os.environ, PATH=f"{commands}:{os.environ['PATH']}",
                       COCONUT_BUILD_DIR=str(staged), CARGO_TARGET_DIR=str(target.parent),
                       COCONUT_TEST_UPX_RECORD=str(record), COCONUT_QT_MAJORS="6")
            result = subprocess.run(["bash", str(ROOT / "build.sh"), "--no-upx"], env=env,
                                    capture_output=True, text=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertFalse(record.exists())
            for binary in BINARIES:
                self.assertEqual((target / binary).read_bytes(), (staged / binary).read_bytes())


if __name__ == "__main__":
    unittest.main()
