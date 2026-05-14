pub mod drwa_interactor_config;
pub mod drwa_interactor_state;

use drwa_asset_manager::drwa_asset_manager_proxy;
use drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy;
use drwa_policy_registry::drwa_policy_registry_proxy;

pub use drwa_interactor_config::Config;
use drwa_interactor_state::State;

use multiversx_sc_snippets::imports::*;
use multiversx_sc_snippets::sdk::gateway::SetStateAccount;
use std::collections::HashMap;

const IDENTITY_REGISTRY_CODE: MxscPath =
    MxscPath::new("../identity-registry/output/drwa-identity-registry.mxsc.json");
const POLICY_REGISTRY_CODE: MxscPath =
    MxscPath::new("../policy-registry/output/drwa-policy-registry.mxsc.json");
const ASSET_MANAGER_CODE: MxscPath =
    MxscPath::new("../asset-manager/output/drwa-asset-manager.mxsc.json");
const ATTESTATION_CODE: MxscPath =
    MxscPath::new("../attestation/output/drwa-attestation.mxsc.json");
const AUTH_ADMIN_CODE: MxscPath =
    MxscPath::new("../drwa-auth-admin/output/drwa-auth-admin.mxsc.json");

const DEPLOY_GAS: u64 = 100_000_000;
const CALL_GAS: u64 = 30_000_000;
const DRWA_SYSTEM_ACCOUNT: &str = "erd1lllllllllllllllllllllllllllllllllllllllllllllllllllsckry7t";
const DRWA_DOMAIN_POLICY_REGISTRY: &str = "policy_registry";
const DRWA_DOMAIN_ASSET_MANAGER: &str = "asset_manager";
const DRWA_DOMAIN_IDENTITY_REGISTRY: &str = "identity_registry";
const DRWA_DOMAIN_ATTESTATION: &str = "attestation";
const DRWA_DOMAIN_AUTH_ADMIN: &str = "auth_admin";

pub struct DrwaInteractor {
    pub interactor: Interactor,
    pub owner_address: Bech32Address,
    pub governance_address: Bech32Address,
    pub auditor_address: Bech32Address,
    pub holder_shard0_address: Bech32Address,
    pub holder_shard1_address: Bech32Address,
    pub holder_extra_address: Bech32Address,
    pub holder_extra_alt_address: Bech32Address,
    pub holder_extra_cross_shard0_address: Bech32Address,
    pub holder_extra_cross_shard1_address: Bech32Address,
    pub holder_extra_governance_address: Bech32Address,
    pub state: State,
}

impl DrwaInteractor {
    pub async fn new(config: Config) -> Self {
        let mut interactor = Interactor::new(config.gateway_uri())
            .await
            .use_chain_simulator(config.use_chain_simulator());
        interactor.current_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let owner_address = interactor.register_wallet(test_wallets::mike()).await;
        let governance_address = interactor.register_wallet(test_wallets::ivan()).await;
        let auditor_address = interactor.register_wallet(test_wallets::carol()).await;
        let holder_shard0_address = interactor.register_wallet(test_wallets::alice()).await;
        let holder_shard1_address = interactor.register_wallet(test_wallets::bob()).await;
        let holder_extra_address = interactor.register_wallet(test_wallets::dan()).await;
        let holder_extra_alt_address = interactor.register_wallet(test_wallets::eve()).await;
        let holder_extra_cross_shard0_address =
            interactor.register_wallet(test_wallets::frank()).await;
        let holder_extra_cross_shard1_address =
            interactor.register_wallet(test_wallets::grace()).await;
        let holder_extra_governance_address =
            interactor.register_wallet(test_wallets::heidi()).await;

        interactor.generate_blocks(30u64).await.unwrap();

        DrwaInteractor {
            interactor,
            owner_address: owner_address.into(),
            governance_address: governance_address.into(),
            auditor_address: auditor_address.into(),
            holder_shard0_address: holder_shard0_address.into(),
            holder_shard1_address: holder_shard1_address.into(),
            holder_extra_address: holder_extra_address.into(),
            holder_extra_alt_address: holder_extra_alt_address.into(),
            holder_extra_cross_shard0_address: holder_extra_cross_shard0_address.into(),
            holder_extra_cross_shard1_address: holder_extra_cross_shard1_address.into(),
            holder_extra_governance_address: holder_extra_governance_address.into(),
            state: State::load_state(),
        }
    }

