use std::{ffi::OsString, path::PathBuf};
pub const HELP:&str="Terminal\n\nUsage: cterm [--working-directory DIR] [-e COMMAND [ARGS...]]\n       cterm --settings\n       cterm --export-config FILE\n       cterm --import-config FILE\n\n  --settings                Open settings\n  --working-directory DIR   Start shells in DIR\n  -e, --command COMMAND ... Run a program instead of the default shell\n  --export-config FILE      Write the current settings as TOML\n  --import-config FILE      Validate and install settings from TOML\n  -h, --help                Show this help\n  -V, --version             Show the version\n\nDefault shortcuts (configurable in --settings):\n           Ctrl+Shift+T new tab, Ctrl+Shift+N new window,\n           Ctrl+Shift+W close tab, Ctrl+Tab next tab,\n           Ctrl+Shift+C/V copy/paste, Shift+PageUp/PageDown scrollback.";
#[derive(Debug, PartialEq)]
pub enum Launch {
    Terminal { cwd: PathBuf, command: Vec<String> },
    Settings,
    ExportConfig(PathBuf),
    ImportConfig(PathBuf),
    Help,
    Version,
}
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Launch, String> {
    let mut args = args.into_iter();
    let mut cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut command = Vec::new();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--settings") => {
                if args.next().is_some() {
                    return Err("--settings doesn't accept other arguments".into());
                }
                return Ok(Launch::Settings);
            }
            Some("--export-config") => {
                let path =
                    PathBuf::from(args.next().ok_or("--export-config requires a file path")?);
                if args.next().is_some() {
                    return Err("--export-config doesn't accept other arguments".into());
                }
                return Ok(Launch::ExportConfig(path));
            }
            Some("--import-config") => {
                let path =
                    PathBuf::from(args.next().ok_or("--import-config requires a file path")?);
                if args.next().is_some() {
                    return Err("--import-config doesn't accept other arguments".into());
                }
                return Ok(Launch::ImportConfig(path));
            }
            Some("--help" | "-h") => return Ok(Launch::Help),
            Some("--version" | "-V") => return Ok(Launch::Version),
            Some("--working-directory") => {
                cwd = PathBuf::from(
                    args.next()
                        .ok_or("--working-directory requires a directory")?,
                )
            }
            Some("--command" | "-e" | "--") => {
                command = args
                    .map(|s| {
                        s.into_string()
                            .map_err(|_| "command arguments must be UTF-8".to_string())
                    })
                    .collect::<Result<_, _>>()?;
                if command.is_empty() {
                    return Err("expected a command".into());
                }
                break;
            }
            _ => {
                return Err(format!(
                    "Unknown argument: {}. Use --help.",
                    arg.to_string_lossy()
                ))
            }
        }
    }
    if !cwd.is_dir() {
        return Err(format!("Not a directory: {}", cwd.display()));
    }
    Ok(Launch::Terminal { cwd, command })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn p(args: &[&str]) -> Result<Launch, String> {
        parse(args.iter().map(OsString::from))
    }
    #[test]
    fn settings_and_command_arguments() {
        assert_eq!(p(&["--settings"]).unwrap(), Launch::Settings);
        assert!(p(&["--settings", "x"]).is_err());
        assert_eq!(
            p(&["--export-config", "/tmp/settings.toml"]).unwrap(),
            Launch::ExportConfig(PathBuf::from("/tmp/settings.toml"))
        );
        assert_eq!(
            p(&["--import-config", "/tmp/settings.toml"]).unwrap(),
            Launch::ImportConfig(PathBuf::from("/tmp/settings.toml"))
        );
        assert!(p(&["--export-config"]).is_err());
        assert!(p(&["--import-config", "a", "b"]).is_err());
        assert!(p(&["-e"]).is_err());
        assert!(p(&["--working-directory"]).is_err());
        assert!(p(&["--invalid"]).is_err());
        assert!(
            matches!(p(&["-e","echo","--settings"]).unwrap(),Launch::Terminal{command,..}if command==["echo","--settings"])
        );
    }
}
