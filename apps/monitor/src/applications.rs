//! Recognize desktop launchers and account for each application's process family.
use crate::system::Process;
use std::collections::{HashMap, HashSet};
#[cfg(target_os = "linux")]
use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub processes: Vec<Process>,
    pub cpu: f32,
    pub memory: u64,
}

#[derive(Clone)]
struct DesktopApp {
    id: String,
    name: String,
    executable: String,
    arguments: Vec<String>,
}

pub fn collect(processes: &[Process]) -> Vec<Application> {
    #[cfg(target_os = "windows")]
    {
        let catalog = load_catalog(processes);
        group(processes, &catalog, &crate::system::current_user())
    }
    #[cfg(target_os = "linux")]
    {
        struct Catalog {
            loaded: Instant,
            entries: Vec<DesktopApp>,
        }
        static CATALOG: OnceLock<Mutex<Catalog>> = OnceLock::new();
        let mut catalog = CATALOG
            .get_or_init(|| {
                Mutex::new(Catalog {
                    loaded: Instant::now(),
                    entries: load_catalog(processes),
                })
            })
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if catalog.loaded.elapsed() >= Duration::from_secs(60) {
            catalog.entries = load_catalog(processes);
            catalog.loaded = Instant::now();
        }
        group(processes, &catalog.entries, &crate::system::current_user())
    }
}

fn group(processes: &[Process], catalog: &[DesktopApp], uid: &str) -> Vec<Application> {
    let by_pid: HashMap<_, _> = processes.iter().map(|p| (p.pid, p)).collect();
    let direct: HashMap<_, _> = processes
        .iter()
        .filter(|p| p.user == uid)
        .filter_map(|p| {
            catalog
                .iter()
                .filter(|app| app.matches(p))
                .max_by_key(|app| app.arguments.len())
                .map(|app| (p.pid, app))
        })
        .collect();
    let mut groups: HashMap<String, Application> = HashMap::new();
    for process in processes.iter().filter(|p| p.user == uid) {
        let mut ancestor = process;
        let mut visited = HashSet::new();
        while visited.insert(ancestor.pid) {
            if let Some(app) = direct.get(&ancestor.pid) {
                let group = groups.entry(app.id.clone()).or_insert_with(|| Application {
                    id: app.id.clone(),
                    name: app.name.clone(),
                    processes: vec![],
                    cpu: 0.0,
                    memory: 0,
                });
                group.cpu += process.cpu;
                group.memory = group.memory.saturating_add(process.memory);
                group.processes.push(process.clone());
                break;
            }
            let Some(parent) = by_pid.get(&ancestor.parent_pid).filter(|p| p.user == uid) else {
                break;
            };
            ancestor = parent;
        }
    }
    let mut applications: Vec<_> = groups.into_values().collect();
    applications.sort_by(|a, b| b.memory.cmp(&a.memory).then(a.id.cmp(&b.id)));
    applications
}

impl DesktopApp {
    fn matches(&self, p: &Process) -> bool {
        let id = self.id.trim_end_matches(".desktop");
        let cgroup = p.cgroup.replace("\\x2d", "-");
        if [
            format!("app-{id}-"),
            format!("app-flatpak-{id}-"),
            format!("app-gnome-{id}-"),
        ]
        .iter()
        .any(|scope| cgroup.contains(scope))
        {
            return true;
        }
        let program = basename(&self.executable);
        let executable = basename(&p.executable);
        let argv0 = p.arguments.first().map(|s| basename(s));
        if !same_program(&self.executable, &p.executable)
            && !same_program(program, executable)
            && !argv0.is_some_and(|argument| same_program(argument, program))
        {
            return false;
        }
        // Shared launchers/interpreters need the application-specific argument;
        // matching "python", "flatpak", or "electron" alone groups unrelated apps.
        let generic = matches!(
            program,
            "env"
                | "sh"
                | "bash"
                | "python"
                | "python3"
                | "java"
                | "node"
                | "electron"
                | "flatpak"
                | "snap"
                | "waydroid"
                | "wine"
                | "wine64"
        );
        if generic {
            let identity = self.arguments.iter().find(|arg| {
                !arg.starts_with('-')
                    && !matches!(arg.as_str(), "run" | "--" | "show" | "app" | "launch")
            });
            return identity
                .is_some_and(|identity| p.arguments.iter().skip(1).any(|arg| arg == identity));
        }
        self.arguments
            .iter()
            .all(|argument| p.arguments.iter().skip(1).any(|actual| actual == argument))
    }
}
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(target_os = "linux")]
fn same_program(left: &str, right: &str) -> bool {
    left == right
}