    /// Deploy all four DRWA contracts in dependency order.
    pub async fn deploy_all(&mut self) {
        // 1. Deploy identity-registry (init takes governance: ManagedAddress)
        let identity_addr = self
            .interactor
            .tx()
            .from(&self.governance_address)
            .gas(DEPLOY_GAS)
            .typed(DrwaIdentityRegistryProxy)
            .init(self.governance_address.to_address())
            .code(IDENTITY_REGISTRY_CODE)
            .returns(ReturnsNewBech32Address)
            .run()
            .await;

        println!("identity-registry deployed at: {identity_addr}");
        self.state.set_identity_registry_address(identity_addr);
        self.generate_blocks(5).await;

        // 2. Deploy policy-registry (init takes governance: ManagedAddress)
        let policy_addr = self
            .interactor
            .tx()
            .from(&self.governance_address)
            .gas(DEPLOY_GAS)
            .typed(drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
            .init(self.governance_address.to_address())
            .code(POLICY_REGISTRY_CODE)
            .returns(ReturnsNewBech32Address)
            .run()
            .await;

        println!("policy-registry deployed at: {policy_addr}");
        self.state.set_policy_registry_address(policy_addr);
        self.generate_blocks(5).await;

        // 3. Deploy asset-manager (init takes governance: ManagedAddress)
        let asset_addr = self
            .interactor
            .tx()
            .from(&self.governance_address)
            .gas(DEPLOY_GAS)
            .typed(drwa_asset_manager_proxy::DrwaAssetManagerProxy)
            .init(self.governance_address.to_address())
            .code(ASSET_MANAGER_CODE)
            .returns(ReturnsNewBech32Address)
            .run()
            .await;

        println!("asset-manager deployed at: {asset_addr}");
        self.state.set_asset_manager_address(asset_addr.clone());
        self.generate_blocks(5).await;

        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&asset_addr)
            .gas(CALL_GAS)
            .typed(drwa_asset_manager_proxy::DrwaAssetManagerProxy)
            .set_policy_registry_address(self.state.current_policy_registry_address().to_address())
            .run()
            .await;
        self.generate_blocks(3).await;

        // 4. Deploy attestation (init takes auditor: ManagedAddress)
        let attestation_addr = self
            .interactor
            .tx()
            .from(&self.governance_address)
            .gas(DEPLOY_GAS)
            .raw_deploy()
            .argument(&self.auditor_address.to_address())
            .code(ATTESTATION_CODE)
            .returns(ReturnsNewBech32Address)
            .run()
            .await;

        println!("attestation deployed at: {attestation_addr}");
        self.state.set_attestation_address(attestation_addr);
        self.generate_blocks(5).await;

        self.provision_sync_authorized_callers(&[
            (
                DRWA_DOMAIN_IDENTITY_REGISTRY,
                self.state.current_identity_registry_address(),
            ),
            (
                DRWA_DOMAIN_POLICY_REGISTRY,
                self.state.current_policy_registry_address(),
            ),
            (
                DRWA_DOMAIN_ASSET_MANAGER,
                self.state.current_asset_manager_address(),
            ),
            (
                DRWA_DOMAIN_ATTESTATION,
                self.state.current_attestation_address(),
            ),
        ])
        .await;

        println!("all DRWA contracts deployed successfully");
    }

    /// Deploy the drwa-auth-admin multisig contract.
    pub async fn deploy_auth_admin(&mut self) {
        let auth_admin_addr = self
            .interactor
            .tx()
            .from(&self.governance_address)
            .gas(DEPLOY_GAS)
            .raw_deploy()
            .argument(&2u64)
            .argument(&100u64)
            .argument(&self.owner_address.to_address())
            .argument(&self.governance_address.to_address())
            .argument(&self.auditor_address.to_address())
            .code(AUTH_ADMIN_CODE)
            .returns(ReturnsNewBech32Address)
            .run()
            .await;

        println!("drwa-auth-admin deployed at: {auth_admin_addr}");
        self.state.set_auth_admin_address(auth_admin_addr);
        self.generate_blocks(5).await;

        self.provision_sync_authorized_callers(&[(
            DRWA_DOMAIN_AUTH_ADMIN,
            self.state.current_auth_admin_address(),
        )])
        .await;
    }

