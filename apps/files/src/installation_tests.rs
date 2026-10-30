//! Exercise the developer installer against temporary XDG directories.
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn user_installer_installs_theme_integrations_and_preserves_existing_policy() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let commands = root.join("commands");
    fs::create_dir(&commands).unwrap();
    // Keep this a user-local fixture even on a root-run CI worker.
    executable(&commands.join("id"), "#!/bin/sh\nprintf '1000\\n'\n");
    executable(&commands.join("cargo"), "#!/bin/sh\nexit 0\n");
    executable(&commands.join("fc-cache"), "#!/bin/sh\nexit 0\n");
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
    let staged = root.join("staged");
    fs::create_dir_all(staged.join("qt/plugins/platformthemes")).unwrap();
    for binary in [
        "coconut",
        "coconut-bg",
        "coconut-greeter",
        "coconut-settings",
        "coconut-appearance",
        "cterm",
        "cfiles",
        "cmonitor",
        "cplayer",
    ] {
        executable(&staged.join(binary), "#!/bin/sh\nexit 0\n");
    }
    fs::write(
        staged.join("qt/plugins/platformthemes/libcoconut-qt6.so"),
        "fixture plugin",
    )
    .unwrap();
    let policy = config.join("xdg-desktop-portal/coconut-portals.conf");
    fs::create_dir_all(policy.parent().unwrap()).unwrap();
    fs::write(&policy, "user preferences\n").unwrap();
    let output = Command::new("bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh"))
        .current_dir(root)
        .env("PATH", format!("{}:/usr/bin:/bin", commands.display()))
        .env("COCONUT_BUILD_DIR", &staged)
        .env("COCONUT_INSTALL_BIN", install_root.join("bin"))
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
    assert!(session.contains(&format!(
        "SESSION=\"{}/bin/coconut-session\"",
        install_root.display()
    )));
    let launcher = fs::read_to_string(install_root.join("bin/coconut-session")).unwrap();
    assert!(launcher.contains("coconut-bg"));
    assert!(launcher.contains("coconut\""));
    assert!(data.join("wayland-sessions/coconut.desktop").exists());
    assert!(data.join("coconut/themes/coconut/theme.toml").exists());
    assert!(data.join("fonts/coconut/Regular.ttf").exists());
    assert!(data
        .join("coconut/qt/plugins/platformthemes/libcoconut-qt6.so")
        .exists());
    assert!(data
        .join("xdg-desktop-portal/portals/coconut.portal")
        .exists());
    let environment = fs::read_to_string(data.join("coconut/environment")).unwrap();
    assert!(environment.contains("CREAMUI_APPEARANCE"));
    assert!(environment.contains(&format!("{}/coconut/qt/plugins", data.display())));
    let default_policy =
        fs::read_to_string(config.join("xdg-desktop-portal/blair-portals.conf")).unwrap();
    assert!(default_policy.contains("org.freedesktop.impl.portal.Settings=coconut"));
}