#[cfg(target_os = "windows")]
fn same_program(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(target_os = "linux")]
fn load_catalog(_processes: &[Process]) -> Vec<DesktopApp> {
    let paths = delib::xdg::XdgPaths::default();
    let mut directories = paths.application_dirs();
    // Flatpak exports may be absent from a minimal session's XDG_DATA_DIRS.
    directories.push(paths.data_home.join("flatpak/exports/share/applications"));
    directories.push("/var/lib/flatpak/exports/share/applications".into());
    delib::desktop_entry::installed_applications_in(directories)
        .into_iter()
        .filter_map(to_desktop_app)
        .collect()
}

#[cfg(target_os = "windows")]
fn load_catalog(processes: &[Process]) -> Vec<DesktopApp> {
    let current_user = crate::system::current_user();
    let by_pid: HashMap<_, _> = processes
        .iter()
        .map(|process| (process.pid, process))
        .collect();
    let mut applications = processes
        .iter()
        .filter(|process| process.user == current_user && !process.executable.is_empty())
        .filter(|process| {
            by_pid
                .get(&process.parent_pid)
                .is_none_or(|parent| parent.user != current_user)
        })
        .map(|process| {
            let name = std::path::Path::new(&process.executable)
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| process.name.clone());
            DesktopApp {
                id: process.executable.to_lowercase(),
                name,
                executable: process.executable.clone(),
                arguments: Vec::new(),
            }
        })
        .collect::<Vec<_>>();
    applications.sort_by(|left, right| left.id.cmp(&right.id));
    applications.dedup_by(|left, right| left.id == right.id);
    applications
}

#[cfg(target_os = "linux")]
fn to_desktop_app(entry: delib::desktop_entry::DesktopEntry) -> Option<DesktopApp> {
    if !entry.is_visible_application() || entry.terminal() {
        return None;
    }
    let id = entry.id().unwrap_or_default().to_owned();
    let name = entry.name(None).unwrap_or(&id).to_owned();
    let mut command = entry.command_for_files(&[])?;
    if basename(command.first()?) == "env" {
        command.remove(0);
        while command.first().is_some_and(|arg| arg.contains('=')) {
            command.remove(0);
        }
    }
    let executable = command.first()?.clone();
    Some(DesktopApp {
        id,
        name,
        executable,
        arguments: command.into_iter().skip(1).collect(),
    })
}
#[cfg(all(test, target_os = "linux"))]
fn parse_desktop(id: &str, contents: &str) -> Option<DesktopApp> {
    to_desktop_app(delib::desktop_entry::DesktopEntry::parse(id, contents).with_id(id))
}

