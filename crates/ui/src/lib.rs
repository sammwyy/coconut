pub mod about;

/// Builds an About page with the invoking package's Cargo metadata.
/// `env!("CARGO_PKG_*")` expands at the call site, so each application
/// shows its own version, description, authors, license and repository.
#[macro_export]
macro_rules! about {
    ($display_name:expr, $icon:expr) => {
        $crate::about::build(
            $display_name,
            $icon,
            $crate::about::Metadata {
                version: env!("CARGO_PKG_VERSION"),
                description: env!("CARGO_PKG_DESCRIPTION"),
                authors: env!("CARGO_PKG_AUTHORS"),
                license: env!("CARGO_PKG_LICENSE"),
                repository: env!("CARGO_PKG_REPOSITORY"),
            },
        )
    };
}
