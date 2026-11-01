use crate::{installed_applications, DesktopEntry, IconRequest, IconResolver, XdgPaths};
use creamui_image::ImageData;
use creamui_reactive::Signal;
use creamui_render::AppHandle;
use std::{collections::BTreeSet, process::Command, thread};

#[derive(Clone)]
pub struct AppCatalog {
    entries: Signal<Vec<AppEntry>>,
    loaded: Signal<bool>,
    revision: std::rc::Rc<std::cell::Cell<u64>>,
    theme: std::rc::Rc<std::cell::RefCell<Option<String>>>,
}

#[derive(Clone)]
pub struct AppEntry {
    pub desktop_file: Option<std::path::PathBuf>,
    pub name: String,
    pub exec: Vec<String>,
    pub terminal: bool,
    pub category: String,
    pub icon_name: Option<String>,
    pub icon: Option<ImageData>,
}

impl AppCatalog {
    pub fn new() -> Self {
        Self {
            entries: Signal::new(Vec::new()),
            loaded: Signal::new(false),
            revision: Default::default(),
            theme: Default::default(),
        }
    }

    pub fn from_entries(entries: Vec<AppEntry>) -> Self {
        Self {
            entries: Signal::new(entries),
            loaded: Signal::new(true),
            revision: Default::default(),
            theme: Default::default(),
        }
    }

    pub fn set_icon_theme(&self, app: &AppHandle, icon_theme: String) {
        if self.theme.borrow().as_ref() != Some(&icon_theme) {
            self.start_loading(app, icon_theme);
        }
    }

    pub fn start_loading(&self, app: &AppHandle, icon_theme: String) {
        *self.theme.borrow_mut() = Some(icon_theme.clone());
        let token = self.revision.get().wrapping_add(1);
        self.revision.set(token);
        let revision = self.revision.clone();
        let loaded = self.loaded.clone();
        let after_scan = self.entries.clone();
        let icon_app = app.clone();
        app.spawn_background(load_apps, move |apps: Vec<AppEntry>| {
            if revision.get() != token {
                return;
            }
            loaded.set(true);
            after_scan.set(apps.clone());
            let after_icons = after_scan.clone();
            icon_app.spawn_background(
                move || resolve_icons(apps, &icon_theme),
                move |resolved| {
                    if revision.get() == token {
                        after_icons.set(resolved);
                    }
                },
            );
        });
    }

    pub fn loaded(&self) -> bool {
        self.loaded.get()
    }

    pub fn ensure(ctx: &crate::PluginInitContext) -> Self {
        ctx.shared.get_or_insert_with(|| {
            let catalog = Self::new();
            catalog.start_loading(
                &ctx.app,
                coconut_core::ShellConfig::load().appearance.icon_theme,
            );
            catalog
        })
    }

    pub fn apps(&self) -> Vec<AppEntry> {
        self.entries.get()
    }
}

impl Default for AppCatalog {
    fn default() -> Self {
        Self::new()
    }
}

pub fn app_categories(apps: &[AppEntry]) -> Vec<String> {
    const ORDER: [&str; 13] = [
        "All",
        "Multimedia",
        "Internet",
        "Graphics",
        "Office",
        "Development",
        "Education",
        "Science",
        "Games",
        "Accessories",
        "Settings",
        "System Tools",
        "Other",
    ];
    let present: BTreeSet<_> = apps.iter().map(|app| app.category.as_str()).collect();
    ORDER
        .into_iter()
        .filter(|category| *category == "All" || present.contains(category))
        .map(str::to_owned)
        .collect()
}

pub fn app_matches(app: &AppEntry, query: &str, category: &str) -> bool {
    let query = query.trim().to_lowercase();
    let query_matches = query.is_empty()
        || app.name.to_lowercase().contains(&query)
        || app.category.to_lowercase().contains(&query);
    query_matches && (category == "All" || app.category == category)
}

pub fn launch_app(app: &AppEntry) {
    let Some((program, args)) = app
        .exec
        .split_first()
        .map(|(program, args)| (program.clone(), args.to_vec()))
    else {
        return;
    };
    let terminal = app.terminal;
    thread::spawn(move || {
        let mut command = if terminal {
            coconut_core::terminal_command(&program, &args)
        } else {
            let mut command = Command::new(program);
            command.args(args);
            command
        };
        if let Err(error) = coconut_core::spawn_detached(&mut command) {
            eprintln!("applications: failed to launch {command:?}: {error}");
        }
    });
}

fn load_apps() -> Vec<AppEntry> {
    #[cfg(target_os = "windows")]
    {
        return load_windows_apps();
    }
    #[cfg(not(target_os = "windows"))]
    load_linux_apps()
}

#[cfg(not(target_os = "windows"))]
fn load_linux_apps() -> Vec<AppEntry> {
    let mut apps: Vec<_> = installed_applications(&XdgPaths::default())
        .into_iter()
        .filter_map(app_from_desktop_entry)
        .collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

#[cfg(target_os = "windows")]
fn load_windows_apps() -> Vec<AppEntry> {
    let script = "Get-StartApps | ForEach-Object { \"$($_.Name)`t$($_.AppID)\" }";
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
        .unwrap_or_default();
    let mut apps = output
        .lines()
        .filter_map(|line| {
            let (name, app_id) = line.split_once('\t')?;
            let name = name.trim();
            let app_id = app_id.trim();
            (!name.is_empty() && !app_id.is_empty()).then(|| AppEntry {
                desktop_file: None,
                name: name.to_owned(),
                exec: vec!["explorer.exe".into(), format!("shell:AppsFolder\\{app_id}")],
                terminal: false,
                category: "System Tools".into(),
                icon_name: None,
                icon: None,
            })
        })
        .collect::<Vec<_>>();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

fn resolve_icons(mut apps: Vec<AppEntry>, icon_theme: &str) -> Vec<AppEntry> {
    let icons = IconResolver::new(icon_theme);
    for app in &mut apps {
        app.icon = app
            .icon_name
            .as_deref()
            .and_then(|name| icons.load(IconRequest::named(name, 48)));
    }
    apps
}

pub fn app_from_desktop_entry(entry: DesktopEntry) -> Option<AppEntry> {
    if !entry.is_visible_application() {
        return None;
    }
    let name = entry
        .name(std::env::var("LANG").ok().as_deref())?
        .to_owned();
    let exec = entry.command_for_files(&[])?;
    Some(AppEntry {
        desktop_file: Some(entry.path().to_owned()),
        name,
        exec,
        terminal: entry.terminal(),
        category: category_for(&entry.categories()),
        icon_name: entry.icon().map(str::to_owned),
        icon: None,
    })
}

fn category_for(categories: &[&str]) -> String {
    if categories
        .iter()
        .any(|category| matches!(*category, "AudioVideo" | "Audio" | "Video"))
    {
        "Multimedia"
    } else if categories.contains(&"Network") {
        "Internet"
    } else if categories.contains(&"Development") {
        "Development"
    } else if categories.contains(&"Graphics") {
        "Graphics"
    } else if categories.contains(&"Office") {
        "Office"
    } else if categories.contains(&"Education") {
        "Education"
    } else if categories.contains(&"Science") {
        "Science"
    } else if categories.contains(&"Game") {
        "Games"
    } else if categories.contains(&"Settings") {
        "Settings"
    } else if categories.contains(&"System") {
        "System Tools"
    } else if categories.contains(&"Utility") {
        "Accessories"
    } else {
        "Other"
    }
    .into()
}
