use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Launch {
    Manager(Option<PathBuf>),
    Settings,
    Portal,
    Help,
    Version,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Launch, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Launch::Manager(None));
    };
    let launch = if first == "--portal" {
        Launch::Portal
    } else if first == "--settings" || first == "--setings" {
        Launch::Settings
    } else if first == "--help" || first == "-h" {
        Launch::Help
    } else if first == "--version" || first == "-V" {
        Launch::Version
    } else if first == "--" {
        Launch::Manager(args.next().map(PathBuf::from))
    } else if first.to_string_lossy().starts_with('-') {
        return Err(format!("Unknown option: {}", first.to_string_lossy()));
    } else if first.to_string_lossy().starts_with("file:") {
        let path = first
            .to_str()
            .and_then(|value| url::Url::parse(value).ok())
            .and_then(|url| url.to_file_path().ok())
            .ok_or("Expected a local file URI")?;
        Launch::Manager(Some(path))
    } else {
        Launch::Manager(Some(PathBuf::from(first)))
    };
    if args.next().is_some() {
        return Err("Expected one folder, or --portal without a folder".into());
    }
    Ok(launch)
}

pub const HELP: &str = "Usage: cfiles [FOLDER | file://URI]\n       cfiles --settings\n       cfiles --portal\n\n  --settings   Open File Explorer settings (--setings is also accepted)\n  --portal     Run the D-Bus file chooser backend\n  --help       Show this help\n  --version    Show the version\n  -- PATH      Open a path that starts with a dash";

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }
    #[test]
    fn selects_one_launch_mode() {
        assert_eq!(parse(args(&[])).unwrap(), Launch::Manager(None));
        assert_eq!(parse(args(&["--portal"])).unwrap(), Launch::Portal);
        assert_eq!(parse(args(&["--settings"])).unwrap(), Launch::Settings);
        assert_eq!(parse(args(&["--setings"])).unwrap(), Launch::Settings);
        assert_eq!(
            parse(args(&["file:///tmp/a%20b"])).unwrap(),
            Launch::Manager(Some("/tmp/a b".into()))
        );
        assert_eq!(
            parse(args(&["--", "--portal"])).unwrap(),
            Launch::Manager(Some("--portal".into()))
        );
        assert!(parse(args(&["--portal", "/tmp"])).is_err());
        assert!(parse(args(&["--unknown"])).is_err());
    }
}
