use serde::Deserialize;
use std::io::Read;

/// Config file
const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainType {
    Real,
    Simulator,
}

/// DRWA Interactor configuration
#[derive(Debug, Deserialize)]
pub struct Config {
    pub gateway_uri: String,
    pub chain_type: ChainType,
}

impl Config {
    /// Deserializes config from the TOML file at `CONFIG_FILE`.
    pub fn load_config() -> Self {
        Self::try_load_config().unwrap_or_else(|err| panic!("{err}"))
    }

    /// Fallible config loader for callers that want to surface operator-facing
    /// diagnostics instead of panicking on malformed local state.
    pub fn try_load_config() -> Result<Self, String> {
        let mut file = std::fs::File::open(CONFIG_FILE)
            .map_err(|err| format!("failed to open {CONFIG_FILE}: {err}"))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|err| format!("failed to read {CONFIG_FILE}: {err}"))?;
        toml::from_str(&content).map_err(|err| format!("failed to parse {CONFIG_FILE}: {err}"))
    }

    pub fn chain_simulator_config() -> Self {
        Config {
            gateway_uri: "http://localhost:8085".to_owned(),
            chain_type: ChainType::Simulator,
        }
    }

    /// Returns the gateway URI.
    pub fn gateway_uri(&self) -> &str {
        &self.gateway_uri
    }

    /// Returns `true` when the configured chain type is `Simulator`.
    pub fn use_chain_simulator(&self) -> bool {
        match self.chain_type {
            ChainType::Real => false,
            ChainType::Simulator => true,
        }
    }
}
