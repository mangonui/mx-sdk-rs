use multiversx_sc_snippets::imports::*;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};

/// State file
const STATE_FILE: &str = "state.toml";

/// DRWA Interactor state — persists deployed contract addresses across runs.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    identity_registry_address: Option<Bech32Address>,
    policy_registry_address: Option<Bech32Address>,
    asset_manager_address: Option<Bech32Address>,
    attestation_address: Option<Bech32Address>,
    auth_admin_address: Option<Bech32Address>,
}

impl State {
    /// Deserializes state from the TOML file at `STATE_FILE`, or returns
    /// default state if the file does not exist.
    pub fn load_state() -> Self {
        Self::try_load_state().unwrap_or_else(|err| panic!("{err}"))
    }

    /// Fallible state loader for tests and operator tooling that want explicit
    /// file/parse errors instead of generic unwrap panics.
    pub fn try_load_state() -> Result<Self, String> {
        if Path::new(STATE_FILE).exists() {
            let mut file = std::fs::File::open(STATE_FILE)
                .map_err(|err| format!("failed to open {STATE_FILE}: {err}"))?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|err| format!("failed to read {STATE_FILE}: {err}"))?;
            toml::from_str(&content).map_err(|err| format!("failed to parse {STATE_FILE}: {err}"))
        } else {
            Ok(Self::default())
        }
    }

    /// Sets the identity registry contract address
    pub fn set_identity_registry_address(&mut self, address: Bech32Address) {
        self.identity_registry_address = Some(address);
    }

    /// Sets the policy registry contract address
    pub fn set_policy_registry_address(&mut self, address: Bech32Address) {
        self.policy_registry_address = Some(address);
    }

    /// Sets the asset manager contract address
    pub fn set_asset_manager_address(&mut self, address: Bech32Address) {
        self.asset_manager_address = Some(address);
    }

    /// Sets the attestation contract address
    pub fn set_attestation_address(&mut self, address: Bech32Address) {
        self.attestation_address = Some(address);
    }

    /// Sets the drwa-auth-admin contract address
    pub fn set_auth_admin_address(&mut self, address: Bech32Address) {
        self.auth_admin_address = Some(address);
    }

    /// Returns the identity registry contract address
    pub fn current_identity_registry_address(&self) -> &Bech32Address {
        self.identity_registry_address
            .as_ref()
            .expect("no known identity-registry contract, deploy first")
    }

    /// Returns the policy registry contract address
    pub fn current_policy_registry_address(&self) -> &Bech32Address {
        self.policy_registry_address
            .as_ref()
            .expect("no known policy-registry contract, deploy first")
    }

    /// Returns the asset manager contract address
    pub fn current_asset_manager_address(&self) -> &Bech32Address {
        self.asset_manager_address
            .as_ref()
            .expect("no known asset-manager contract, deploy first")
    }

    /// Returns the attestation contract address
    pub fn current_attestation_address(&self) -> &Bech32Address {
        self.attestation_address
            .as_ref()
            .expect("no known attestation contract, deploy first")
    }

    /// Returns the drwa-auth-admin contract address
    pub fn current_auth_admin_address(&self) -> &Bech32Address {
        self.auth_admin_address
            .as_ref()
            .expect("no known drwa-auth-admin contract, deploy first")
    }

    pub fn try_save_state(&self) -> Result<(), String> {
        let mut file = std::fs::File::create(STATE_FILE)
            .map_err(|err| format!("failed to create {STATE_FILE}: {err}"))?;
        let content = toml::to_string(self)
            .map_err(|err| format!("failed to serialize {STATE_FILE}: {err}"))?;
        file.write_all(content.as_bytes())
            .map_err(|err| format!("failed to write {STATE_FILE}: {err}"))
    }
}

impl Drop for State {
    /// Serializes state to the TOML file at `STATE_FILE` on drop.
    fn drop(&mut self) {
        if let Err(err) = self.try_save_state() {
            eprintln!("{err}");
        }
    }
}
