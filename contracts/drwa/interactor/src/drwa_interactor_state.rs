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
        if Path::new(STATE_FILE).exists() {
            let mut file = std::fs::File::open(STATE_FILE).unwrap();
            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();
            toml::from_str(&content).unwrap()
        } else {
            Self::default()
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
}

impl Drop for State {
    /// Serializes state to the TOML file at `STATE_FILE` on drop.
    fn drop(&mut self) {
        let mut file = std::fs::File::create(STATE_FILE).unwrap();
        file.write_all(toml::to_string(self).unwrap().as_bytes())
            .unwrap();
    }
}
