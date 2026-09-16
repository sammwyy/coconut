use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserProfile {
    pub first_name: String,
    pub last_name: String,
    pub city: String,
    pub region: String,
    pub country: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl UserProfile {
    pub fn load() -> Self {
        std::fs::read_to_string(profile_path())
            .ok()
            .and_then(|contents| toml::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = profile_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            path,
            toml::to_string_pretty(self)
                .map_err(|error| std::io::Error::other(error.to_string()))?,
        )
    }

    pub fn display_name(&self) -> String {
        [self.first_name.trim(), self.last_name.trim()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn location(&self) -> String {
        [self.city.trim(), self.region.trim(), self.country.trim()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn profile_path() -> std::path::PathBuf {
    super::config_dir().join("coconut").join("profile.toml")
}