    /// Set up a holder as compliant for a given token: register identity,
    /// approve KYC/AML, register the asset, sync holder compliance with
    /// transfer unlocked, and set a permissive token policy.
    pub async fn setup_compliant_holder(&mut self, token_id: &str, holder_address: &Bech32Address) {
        let identity_registry = self.state.current_identity_registry_address().clone();
        let asset_manager = self.state.current_asset_manager_address().clone();
        let policy_registry = self.state.current_policy_registry_address().clone();

        // 1. Register identity via identity-registry (from governance)
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&identity_registry)
            .gas(CALL_GAS)
            .raw_call("registerIdentity")
            .argument(&holder_address.to_address())
            .argument(&"Compliant Holder")
            .argument(&"US")
            .argument(&"REG-001")
            .argument(&"individual")
            .run()
            .await;

        println!("registered identity for {holder_address}");
        self.generate_blocks(3).await;

        // 2. Update compliance to approved (from governance)
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&identity_registry)
            .gas(CALL_GAS)
            .raw_call("updateComplianceStatus")
            .argument(&holder_address.to_address())
            .argument(&"approved")
            .argument(&"clear")
            .argument(&"qualified")
            .argument(&0u64)
            .run()
            .await;

        println!("compliance approved for {holder_address}");
        self.generate_blocks(3).await;

        // 3. Set token policy allowing transfers (from governance)
        let empty_vec: Vec<String> = Vec::new();
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&policy_registry)
            .gas(CALL_GAS)
            .typed(drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
            .set_token_policy(
                token_id,
                true,              // drwa_enabled
                false,             // global_pause
                false,             // strict_auditor_mode
                false,             // metadata_protection_enabled
                empty_vec.clone(), // allowed_investor_classes (empty = all)
                empty_vec,         // allowed_jurisdictions (empty = all)
            )
            .run()
            .await;

        println!("token policy set for {token_id} (transfers enabled)");
        self.generate_blocks(3).await;

        self.ensure_asset_registered(token_id).await;

        // 4. Sync holder compliance — approved, unlocked (from governance)
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&asset_manager)
            .gas(CALL_GAS)
            .typed(drwa_asset_manager_proxy::DrwaAssetManagerProxy)
            .sync_holder_compliance(
                token_id,
                holder_address.to_address(),
                "approved",  // kyc_status
                "clear",     // aml_status
                "qualified", // investor_class
                "US",        // jurisdiction_code
                0u64,        // expiry_round (permanent)
                false,       // transfer_locked
                false,       // receive_locked
                false,       // auditor_authorized
            )
            .run()
            .await;

        println!("holder compliance synced for {holder_address} on {token_id}");
        self.generate_blocks(3).await;
    }

    /// Registers an asset if it is not already registered. Idempotent.
    pub async fn ensure_asset_registered(&mut self, token_id: &str) {
        let asset_manager = self.state.current_asset_manager_address().clone();

        let result: Result<_, _> = self
            .interactor
            .tx()
            .from(&self.governance_address)
            .to(&asset_manager)
            .gas(CALL_GAS)
            .typed(drwa_asset_manager_proxy::DrwaAssetManagerProxy)
            .register_asset(token_id, "fungible", "security", "policy-001")
            .returns(ReturnsHandledOrError::new())
            .run()
            .await;

        match result {
            Ok(_) => {
                println!("asset {token_id} registered");
            }
            Err(tx_err) => {
                if tx_err.message.contains("asset already registered") {
                    println!("asset {token_id} already registered, skipping");
                } else {
                    panic!(
                        "unexpected error registering asset {token_id}: {}",
                        tx_err.message
                    );
                }
            }
        }
        self.generate_blocks(3).await;
    }

    /// Set up a holder as non-compliant / blocked for a given token: register
    /// identity with AML blocked, sync holder compliance with transfer locked.
    pub async fn setup_blocked_holder(&mut self, token_id: &str, holder_address: &Bech32Address) {
        let identity_registry = self.state.current_identity_registry_address().clone();
        let asset_manager = self.state.current_asset_manager_address().clone();
        let policy_registry = self.state.current_policy_registry_address().clone();

        // 1. Register identity via identity-registry (from governance)
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&identity_registry)
            .gas(CALL_GAS)
            .raw_call("registerIdentity")
            .argument(&holder_address.to_address())
            .argument(&"Blocked Holder")
            .argument(&"XX")
            .argument(&"REG-BLOCKED")
            .argument(&"individual")
            .run()
            .await;

        println!("registered identity for blocked holder {holder_address}");
        self.generate_blocks(3).await;

        // 2. Update compliance to blocked AML (from governance)
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&identity_registry)
            .gas(CALL_GAS)
            .raw_call("updateComplianceStatus")
            .argument(&holder_address.to_address())
            .argument(&"approved")
            .argument(&"blocked")
            .argument(&"none")
            .argument(&0u64)
            .run()
            .await;

        println!("AML blocked for {holder_address}");
        self.generate_blocks(3).await;

        // 3. Set token policy before asset registration.
        let empty_vec: Vec<String> = Vec::new();
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&policy_registry)
            .gas(CALL_GAS)
            .typed(drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
            .set_token_policy(
                token_id,
                true,  // drwa_enabled
                false, // global_pause
                false, // strict_auditor_mode
                true,  // metadata_protection_enabled
                empty_vec.clone(),
                empty_vec,
            )
            .run()
            .await;

        println!("token policy set for {token_id} (blocked-holder path)");
        self.generate_blocks(3).await;

        // 4. Register asset (idempotent — required before syncing holder compliance)
        self.ensure_asset_registered(token_id).await;

        // 5. Sync holder compliance — blocked, transfer locked (from governance)
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&asset_manager)
            .gas(CALL_GAS)
            .typed(drwa_asset_manager_proxy::DrwaAssetManagerProxy)
            .sync_holder_compliance(
                token_id,
                holder_address.to_address(),
                "approved", // kyc_status
                "blocked",  // aml_status
                "none",     // investor_class
                "XX",       // jurisdiction_code
                0u64,       // expiry_round (permanent)
                true,       // transfer_locked
                true,       // receive_locked
                false,      // auditor_authorized
            )
            .run()
            .await;

        println!("holder compliance synced (blocked) for {holder_address} on {token_id}");
        self.generate_blocks(3).await;
    }

    /// Generate blocks on the chain simulator.
    pub async fn generate_blocks(&self, num_blocks: u64) {
        self.interactor.generate_blocks(num_blocks).await.unwrap();
    }

    /// Query the governance address from a DRWA contract.
    pub async fn query_governance(&mut self, contract_address: &Bech32Address) -> Bech32Address {
        let result: Address = self
            .interactor
            .query()
            .to(contract_address)
            .typed(DrwaIdentityRegistryProxy)
            .governance()
            .returns(ReturnsResultUnmanaged)
            .run()
            .await;

        result.into()
    }

    /// Query whether an identity exists for a subject on the identity registry.
    pub async fn query_identity_exists(&mut self, subject: &Bech32Address) -> bool {
        let identity_registry = self.state.current_identity_registry_address().clone();

        let result: Result<drwa_identity_registry::IdentityRecord<StaticApi>, _> = self
            .interactor
            .query()
            .to(&identity_registry)
            .typed(DrwaIdentityRegistryProxy)
            .identity(subject.to_address())
            .returns(ReturnsHandledOrError::new().returns(ReturnsResultUnmanaged))
            .run()
            .await;

        result.is_ok()
    }

    async fn provision_sync_authorized_callers(&self, entries: &[(&str, &Bech32Address)]) {
        let system_account = Bech32Address::from_bech32_string(DRWA_SYSTEM_ACCOUNT.to_owned());
        let mut storage_pairs = HashMap::with_capacity(entries.len());

        for (domain, address) in entries {
            let key = format!("drwa:auth:{domain}");
            storage_pairs.insert(
                hex_encode(key.as_bytes()),
                hex_encode(address.to_address().as_bytes()),
            );
        }

        self.interactor
            .set_state(vec![
                SetStateAccount::from_address(system_account.to_bech32_string())
                    .with_storage(storage_pairs),
            ])
            .await
            .expect("failed to provision DRWA authorized callers in chain simulator");
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}
