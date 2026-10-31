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
    executable(&commands.join("cfiles"), "#!/bin/sh\nexit 99\n");
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
        "cedit",
        "ccalc",
        "cviewer",
        "coconut-screenshots",
        "coconut-permission-portal",
        "coconut-clipboard",
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
    // A leftover copy of the previously shipped policy must be upgraded in
    // place so new portal backends (like Access) get registered.
    let blair_policy = config.join("xdg-desktop-portal/blair-portals.conf");
    fs::write(
        &blair_policy,
        "[preferred]\ndefault=gtk\norg.freedesktop.impl.portal.Settings=coconut\norg.freedesktop.impl.portal.FileChooser=cfiles;gtk\n",
    )
    .unwrap();
    let output = Command::new("bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh"))
        .arg("--user")
        // Positional compositor names resolve through the alias table even
        // when the binary is not on PATH.
        .arg("kwin")
        .current_dir(root)
        .env("PATH", format!("{}:/usr/bin:/bin", commands.display()))
        .env("COCONUT_BUILD_DIR", &staged)
        .env("COCONUT_INSTALL_BIN", install_root.join("bin"))
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("COCONUT_SESSIONS_DIR", data.join("wayland-sessions"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Installed 15 staged binaries"));
    assert!(stdout.contains("PATH selects other copies"));
    assert!(stdout.contains(&format!("cfiles: {}", commands.join("cfiles").display())));
    for binary in fs::read_dir(&staged)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().unwrap().is_file())
    {
        assert_eq!(
            fs::read(binary.path()).unwrap(),
            fs::read(install_root.join("bin").join(binary.file_name())).unwrap()
        );
    }
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
    let desktop = fs::read_to_string(data.join("applications/cfiles.desktop")).unwrap();
    assert!(desktop.contains("Icon=cfiles\n"));
    assert_eq!(
        fs::read(data.join("icons/hicolor/scalable/apps/cfiles.svg")).unwrap(),
        include_bytes!("../../../assets/icons/apps/app-files.svg")
    );
    assert!(data.join("applications/cedit.desktop").exists());
    assert!(data.join("applications/ccalc.desktop").exists());
    assert!(install_root.join("bin/ccalc").exists());
    let calculator = fs::read_to_string(data.join("applications/ccalc.desktop")).unwrap();
    assert!(calculator.contains("Exec=ccalc"));
    assert!(install_root.join("bin/cviewer").exists());
    let viewer = fs::read_to_string(data.join("applications/cviewer.desktop")).unwrap();
    assert!(viewer.contains("Exec=cviewer %F"));
    assert!(viewer.contains("MimeType=image/png;image/jpeg;"));
    assert!(install_root.join("bin/coconut-screenshots").exists());
    let screenshots =
        fs::read_to_string(data.join("applications/coconut-screenshots.desktop")).unwrap();
    assert!(screenshots.contains("Exec=coconut-screenshots"));
    assert!(screenshots.contains("X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2"));
    assert!(install_root.join("bin/coconut-permission-portal").exists());
    assert!(install_root.join("bin/coconut-clipboard").exists());
    let clipboard =
        fs::read_to_string(data.join("dbus-1/services/org.coconut.Clipboard.service")).unwrap();
    assert!(clipboard.contains(&format!(
        "Exec=\"{}/bin/coconut-clipboard\"",
        install_root.display()
    )));
    assert!(config
        .join("systemd/user/coconut-clipboard.service")
        .exists());
    let permissions =
        fs::read_to_string(data.join("dbus-1/services/org.coconut.Permissions.service")).unwrap();
    assert!(permissions.contains(&format!(
        "Exec=\"{}/bin/coconut-permission-portal\" --service",
        install_root.display()
    )));
    assert!(config
        .join("systemd/user/coconut-permission-portal.service")
        .exists());
    assert!(data
        .join("xdg-desktop-portal/portals/coconut-permissions.portal")
        .exists());
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
    assert!(default_policy.contains("org.freedesktop.impl.portal.Access=coconut-permissions"));
}

#[test]
fn system_installer_preserves_staging_directory_and_caller_path_when_elevating() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let commands = root.join("commands");
    fs::create_dir(&commands).unwrap();
    executable(&commands.join("id"), "#!/bin/sh\nprintf '1000\n'\n");
    executable(
        &commands.join("sudo"),
        "#!/bin/sh\nprintf '%s\n' \"$@\" > \"$COCONUT_TEST_SUDO_ARGS\"\n",
    );
    let staged = root.join("custom staged binaries");
    let arguments = root.join("sudo-arguments");
    let path = format!("{}:/usr/bin:/bin", commands.display());
    let output = Command::new("bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh"))
        .args(["blair", "--force"])
        .env("PATH", &path)
        .env("COCONUT_BUILD_DIR", &staged)
        .env("COCONUT_TEST_SUDO_ARGS", &arguments)
        .env_remove("COCONUT_INSTALL_CALLER_PATH")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let arguments = fs::read_to_string(arguments).unwrap();
    assert!(arguments
        .lines()
        .any(|line| line == format!("COCONUT_BUILD_DIR={}", staged.display())));
    assert!(arguments
        .lines()
        .any(|line| line == format!("COCONUT_INSTALL_CALLER_PATH={path}")));
}

#[test]
fn user_installation_rejects_root_instead_of_writing_system_paths() {
    let fixture = tempfile::tempdir().unwrap();
    executable(&fixture.path().join("id"), "#!/bin/sh\nprintf '0\n'\n");
    let output = Command::new("bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh"))
        .arg("--user")
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", fixture.path().display()),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("run --user without sudo"));
}
