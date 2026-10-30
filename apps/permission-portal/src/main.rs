//! coconut-permission-portal
//!
//! Modes:
//!   --service            session daemon: org.coconut.Permissions1 plus
//!                        org.freedesktop.impl.portal.Access
//!   --prompt <id>        consent dialog for one pending request
//!   --list               dump stored permissions
//!   --set <s> <p> <v>    store a verdict (allow|deny)
//!   --revoke <s> <p>     drop a stored verdict
//!   --forget <s>         drop every verdict for a subject

use coconut_permission_portal::{daemon, prompt};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.first().map(String::as_str) {
        Some("--service") => service(),
        Some("--prompt") => arguments
            .get(1)
            .ok_or_else(|| "--prompt needs a request id".to_string())
            .and_then(|id| prompt::prompt(id)),
        Some("--list") => list(),
        Some("--set") => three(&arguments[1..]).and_then(|[subject, permission, verdict]| {
            manage("set", subject, permission, verdict)
        }),
        Some("--revoke") => two(&arguments[1..]).and_then(|[subject, permission]| {
            manage("revoke", subject, permission, String::new())
        }),
        Some("--forget") => one(&arguments[1..]).and_then(|subject| {
            manage("forget", subject, String::new(), String::new())
        }),
        _ => Err(
            "usage: coconut-permission-portal --service | --prompt <id> | --list | --set <subject> <permission> <allow|deny> | --revoke <subject> <permission> | --forget <subject>"
                .to_string(),
        ),
    };
    if let Err(error) = result {
        eprintln!("coconut-permission-portal: {error}");
        std::process::exit(1);
    }
}

fn service() -> Result<(), String> {
    let runtime =
        tokio::runtime::Runtime::new().map_err(|error| format!("tokio runtime failed: {error}"))?;
    runtime.block_on(daemon::serve())
}

fn dbus_error(error: impl std::fmt::Display) -> String {
    format!("{error}")
}

fn proxy_call<T>(
    call: impl FnOnce(&daemon::PermissionsProxyBlocking<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let connection = zbus::blocking::Connection::session()
        .map_err(|error| format!("session bus connect failed: {error}"))?;
    let proxy = daemon::PermissionsProxyBlocking::new(&connection)
        .map_err(|error| format!("permissions proxy failed: {error}"))?;
    call(&proxy)
}

fn list() -> Result<(), String> {
    let entries = proxy_call(|proxy| proxy.list().map_err(dbus_error))?;
    for (subject, permission, verdict, granted_at) in entries {
        println!("{verdict}\t{subject}\t{permission}\t{granted_at}");
    }
    Ok(())
}

fn one(arguments: &[String]) -> Result<String, String> {
    arguments
        .first()
        .cloned()
        .ok_or_else(|| "missing argument".to_string())
}

fn two(arguments: &[String]) -> Result<[String; 2], String> {
    match arguments {
        [a, b, ..] => Ok([a.clone(), b.clone()]),
        _ => Err("missing arguments".to_string()),
    }
}

fn three(arguments: &[String]) -> Result<[String; 3], String> {
    match arguments {
        [a, b, c, ..] => Ok([a.clone(), b.clone(), c.clone()]),
        _ => Err("missing arguments".to_string()),
    }
}

fn manage(
    operation: &str,
    subject: String,
    permission: String,
    verdict: String,
) -> Result<(), String> {
    let applied = proxy_call(|proxy| {
        proxy
            .manage(operation.into(), subject, permission, verdict)
            .map_err(dbus_error)
    })?;
    if applied {
        Ok(())
    } else {
        Err("Permission change cancelled".into())
    }
}
