use std::env;

/// Deployment environment, selected by `TASKLIST_ENV`. In later tasks this steers the
/// adapters between local emulators (`Local`) and real Azure via managed identity
/// (`Cloud`). For the skeleton it is just the seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    #[default]
    Local,
    Cloud,
}

impl Environment {
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cloud" => Self::Cloud,
            _ => Self::Local,
        }
    }
}

/// Which `TaskRepository` adapter the composition root should wire up, selected by
/// `TASKLIST_PERSISTENCE`. Defaults to the hermetic in-memory fake; `cosmos` opts into the
/// real Cosmos adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Persistence {
    #[default]
    Memory,
    Cosmos,
}

impl Persistence {
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cosmos" => Self::Cosmos,
            _ => Self::Memory,
        }
    }
}

/// Non-secret Cosmos connection settings. The account key is optional: locally it defaults
/// to the well-known emulator key inside the adapter; in the cloud a token credential is
/// used instead and no key is set.
#[derive(Debug, Clone)]
pub struct CosmosSettings {
    /// Account endpoint (`COSMOS__ENDPOINT`), e.g. the emulator's `https://localhost:8081`.
    pub endpoint: String,
    /// Database id (`COSMOS__DATABASE`).
    pub database: String,
    /// Container id (`COSMOS__CONTAINER`).
    pub container: String,
    /// Optional account key (`COSMOS__KEY`); unset means "use the credential / emulator key".
    pub key: Option<String>,
}

/// Default emulator endpoint used when `COSMOS__ENDPOINT` is unset.
const DEFAULT_COSMOS_ENDPOINT: &str = "https://localhost:8081";

impl CosmosSettings {
    /// Load Cosmos settings from the environment, applying local-friendly defaults.
    pub fn from_env() -> Self {
        Self {
            endpoint: env::var("COSMOS__ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_COSMOS_ENDPOINT.to_owned()),
            database: env::var("COSMOS__DATABASE").unwrap_or_else(|_| "tasklist".to_owned()),
            container: env::var("COSMOS__CONTAINER").unwrap_or_else(|_| "tasks".to_owned()),
            key: env::var("COSMOS__KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
        }
    }
}

/// Runtime configuration loaded from the environment. Non-secret only; secrets arrive via
/// Key Vault in the cloud (a later task).
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Selected environment (`TASKLIST_ENV`).
    pub environment: Environment,
    /// Selected persistence adapter (`TASKLIST_PERSISTENCE`).
    pub persistence: Persistence,
    /// Socket address the API binds to (`TASKLIST_BIND_ADDRESS`).
    pub bind_address: String,
    /// Directory containing the built SPA to serve (`TASKLIST_WEB_DIST`).
    pub web_dist_dir: String,
}

impl AppConfig {
    /// Load configuration from environment variables, applying skeleton defaults.
    pub fn from_env() -> Self {
        let environment = env::var("TASKLIST_ENV")
            .map(|v| Environment::parse(&v))
            .unwrap_or_default();
        let persistence = env::var("TASKLIST_PERSISTENCE")
            .map(|v| Persistence::parse(&v))
            .unwrap_or_default();
        let bind_address =
            env::var("TASKLIST_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());
        let web_dist_dir = env::var("TASKLIST_WEB_DIST").unwrap_or_else(|_| "web/dist".to_owned());

        Self {
            environment,
            persistence,
            bind_address,
            web_dist_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cloud_case_insensitively() {
        assert_eq!(Environment::parse("Cloud"), Environment::Cloud);
        assert_eq!(Environment::parse("CLOUD"), Environment::Cloud);
    }

    #[test]
    fn defaults_to_local_for_anything_else() {
        assert_eq!(Environment::parse(""), Environment::Local);
        assert_eq!(Environment::parse("local"), Environment::Local);
        assert_eq!(Environment::parse("prod"), Environment::Local);
    }

    #[test]
    fn persistence_selects_cosmos_case_insensitively() {
        assert_eq!(Persistence::parse("cosmos"), Persistence::Cosmos);
        assert_eq!(Persistence::parse("Cosmos"), Persistence::Cosmos);
        assert_eq!(Persistence::parse(""), Persistence::Memory);
        assert_eq!(Persistence::parse("memory"), Persistence::Memory);
    }
}
