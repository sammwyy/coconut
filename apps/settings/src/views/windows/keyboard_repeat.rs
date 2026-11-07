use super::*;
use std::{fs, io::Write, path::PathBuf, process::Command, time::Duration};

#[derive(Clone, Copy)]
enum Backend {
    KWin,
    Hyprland,
}

pub(super) struct ExternalKeyboard {
    backend: Backend,
    hypr_instance: Option<String>,
    delay: TextController,
    rate: TextController,
    status: Signal<String>,
    busy: Signal<bool>,
}

impl ExternalKeyboard {
    pub(super) fn detect() -> Option<Self> {
        let hypr_instance = active_hypr_instance();
        let kwin = kwin_running();
        let backend = detect_backend(
            &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            hypr_instance.is_some()
                || (!kwin && std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()),
            kwin,
        )?;
        let loaded = backend.read(hypr_instance.as_deref());
        let (delay, rate) = loaded.as_ref().copied().unwrap_or((0, 0));
        Some(Self {
            backend,
            hypr_instance,
            delay: TextController::new(if loaded.is_ok() {
                delay.to_string()
            } else {
                String::new()
            }),
            rate: TextController::new(if loaded.is_ok() {
                rate.to_string()
            } else {
                String::new()
            }),
            status: Signal::new(loaded.err().unwrap_or_default()),
            busy: Signal::new(false),
        })
    }

    pub(super) fn view(&self, app: &Rc<RefCell<Option<creamui_render::AppHandle>>>) -> BoxedWidget {
        let backend = self.backend;
        let hypr_instance = self.hypr_instance.clone();
        let delay = self.delay.clone();
        let rate = self.rate.clone();
        let status = self.status.clone();
        let busy = self.busy.clone();
        let app = app.clone();
        let description = match backend {
            Backend::KWin => {
                "KWin keyboard repeat settings (milliseconds and characters per second)."
            }
            Backend::Hyprland => {
                "Hyprland keyboard repeat settings (milliseconds and characters per second)."
            }
        };
        section(
            "Keyboard",
            description,
            vec![
                group(vec![
                    row(
                        "Repeat delay (ms)",
                        Box::new(crate::components::form_input(
                            TextInput::controlled(&self.delay).placeholder("0–2000"),
                            112.0,
                        )),
                    ),
                    row(
                        "Repeat rate (per second)",
                        Box::new(crate::components::form_input(
                            TextInput::controlled(&self.rate).placeholder("1–100"),
                            112.0,
                        )),
                    ),
                ]),
                Box::new(Button::styled(
                    ButtonVariant::Primary,
                    ButtonSize::Md,
                    "Save keyboard settings",
                    if self.busy.get() {
                        ButtonState::Loading
                    } else {
                        ButtonState::Normal
                    },
                    move || {
                        if busy.get() {
                            return;
                        }
                        let values = parse_values(&delay.peek(), &rate.peek());
                        let (delay, rate) = match values {
                            Ok(values) => values,
                            Err(error) => {
                                status.set(error);
                                return;
                            }
                        };
                        let Some(app) = app.borrow().clone() else {
                            status.set("Settings are not ready yet".into());
                            return;
                        };
                        busy.set(true);
                        let status = status.clone();
                        let busy = busy.clone();
                        let hypr_instance = hypr_instance.clone();
                        app.spawn_background(
                            move || backend.save(delay, rate, hypr_instance.as_deref()),
                            move |result| {
                                busy.set(false);
                                status.set(match result {
                                    Ok(message) => message,
                                    Err(error) => {
                                        format!("Could not save keyboard settings: {error}")
                                    }
                                });
                            },
                        );
                    },
                )),
                Box::new(Text::secondary(self.status.get()).size(TextSize::Sm)),
            ],
        )
    }
}

fn kwin_running() -> bool {
    let Ok(bus) = dbus::blocking::Connection::new_session() else {
        return false;
    };
    let proxy = bus.with_proxy(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        Duration::from_secs(2),
    );
    let result: Result<(bool,), _> =
        proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("org.kde.KWin",));
    result.is_ok_and(|(running,)| running)
}

fn detect_backend(desktop: &str, hyprland: bool, kwin: bool) -> Option<Backend> {
    if hyprland {
        Some(Backend::Hyprland)
    } else if kwin {
        Some(Backend::KWin)
    } else if desktop
        .split(':')
        .any(|part| part.eq_ignore_ascii_case("hyprland"))
    {
        Some(Backend::Hyprland)
    } else if desktop
        .split(':')
        .any(|part| part.eq_ignore_ascii_case("KDE"))
    {
        Some(Backend::KWin)
    } else {
        None
    }
}

