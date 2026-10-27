//! Exercise the developer installer against temporary XDG directories.
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn user_installer_uses_one_binary_and_preserves_existing_policy() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let commands = root.join("commands");
    fs::create_dir(&commands).unwrap();
    executable(&commands.join("cargo"), "#!/bin/sh\nexit 0\n");
    executable(
        &commands.join("systemctl"),
        "#!/bin/sh\ntest \"$*\" = '--user daemon-reload'\n",
    );
    executable(
        &commands.join("update-desktop-database"),
        "#!/bin/sh\nexit 0\n",
    );
    let config = root.join("config");
    let data = root.join("data");
    let install_root = root.join("cargo & apps");
    let policy = config.join("xdg-desktop-portal/coconut-portals.conf");
    fs::create_dir_all(policy.parent().unwrap()).unwrap();
    fs::write(&policy, "user preferences\n").unwrap();
    let output = Command::new("bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh"))
        .current_dir(root)
        .env("PATH", format!("{}:/usr/bin:/bin", commands.display()))
        .env("CARGO_INSTALL_ROOT", &install_root)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("COCONUT_SESSIONS_DIR", data.join("wayland-sessions"))
        .env("COCONUT_WM", "kwin_wayland")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let service = fs::read_to_string(
        data.join("dbus-1/services/org.freedesktop.impl.portal.desktop.cfiles.service"),
    )
    .unwrap();
    assert!(service.contains(&format!(
        "Exec=\"{}/bin/cfiles\" --portal",
        install_root.display()
    )));
    let unit = fs::read_to_string(config.join("systemd/user/cfiles-portal.service")).unwrap();
    assert!(unit.contains(&format!(
        "ExecStart=\"{}/bin/cfiles\" --portal",
        install_root.display()
    )));
    assert!(data
        .join("xdg-desktop-portal/portals/cfiles.portal")
        .exists());
    assert!(data.join("applications/cfiles.desktop").exists());
    assert_eq!(fs::read_to_string(policy).unwrap(), "user preferences\n");
    assert!(config
        .join("xdg-desktop-portal/blair-portals.conf")
        .exists());
    let session = fs::read_to_string(install_root.join("bin/startcoconut")).unwrap();
    assert!(session.contains("WM=\"kwin_wayland\""));
    assert!(session.contains(&format!("SHELL=\"{}/bin/coconut\"", install_root.display())));
    assert!(data.join("wayland-sessions/coconut.desktop").exists());
}
