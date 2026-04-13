pub mod drwa_interactor_config;
pub mod drwa_interactor_state;

use drwa_asset_manager::drwa_asset_manager_proxy;
use drwa_policy_registry::drwa_policy_registry_proxy;

pub use drwa_interactor_config::Config;
use drwa_interactor_state::State;

use multiversx_sc_snippets::imports::*;

const IDENTITY_REGISTRY_CODE: MxscPath =
    MxscPath::new("mxsc:../identity-registry/output/drwa-identity-registry.mxsc.json");
const POLICY_REGISTRY_CODE: MxscPath =
    MxscPath::new("mxsc:../policy-registry/output/drwa-policy-registry.mxsc.json");
const ASSET_MANAGER_CODE: MxscPath =
    MxscPath::new("mxsc:../asset-manager/output/drwa-asset-manager.mxsc.json");
const ATTESTATION_CODE: MxscPath =
    MxscPath::new("mxsc:../attestation/output/drwa-attestation.mxsc.json");

const DEPLOY_GAS: u64 = 100_000_000;
const CALL_GAS: u64 = 30_000_000;

pub struct DrwaInteractor {
    pub interactor: Interactor,
    pub owner_address: Bech32Address,
    pub governance_address: Bech32Address,
    pub auditor_address: Bech32Address,
    pub holder_shard0_address: Bech32Address,
    pub holder_shard1_address: Bech32Address,
    pub state: State,
}

impl DrwaInteractor {
    pub async fn new(config: Config) -> Self {
        let mut interactor = Interactor::new(config.gateway_uri())
            .await
            .use_chain_simulator(config.use_chain_simulator());
        interactor.set_current_dir_from_workspace("contracts/drwa/interactor");

        let owner_address = interactor.register_wallet(test_wallets::mike()).await;
        let governance_address = interactor.register_wallet(test_wallets::ivan()).await;
        let auditor_address = interactor.register_wallet(test_wallets::carol()).await;
        let holder_shard0_address = interactor.register_wallet(test_wallets::alice()).await;
        let holder_shard1_address = interactor.register_wallet(test_wallets::bob()).await;

        interactor.generate_blocks(30u64).await.unwrap();

        DrwaInteractor {
            interactor,
            owner_address: owner_address.into(),
            governance_address: governance_address.into(),
            auditor_address: auditor_address.into(),
            holder_shard0_address: holder_shard0_address.into(),
            holder_shard1_address: holder_shard1_address.into(),
            state: State::load_state(),
        }
    }

    /// Deploy all four DRWA contracts in dependency order.
    pub async fn deploy_all(&mut self) {
        // 1. Deploy identity-registry (init takes governance: ManagedAddress)
        let identity_addr = self
            .interactor
            .tx()
            .from(&self.owner_address)
            .gas(DEPLOY_GAS)
            .raw_deploy()
            .argument(&self.governance_address.to_address())
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
            .from(&self.owner_address)
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
            .from(&self.owner_address)
            .gas(DEPLOY_GAS)
            .typed(drwa_asset_manager_proxy::DrwaAssetManagerProxy)
            .init(self.governance_address.to_address())
            .code(ASSET_MANAGER_CODE)
            .returns(ReturnsNewBech32Address)
            .run()
            .await;

        println!("asset-manager deployed at: {asset_addr}");
        self.state.set_asset_manager_address(asset_addr);
        self.generate_blocks(5).await;

        // 4. Deploy attestation (init takes auditor: ManagedAddress)
        let attestation_addr = self
            .interactor
            .tx()
            .from(&self.owner_address)
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

        println!("all DRWA contracts deployed successfully");
    }

    /// Set up a holder as compliant for a given token: register identity,
    /// approve KYC/AML, register the asset, sync holder compliance with
    /// transfer unlocked, and set a permissive token policy.
    pub async fn setup_compliant_holder(
        &mut self,
        token_id: &str,
        holder_address: &Bech32Address,
    ) {
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
                "approved",         // kyc_status
                "clear",            // aml_status
                "qualified",        // investor_class
                "US",               // jurisdiction_code
                0u64,               // expiry_round (permanent)
                false,              // transfer_locked
                false,              // receive_locked
                false,              // auditor_authorized
            )
            .run()
            .await;

        println!("holder compliance synced for {holder_address} on {token_id}");
        self.generate_blocks(3).await;

        // 5. Set token policy allowing transfers (from governance)
        let empty_vec: Vec<String> = Vec::new();
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&policy_registry)
            .gas(CALL_GAS)
            .typed(drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
            .set_token_policy(
                token_id,
                true,               // drwa_enabled
                false,              // global_pause
                false,              // strict_auditor_mode
                false,              // metadata_protection_enabled
                empty_vec.clone(),  // allowed_investor_classes (empty = all)
                empty_vec,          // allowed_jurisdictions (empty = all)
            )
            .run()
            .await;

        println!("token policy set for {token_id} (transfers enabled)");
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
    pub async fn setup_blocked_holder(
        &mut self,
        token_id: &str,
        holder_address: &Bech32Address,
    ) {
        let identity_registry = self.state.current_identity_registry_address().clone();
        let asset_manager = self.state.current_asset_manager_address().clone();

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

        // 3. Register asset (idempotent — required before syncing holder compliance)
        self.ensure_asset_registered(token_id).await;

        // 4. Sync holder compliance — blocked, transfer locked (from governance)
        self.interactor
            .tx()
            .from(&self.governance_address)
            .to(&asset_manager)
            .gas(CALL_GAS)
            .typed(drwa_asset_manager_proxy::DrwaAssetManagerProxy)
            .sync_holder_compliance(
                token_id,
                holder_address.to_address(),
                "approved",         // kyc_status
                "blocked",          // aml_status
                "none",             // investor_class
                "XX",               // jurisdiction_code
                0u64,               // expiry_round (permanent)
                true,               // transfer_locked
                true,               // receive_locked
                false,              // auditor_authorized
            )
            .run()
            .await;

        println!("holder compliance synced (blocked) for {holder_address} on {token_id}");
        self.generate_blocks(3).await;
    }

    /// Generate blocks on the chain simulator.
    pub async fn generate_blocks(&self, num_blocks: u64) {
        self.interactor
            .generate_blocks(num_blocks)
            .await
            .unwrap();
    }

    /// Query the governance address from a DRWA contract.
    pub async fn query_governance(&mut self, contract_address: &Bech32Address) -> Bech32Address {
        let result: Address = self
            .interactor
            .query()
            .to(contract_address)
            .raw_call("getGovernance")
            .returns(ReturnsResultUnmanaged)
            .run()
            .await;

        result.into()
    }

    /// Query whether an identity exists for a subject on the identity registry.
    pub async fn query_identity_exists(&mut self, subject: &Bech32Address) -> bool {
        let identity_registry = self.state.current_identity_registry_address().clone();

        let result: OptionalValue<Vec<u8>> = self
            .interactor
            .query()
            .to(&identity_registry)
            .raw_call("getIdentity")
            .argument(&subject.to_address())
            .returns(ReturnsResultUnmanaged)
            .run()
            .await;

        result.is_some()
    }
}