fn active_hypr_instance() -> Option<String> {
    let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
    if !runtime.join("hypr").is_dir() {
        return None;
    }
    let display = std::env::var_os("WAYLAND_DISPLAY")?;
    let display = std::path::Path::new(&display);
    if display.is_absolute() && display.parent() != Some(runtime.as_path()) {
        return None;
    }
    let socket = display.file_name()?.to_str()?;
    let instances = run("hyprctl", &["-j", "instances"]).ok()?;
    let instances: serde_json::Value = serde_json::from_str(&instances).ok()?;
    instance_for_socket(&instances, socket)
}

fn instance_for_socket(instances: &serde_json::Value, socket: &str) -> Option<String> {
    instances
        .as_array()?
        .iter()
        .find(|instance| instance.get("wl_socket").and_then(|value| value.as_str()) == Some(socket))
        .and_then(|instance| instance.get("instance")?.as_str().map(str::to_owned))
}

fn parse_values(delay: &str, rate: &str) -> Result<(i32, i32), String> {
    let delay = delay
        .parse::<i32>()
        .map_err(|_| "Enter a whole-number delay in milliseconds".to_owned())?;
    let rate = rate
        .parse::<i32>()
        .map_err(|_| "Enter a whole-number repeat rate per second".to_owned())?;
    if !(0..=2_000).contains(&delay) || !(1..=100).contains(&rate) {
        return Err("Delay must be 0–2000 ms and rate must be 1–100 per second".into());
    }
    Ok((delay, rate))
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("{program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn run_hypr(instance: Option<&str>, args: &[&str]) -> Result<String, String> {
    let mut command = Vec::with_capacity(args.len() + 2);
    if let Some(instance) = instance {
        command.extend(["-i", instance]);
    }
    command.extend_from_slice(args);
    run("hyprctl", &command)
}

impl Backend {
    fn read(self, instance: Option<&str>) -> Result<(i32, i32), String> {
        match self {
            Self::KWin => {
                let program = if Command::new("kreadconfig6").arg("--help").output().is_ok() {
                    "kreadconfig6"
                } else {
                    "kreadconfig5"
                };
                let read = |key, default| {
                    run(
                        program,
                        &[
                            "--file",
                            "kcminputrc",
                            "--group",
                            "Keyboard",
                            "--key",
                            key,
                            "--default",
                            default,
                        ],
                    )?
                    .parse::<f64>()
                    .map(|value| value.round() as i32)
                    .map_err(|error| format!("Invalid KWin {key}: {error}"))
                };
                Ok((read("RepeatDelay", "660")?, read("RepeatRate", "25")?))
            }
            Self::Hyprland => {
                let read = |key| {
                    let output = run_hypr(instance, &["-j", "getoption", key])?;
                    serde_json::from_str::<serde_json::Value>(&output)
                        .map_err(|error| error.to_string())?
                        .get("int")
                        .and_then(serde_json::Value::as_i64)
                        .and_then(|value| i32::try_from(value).ok())
                        .ok_or_else(|| format!("Hyprland did not report {key}"))
                };
                Ok((read("input:repeat_delay")?, read("input:repeat_rate")?))
            }
        }
    }

    fn save(self, delay: i32, rate: i32, instance: Option<&str>) -> Result<String, String> {
        match self {
            Self::KWin => {
                let program = if Command::new("kwriteconfig6").arg("--help").output().is_ok() {
                    "kwriteconfig6"
                } else {
                    "kwriteconfig5"
                };
                for (key, value) in [("RepeatDelay", delay), ("RepeatRate", rate)] {
                    run(
                        program,
                        &[
                            "--file",
                            "kcminputrc",
                            "--group",
                            "Keyboard",
                            "--key",
                            key,
                            &value.to_string(),
                        ],
                    )?;
                }
                let bus =
                    dbus::blocking::Connection::new_session().map_err(|error| error.to_string())?;
                let proxy = bus.with_proxy("org.kde.KWin", "/KWin", Duration::from_secs(5));
                let _: () = proxy
                    .method_call("org.kde.KWin", "reconfigure", ())
                    .map_err(|error| error.to_string())?;
                Ok("KWin keyboard settings saved and applied".into())
            }
            Self::Hyprland => {
                let config = hypr_config_path()?;
                let (main, managed, source, contents, lua) = if config
                    .join("hyprland.lua")
                    .is_file()
                {
                    let managed = config.join("coconut-keyboard.lua");
                    let path = hypr_path(&managed)?.to_owned();
                    (
                        config.join("hyprland.lua"),
                        managed,
                        format!("dofile(\"{}\")", path.replace('\\', "\\\\").replace('"', "\\\"")),
                        format!(
                            "hl.config({{ input = {{ repeat_rate = {rate}, repeat_delay = {delay} }} }})\n"
                        ),
                        true,
                    )
                } else if config.join("hyprland.conf").is_file() {
                    let managed = config.join("coconut-keyboard.conf");
                    let path = hypr_path(&managed)?.to_owned();
                    (
                        config.join("hyprland.conf"),
                        managed,
                        format!("source = {path}"),
                        format!("input:repeat_rate = {rate}\ninput:repeat_delay = {delay}\n"),
                        false,
                    )
                } else {
                    return Err("No Hyprland configuration file found to save the setting".into());
                };
                let original = fs::read_to_string(&main).map_err(|error| error.to_string())?;
                let updated = add_source(&original, &source);
                match fs::symlink_metadata(&managed) {
                    Ok(metadata) => {
                        if !metadata.file_type().is_file()
                            || !managed_config(
                                &fs::read_to_string(&managed).map_err(|error| error.to_string())?,
                                lua,
                            )
                        {
                            return Err(
                                "Existing Coconut keyboard configuration contains other settings"
                                    .into(),
                            );
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.to_string()),
                }
                fs::write(&managed, contents).map_err(|error| error.to_string())?;
                if updated != original {
                    fs::OpenOptions::new()
                        .append(true)
                        .open(&main)
                        .and_then(|mut file| file.write_all(&updated.as_bytes()[original.len()..]))
                        .map_err(|error| error.to_string())?;
                }
                run_hypr(instance, &["reload"])?;
                Ok("Hyprland keyboard settings saved and applied".into())
            }
        }
    }
}

fn hypr_config_path() -> Result<PathBuf, String> {
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config).join("hypr"));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".config/hypr"))
        .ok_or_else(|| "Cannot locate the Hyprland configuration directory".into())
}

