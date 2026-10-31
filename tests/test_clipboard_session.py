"""Opt-in clipboard workflow on a private bus and nested Blair compositor."""

import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(os.environ.get("COCONUT_CLIPBOARD_TEST_BLAIR"),
                     "set COCONUT_CLIPBOARD_TEST_BLAIR to a Blair executable")
class ClipboardSession(unittest.TestCase):
    def start(self, command, env=None):
        log = (self.root / f"process-{len(self.children)}.log").open("wb")
        try:
            child = subprocess.Popen(command, env=env or self.env, stdout=log,
                                     stderr=log, start_new_session=True)
        finally:
            log.close()
        self.children.append(child)
        return child

    def wait_for(self, callback):
        deadline = time.monotonic() + 12
        last = None
        while time.monotonic() < deadline:
            try:
                last = callback()
                if last:
                    return last
            except (subprocess.SubprocessError, ValueError):
                pass
            time.sleep(0.03)
        logs = "\n".join(path.read_text(errors="replace")
                         for path in self.root.glob("process-*.log"))
        self.fail(f"clipboard session did not become ready: {last}\n{logs}")

    def setUp(self):
        from gi.repository import GLib
        self.GLib = GLib
        self.fixture = tempfile.TemporaryDirectory(prefix="coconut-clipboard-test-")
        self.addCleanup(self.fixture.cleanup)
        self.root = Path(self.fixture.name)
        self.children = []
        self.addCleanup(self.stop)
        self.env = os.environ.copy()
        build = Path(os.environ.get("COCONUT_CLIPBOARD_TEST_BUILD", ROOT / "target/debug"))
        for name in ("coconut-clipboard", "coconut-permission-portal"):
            shutil.copy2(build / name, self.root / name)
        shutil.copy2(shutil.which("gdbus"), self.root / "coconut")
        (self.root / "coconut-settings").hardlink_to(self.root / "coconut")
        for name in ("config", "data", "runtime"):
            (self.root / name).mkdir(mode=0o700)
        parent_runtime = self.env.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
        if self.env.get("WAYLAND_DISPLAY") and not Path(self.env["WAYLAND_DISPLAY"]).is_absolute():
            self.env["WAYLAND_DISPLAY"] = str(Path(parent_runtime) / self.env["WAYLAND_DISPLAY"])
        self.env.update(XDG_CONFIG_HOME=str(self.root / "config"),
                        XDG_DATA_HOME=str(self.root / "data"),
                        XDG_RUNTIME_DIR=str(self.root / "runtime"))
        self.env.pop("WAYLAND_SOCKET", None)
        (self.root / "config/blair").mkdir()
        (self.root / "config/blair/config.toml").write_text('[general]\nbackend = "winit"\n')
        (self.root / "config/coconut").mkdir()
        (self.root / "config/coconut/clipboard.toml").write_text(
            "max_entries = 3\nmax_item_bytes = 128\nmax_total_bytes = 1024\n")
        (self.root / "data/coconut").mkdir()
        subject = str(Path(shutil.which("gdbus")).resolve())
        (self.root / "data/coconut/permissions.json").write_text(json.dumps({"entries": [
            {"subject": subject, "permission": permission, "verdict": "deny", "granted_at": 0}
            for permission in ("clipboard-read", "clipboard-history", "clipboard-write")
        ]}))
        bus_config = self.root / "bus.conf"
        bus_config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen>'
                              '<auth>EXTERNAL</auth><policy context="default">'
                              '<allow send_destination="*"/><allow receive_sender="*"/>'
                              '<allow own="*"/></policy></busconfig>')
        bus = subprocess.Popen(["dbus-daemon", f"--config-file={bus_config}", "--nofork", "--print-address=1"],
                               stdout=subprocess.PIPE, text=True, start_new_session=True)
        self.children.append(bus)
        self.env["DBUS_SESSION_BUS_ADDRESS"] = bus.stdout.readline().strip()
        sentinel = self.root / "sentinel.py"
        sentinel.write_text("import json, os, pathlib, time\n"
                            "pathlib.Path(__file__).with_suffix('.json').write_text(json.dumps(dict(os.environ)))\n"
                            "while True: time.sleep(1)\n")
        self.start([os.environ["COCONUT_CLIPBOARD_TEST_BLAIR"], "--run", f"python3 {sentinel}"])
        self.wait_for(lambda: (self.root / "sentinel.json").exists())
        self.env = json.loads((self.root / "sentinel.json").read_text())
        self.start([str(self.root / "coconut-permission-portal"), "--service"])
        self.service = self.start([str(self.root / "coconut-clipboard")])
        self.wait_for(lambda: self.call("Status")[0])

    def stop(self):
        for child in reversed(self.children):
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
            if child.stdout:
                child.stdout.close()

    def call(self, method, *args, trusted=True, permissions=False):
        binary = self.root / "coconut-settings" if permissions else self.root / "coconut"
        if not trusted:
            binary = shutil.which("gdbus")
        service = "org.coconut.Permissions" if permissions else "org.coconut.Clipboard"
        interface = "org.coconut.Permissions1" if permissions else "org.coconut.Clipboard1"
        result = subprocess.run([str(binary), "call", "--session", "--dest", service,
                                 "--object-path", "/" + interface.replace(".", "/"),
                                 "--method", interface + "." + method, *map(str, args)],
                                env=self.env, capture_output=True, text=True, timeout=8)
        if result.returncode:
            raise ValueError(result.stderr)
        return self.GLib.Variant.parse(None, result.stdout.strip(), None, None).unpack()

    def copy(self, text, mime="text/plain;charset=utf-8"):
        process = subprocess.Popen(["wl-copy", "--foreground", "--type", mime],
                                   stdin=subprocess.PIPE, env=self.env,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                   start_new_session=True)
        self.children.append(process)
        process.stdin.write(text.encode())
        process.stdin.close()

    def test_native_copy_restore_limits_pause_and_permissions(self):
        for text in ("first · 日本語", "second", "third", "fourth"):
            self.copy(text)
            self.wait_for(lambda: self.call("Current") == (True, text))
        entries = self.call("List", 0, 50)[0]
        self.assertEqual([entry[2] for entry in entries], ["fourth", "third", "second"])
        self.assertEqual(self.call("List", 1, 1)[0][0][2], "third")
        self.assertEqual([entry[2] for entry in self.call("Search", "THIR", 0, 50)[0]], ["third"])
        self.assertEqual(self.call("Search", "absent", 0, 50)[0], [])
        identifier = entries[-1][0]
        self.assertEqual(self.call("Get", identifier), ("second",))
        self.call("Restore", identifier)
        self.wait_for(lambda: self.call("Current") == (True, "second"))
        pasted = subprocess.check_output(["wl-paste", "--no-newline"], env=self.env, timeout=5).decode()
        self.assertEqual(pasted, "second")
        self.call("SetEnabled", "false")
        before = self.call("List", 0, 50)[0]
        self.copy("not saved")
        self.wait_for(lambda: self.call("Current") == (True, "not saved"))
        self.assertEqual(self.call("List", 0, 50)[0], before)
        self.call("SetEnabled", "true")
        self.copy("x" * 129)
        self.wait_for(lambda: not self.call("Current")[0])
        self.assertEqual(self.call("List", 0, 50)[0], before)
        self.copy("password", "x-kde-passwordManagerHint")
        self.wait_for(lambda: not self.call("Current")[0])
        self.assertEqual(self.call("List", 0, 50)[0], before)
        for method, args in [("Current", ()), ("List", (0, 10)), ("Search", ("second", 0, 10)), ("SetText", ("denied",)), ("SetEnabled", ("false",)), ("Configure", (10, 128, 1024, 3600))]:
            with self.assertRaises(ValueError):
                self.call(method, *args, trusted=False)
        subject = str(Path(shutil.which("gdbus")).resolve())
        self.call("SetDecision", subject, "clipboard-read", "allow", permissions=True)
        self.call("Current", trusted=False)
        with self.assertRaises(ValueError):
            self.call("List", 0, 10, trusted=False)
        self.call("SetDecision", subject, "clipboard-read", "deny", permissions=True)
        with self.assertRaises(ValueError):
            self.call("Current", trusted=False)
        self.call("Configure", 2, 128, 1024, 3600)
        self.assertEqual(self.call("GetConfig"), (2, 128, 1024, 3600))
        with self.assertRaises(ValueError):
            self.call("Configure", 0, 128, 1024, 3600)
        revision = self.call("Status")[3]
        self.call("SetText", "kept after clearing history")
        self.assertNotEqual(self.call("WaitChanged", revision)[0], revision)
        self.call("Clear")
        self.assertEqual(self.call("List", 0, 50)[0], [])
        self.assertEqual(self.call("Current"), (True, "kept after clearing history"))
        config = self.root / "config/coconut/clipboard.toml"
        saved = config.with_suffix(".saved")
        config.rename(saved)
        config.mkdir()
        with self.assertRaises(ValueError):
            self.call("SetEnabled", "false")
        self.assertTrue(self.call("Status")[1])
        config.rmdir()
        saved.rename(config)
        original = config.read_text()
        config.write_text(original.replace("max_entries = 2", "max_entries = 3"))
        with self.assertRaises(ValueError):
            self.call("SetEnabled", "false")
        self.assertIn("max_entries = 3", config.read_text())
        self.assertEqual(self.call("GetConfig"), (2, 128, 1024, 3600))
        config.write_text(original)
        self.service.terminate()
        self.service.wait(timeout=5)
        self.service = self.start([str(self.root / "coconut-clipboard")])
        self.wait_for(lambda: self.call("Status")[0])
        self.assertEqual(self.call("List", 0, 50)[0], [])
        self.assertEqual(self.call("GetConfig"), (2, 128, 1024, 3600))
        self.children[1].terminate()
        self.children[1].wait(timeout=5)
        self.service.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
