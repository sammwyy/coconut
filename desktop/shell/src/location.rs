use coconut_core::ShellConfig;

const AGENT_ID: &str = "org.coconut.LocationAgent";
const AGENT_PATH: &str = "/org/freedesktop/GeoClue2/Agent";

struct LocationAgent;

fn decision(enabled: bool, requested_accuracy: u32) -> (bool, u32) {
    (
        enabled,
        enabled.then_some(requested_accuracy).unwrap_or_default(),
    )
}

#[zbus::interface(name = "org.freedesktop.GeoClue2.Agent")]
impl LocationAgent {
    fn authorize_app(&self, _desktop_id: &str, requested_accuracy: u32) -> (bool, u32) {
        let enabled = ShellConfig::load_existing().location.enabled;
        decision(enabled, requested_accuracy)
    }
}

#[cfg(test)]
mod tests {
    use super::decision;

    #[test]
    fn policy_denies_requests_when_location_is_disabled() {
        assert_eq!(decision(false, 8), (false, 0));
        assert_eq!(decision(true, 8), (true, 8));
    }
}

pub fn start() {
    let _ = std::thread::Builder::new()
        .name("coconut-location-agent".into())
        .spawn(|| {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("location: could not start D-Bus runtime: {error}");
                    return;
                }
            };
            runtime.block_on(async {
                let builder = match zbus::connection::Builder::system() {
                    Ok(builder) => builder,
                    Err(error) => {
                        eprintln!("location: could not prepare the system bus: {error}");
                        return;
                    }
                };
                let builder = match builder.serve_at(AGENT_PATH, LocationAgent) {
                    Ok(builder) => builder,
                    Err(error) => {
                        eprintln!("location: could not export the GeoClue agent: {error}");
                        return;
                    }
                };
                let connection = match builder.build().await {
                    Ok(connection) => connection,
                    Err(error) => {
                        eprintln!("location: could not connect to the system bus: {error}");
                        return;
                    }
                };
                let manager = match zbus::Proxy::new(
                    &connection,
                    "org.freedesktop.GeoClue2",
                    "/org/freedesktop/GeoClue2/Manager",
                    "org.freedesktop.GeoClue2.Manager",
                )
                .await
                {
                    Ok(manager) => manager,
                    Err(error) => {
                        eprintln!("location: GeoClue is unavailable: {error}");
                        return;
                    }
                };
                if let Err(error) = manager.call::<_, _, ()>("AddAgent", &(AGENT_ID,)).await {
                    eprintln!("location: GeoClue rejected Coconut's agent: {error}");
                    return;
                }
                std::future::pending::<()>().await;
            });
        });
}