fn hypr_path(path: &std::path::Path) -> Result<&str, String> {
    path.to_str()
        .filter(|path| !path.contains('\n') && !path.contains('\r'))
        .ok_or_else(|| "Unsupported Hyprland configuration path".to_owned())
}

fn add_source(config: &str, source: &str) -> String {
    if config.lines().any(|line| line.trim() == source) {
        return config.to_owned();
    }
    format!("{config}\n{source}\n")
}

fn managed_config(contents: &str, lua: bool) -> bool {
    if lua {
        contents
            .strip_prefix("hl.config({ input = { repeat_rate = ")
            .and_then(|rest| rest.strip_suffix(" } })\n"))
            .and_then(|rest| rest.split_once(", repeat_delay = "))
            .is_some_and(|(rate, delay)| {
                rate.parse::<u32>().is_ok() && delay.parse::<u32>().is_ok()
            })
    } else {
        contents.lines().all(|line| {
            let setting = line
                .strip_prefix("input:repeat_rate = ")
                .or_else(|| line.strip_prefix("input:repeat_delay = "));
            setting.is_some_and(|value| value.parse::<u32>().is_ok())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_current_compositor_and_validates_repeat() {
        assert!(matches!(
            detect_backend("KDE:Plasma", false, false),
            Some(Backend::KWin)
        ));
        assert!(matches!(
            detect_backend("Coconut", true, true),
            Some(Backend::Hyprland)
        ));
        assert!(matches!(
            detect_backend("Coconut", false, true),
            Some(Backend::KWin)
        ));
        assert!(matches!(
            detect_backend("Hyprland", false, true),
            Some(Backend::KWin)
        ));
        assert!(detect_backend("GNOME", false, false).is_none());
        assert_eq!(parse_values("400", "25"), Ok((400, 25)));
        assert!(parse_values("-1", "25").is_err());
        assert!(parse_values("400", "0").is_err());
        let instances = serde_json::json!([
            {"instance": "other", "wl_socket": "wayland-0"},
            {"instance": "current", "wl_socket": "wayland-1"}
        ]);
        assert_eq!(
            instance_for_socket(&instances, "wayland-1").as_deref(),
            Some("current")
        );
        assert_eq!(instance_for_socket(&instances, "wayland-2"), None);
    }

    #[test]
    fn hypr_source_is_added_only_once() {
        let source = "source = /home/user/.config/hypr/coconut-keyboard.conf";
        let result = add_source("input:repeat_rate = 25\n", source);
        assert_eq!(add_source(&result, source), result);
        assert!(result.ends_with(&format!("\n{source}\n")));
        assert!(managed_config(
            "input:repeat_rate = 25\ninput:repeat_delay = 400\n",
            false
        ));
        assert!(!managed_config(
            "input:repeat_rate = 25\nexec = command\n",
            false
        ));
        assert!(managed_config(
            "hl.config({ input = { repeat_rate = 25, repeat_delay = 400 } })\n",
            true
        ));
        assert!(!managed_config("hl.exec_cmd(\"something\")\n", true));
        let lua = "dofile(\"/home/user/.config/hypr/coconut-keyboard.lua\")";
        assert_eq!(add_source(&result, lua), format!("{result}\n{lua}\n"));
        assert_eq!(
            add_source(&add_source(&result, lua), lua),
            add_source(&result, lua)
        );
    }
}