#[cfg(all(test, target_os = "linux"))]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    fn process(pid: u32, parent: u32, exe: &str, args: &[&str]) -> Process {
        let mut p = Process::default();
        p.pid = pid;
        p.parent_pid = parent;
        p.user = "1000".into();
        p.executable = exe.into();
        p.arguments = args.iter().map(|s| s.to_string()).collect();
        p.memory = 1024;
        p.cpu = 2.0;
        p
    }
    #[test]
    fn desktop_apps_group_helpers_and_exclude_unrelated_daemons() {
        let app = parse_desktop(
            "browser.desktop",
            "[Desktop Entry]\nType=Application\nName=Browser\nExec=/usr/bin/browser %U\n",
        )
        .unwrap();
        let processes = vec![
            process(1, 0, "/usr/lib/browser", &["browser"]),
            process(2, 1, "/usr/lib/renderer", &["renderer"]),
            process(3, 2, "/usr/bin/helper", &["helper"]),
            process(4, 0, "/usr/bin/pipewire", &["pipewire"]),
            process(5, 0, "/usr/bin/systemd", &["systemd"]),
        ];
        let apps = group(&processes, &[app], "1000");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].processes.len(), 3);
        assert_eq!(apps[0].memory, 3072);
        assert_eq!(apps[0].cpu, 6.0);
    }
    #[test]
    fn shared_launchers_require_application_identity() {
        let app=parse_desktop("org.example.App.desktop","[Desktop Entry]\nType=Application\nName=Example\nExec=flatpak run --branch=stable org.example.App %U\n").unwrap();
        assert!(app.matches(&process(
            1,
            0,
            "/usr/bin/flatpak",
            &["flatpak", "run", "org.example.App"]
        )));
        assert!(!app.matches(&process(
            2,
            0,
            "/usr/bin/flatpak",
            &["flatpak", "run", "org.other.App"]
        )));
        let mut scoped = process(3, 0, "/app/bin/program", &["program"]);
        scoped.cgroup = "0::/user.slice/app-flatpak-org.example.App-123.scope".into();
        assert!(app.matches(&scoped));
    }
    #[test]
    fn hidden_and_terminal_launchers_are_not_desktop_apps() {
        for flag in ["Hidden", "NoDisplay", "Terminal"] {
            assert!(parse_desktop(
                "hidden.desktop",
                &format!(
                    "[Desktop Entry]\nType=Application\nName=Hidden\nExec=service\n{flag}=true\n"
                )
            )
            .is_none());
        }
        let app=parse_desktop("quoted.desktop","[Desktop Entry]\nType=Application\nName=Quoted\nExec=env MODE=desktop \"/opt/My App/program\" %F\n[Desktop Action Other]\nExec=wrong\n").unwrap();
        assert_eq!(app.executable, "/opt/My App/program");
    }

    #[test]
    fn launch_modes_and_android_apps_do_not_match_each_other() {
        let settings = parse_desktop(
            "terminal-settings.desktop",
            "[Desktop Entry]\nType=Application\nName=Terminal Settings\nExec=cterm --settings\n",
        )
        .unwrap();
        assert!(!settings.matches(&process(1, 0, "/usr/bin/cterm", &["cterm"])));
        assert!(settings.matches(&process(2, 0, "/usr/bin/cterm", &["cterm", "--settings"])));
        let android=parse_desktop("android.desktop","[Desktop Entry]\nType=Application\nName=Clock\nExec=waydroid app launch com.android.deskclock\n").unwrap();
        assert!(android.matches(&process(
            3,
            0,
            "/usr/bin/waydroid",
            &["waydroid", "app", "launch", "com.android.deskclock"]
        )));
        assert!(!android.matches(&process(
            4,
            0,
            "/usr/bin/waydroid",
            &["waydroid", "app", "launch", "com.android.settings"]
        )));
    }

    #[test]
    fn application_families_never_absorb_another_users_processes() {
        let app = parse_desktop(
            "browser.desktop",
            "[Desktop Entry]\nType=Application\nName=Browser\nExec=browser\n",
        )
        .unwrap();
        let mut other_user = process(2, 1, "/usr/lib/renderer", &["renderer"]);
        other_user.user = "1001".into();
        let apps = group(
            &[process(1, 0, "/usr/bin/browser", &["browser"]), other_user],
            &[app],
            "1000",
        );
        assert_eq!(apps[0].processes.len(), 1);
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;

    #[test]
    fn executable_families_are_refreshed_and_group_helpers() {
        let user = crate::system::current_user();
        let mut root = Process::default();
        root.pid = 10;
        root.executable = r"C:\Apps\Example.exe".into();
        root.name = "Example.exe".into();
        root.arguments = vec![r"C:\Apps\Example.exe".into()];
        root.user = user.clone();
        let mut helper = Process::default();
        helper.pid = 11;
        helper.parent_pid = 10;
        helper.executable = r"C:\Apps\Helper.exe".into();
        helper.name = "Helper.exe".into();
        helper.user = user.clone();
        let catalog = load_catalog(&[root.clone(), helper.clone()]);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].name, "Example");
        let applications = group(&[root, helper.clone()], &catalog, &user);
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].processes.len(), 2);
        let refreshed = load_catalog(&[helper]);
        assert_eq!(refreshed.len(), 1);
        assert_eq!(refreshed[0].name, "Helper");
    }
}
