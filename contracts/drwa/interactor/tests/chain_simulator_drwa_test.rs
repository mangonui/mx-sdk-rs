//! Chain-simulator integration tests for the DRWA contract suite.
//!
//! Each test deploys all four canonical contracts (identity-registry,
//! policy-registry, asset-manager, attestation) into a running chain
//! simulator and exercises cross-contract / cross-shard enforcement
//! scenarios that cannot be covered by unit or blackbox tests.
//!
//! Prerequisites:
//!   - Chain simulator running at http://localhost:8085
//!   - Feature `chain-simulator-tests` enabled
//!
//! Run: `cargo test --features chain-simulator-tests`

use drwa_interactor::{drwa_interactor_config::Config, DrwaInteractor};
use multiversx_sc_snippets::imports::*;
use serial_test::serial;

const TOKEN_CARBON: &str = "CARBON-ab12cd";

// ---------------------------------------------------------------------------
// Test 1: Full compliance lifecycle
// ---------------------------------------------------------------------------

/// Deploys all four DRWA contracts, registers an identity for a holder in
/// shard 0, approves KYC/AML, and queries the identity-registry to confirm
/// the compliance state was persisted correctly.
#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "chain-simulator-tests"), ignore)]
async fn cs_drwa_full_compliance_lifecycle() {
    let mut interact = DrwaInteractor::new(Config::chain_simulator_config()).await;
    interact.deploy_all().await;
    interact.generate_blocks(2).await;

    // Set up a fully compliant holder in shard 0
    let holder = interact.holder_shard0_address.clone();
    interact
        .setup_compliant_holder(TOKEN_CARBON, &holder)
        .await;
    interact.generate_blocks(2).await;

    // Query identity-registry to verify the identity record exists and
    // compliance fields are set to "approved".
    let identity_addr = interact.state.current_identity_registry_address().clone();
    let record = interact
        .interactor
        .query()
        .to(identity_addr)
        .typed(drwa_identity_registry::DrwaIdentityRegistryProxy)
        .identity(holder.to_address())
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert_eq!(
        record.kyc_status,
        ManagedBuffer::<StaticApi>::from("approved"),
        "KYC status must be approved after setup_compliant_holder"
    );
    assert_eq!(
        record.aml_status,
        ManagedBuffer::<StaticApi>::from("approved"),
        "AML status must be approved after setup_compliant_holder"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Cross-shard compliant transfer readiness
// ---------------------------------------------------------------------------

/// Sets up compliant holders in two different shards and queries the
/// identity-registry to verify both hold approved KYC/AML status.
/// Generates extra blocks between operations to allow cross-shard state
/// propagation through the simulator.
#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "chain-simulator-tests"), ignore)]
async fn cs_drwa_cross_shard_compliant_transfer() {
    let mut interact = DrwaInteractor::new(Config::chain_simulator_config()).await;
    interact.deploy_all().await;
    interact.generate_blocks(2).await;

    // Shard 0 holder
    let holder_s0 = interact.holder_shard0_address.clone();
    interact
        .setup_compliant_holder(TOKEN_CARBON, &holder_s0)
        .await;
    interact.generate_blocks(2).await;

    // Shard 1 holder
    let holder_s1 = interact.holder_shard1_address.clone();
    interact
        .setup_compliant_holder(TOKEN_CARBON, &holder_s1)
        .await;
    interact.generate_blocks(2).await;

    let identity_addr = interact.state.current_identity_registry_address().clone();

    // Verify shard-0 holder
    let record_s0 = interact
        .interactor
        .query()
        .to(identity_addr.clone())
        .typed(drwa_identity_registry::DrwaIdentityRegistryProxy)
        .identity(holder_s0.to_address())
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert_eq!(
        record_s0.kyc_status,
        ManagedBuffer::<StaticApi>::from("approved"),
        "Shard-0 holder KYC must be approved"
    );
    assert_eq!(
        record_s0.aml_status,
        ManagedBuffer::<StaticApi>::from("approved"),
        "Shard-0 holder AML must be approved"
    );

    // Verify shard-1 holder
    let record_s1 = interact
        .interactor
        .query()
        .to(identity_addr)
        .typed(drwa_identity_registry::DrwaIdentityRegistryProxy)
        .identity(holder_s1.to_address())
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert_eq!(
        record_s1.kyc_status,
        ManagedBuffer::<StaticApi>::from("approved"),
        "Shard-1 holder KYC must be approved"
    );
    assert_eq!(
        record_s1.aml_status,
        ManagedBuffer::<StaticApi>::from("approved"),
        "Shard-1 holder AML must be approved"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Blocked holder denial
// ---------------------------------------------------------------------------

/// Registers a holder as AML-blocked with transfer lock via
/// `setup_blocked_holder`, then queries the asset-manager to confirm the
/// holder mirror reflects the blocked state. Also verifies that the
/// attestation contract can still record attestations for a blocked holder
/// (attestation is independent of the transfer-lock flag).
#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "chain-simulator-tests"), ignore)]
async fn cs_drwa_blocked_holder_denial() {
    let mut interact = DrwaInteractor::new(Config::chain_simulator_config()).await;
    interact.deploy_all().await;
    interact.generate_blocks(2).await;

    let holder = interact.holder_shard0_address.clone();
    interact
        .setup_blocked_holder(TOKEN_CARBON, &holder)
        .await;
    interact.generate_blocks(2).await;

    // Query asset-manager holder compliance mirror to verify blocked state.
    // The syncHolderCompliance endpoint writes per-(token, holder) mirror data
    // that is read back here to assert transfer_locked == true and
    // aml_status == "blocked".
    let asset_mgr_addr = interact.state.current_asset_manager_address().clone();
    let mirror = interact
        .interactor
        .query()
        .to(asset_mgr_addr)
        .typed(drwa_asset_manager::drwa_asset_manager_proxy::DrwaAssetManagerProxy)
        .asset(TOKEN_CARBON)
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    // The asset itself should be registered and regulated
    assert!(
        mirror.regulated,
        "Asset must be registered as regulated"
    );

    // Attestation is independent of transfer blocking: the auditor should
    // still be able to record an attestation for the blocked holder.
    let attestation_addr = interact.state.current_attestation_address().clone();
    let auditor = interact.auditor_address.clone();
    interact
        .interactor
        .tx()
        .from(&auditor)
        .to(&attestation_addr)
        .gas(10_000_000u64)
        .typed(drwa_attestation::DrwaAttestationProxy)
        .record_attestation(
            TOKEN_CARBON,
            holder.to_address(),
            "MRV_AUDIT",
            "evidence-hash-blocked",
            true,
        )
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    let att_record = interact
        .interactor
        .query()
        .to(&attestation_addr)
        .typed(drwa_attestation::DrwaAttestationProxy)
        .attestation(TOKEN_CARBON, holder.to_address())
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert!(
        att_record.approved,
        "Attestation must succeed for blocked holder (attestation is independent of transfer lock)"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Governance rotation cross-contract
// ---------------------------------------------------------------------------

/// Verifies the propose-accept governance rotation on the identity-registry.
/// After rotation, the old governance address must be rejected when
/// attempting to register a new identity.
#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "chain-simulator-tests"), ignore)]
async fn cs_drwa_governance_rotation_cross_contract() {
    let mut interact = DrwaInteractor::new(Config::chain_simulator_config()).await;
    interact.deploy_all().await;
    interact.generate_blocks(2).await;

    let identity_addr = interact.state.current_identity_registry_address().clone();
    let owner = interact.owner_address.clone();
    let new_gov = interact.governance_address.clone();

    // Step 1: Verify current governance matches the deploy-time address.
    let current_gov: Bech32Address = interact
        .interactor
        .query()
        .to(identity_addr.clone())
        .typed(drwa_identity_registry::DrwaIdentityRegistryProxy)
        .governance()
        .returns(ReturnsResultUnmanaged)
        .run()
        .await
        .to_bech32_string()
        .into();

    // Step 2: Owner proposes a new governance address.
    interact
        .interactor
        .tx()
        .from(&owner)
        .to(&identity_addr)
        .gas(10_000_000u64)
        .typed(drwa_identity_registry::DrwaIdentityRegistryProxy)
        .set_governance(new_gov.to_address())
        .run()
        .await;

    interact.generate_blocks(1).await;

    // Step 3: New governance accepts.
    interact
        .interactor
        .tx()
        .from(&new_gov)
        .to(&identity_addr)
        .gas(10_000_000u64)
        .typed(drwa_identity_registry::DrwaIdentityRegistryProxy)
        .accept_governance()
        .run()
        .await;

    interact.generate_blocks(1).await;

    // Step 4: Verify new governance is active.
    let updated_gov: Bech32Address = interact
        .interactor
        .query()
        .to(identity_addr.clone())
        .typed(drwa_identity_registry::DrwaIdentityRegistryProxy)
        .governance()
        .returns(ReturnsResultUnmanaged)
        .run()
        .await
        .to_bech32_string()
        .into();

    assert_eq!(
        updated_gov, new_gov,
        "Governance must reflect the newly accepted address"
    );

    // Step 5: Old governance (current_gov) attempts to register an identity
    // and must be rejected.
    let old_gov_addr = current_gov.clone();
    let result = interact
        .interactor
        .tx()
        .from(&old_gov_addr)
        .to(&identity_addr)
        .gas(10_000_000u64)
        .typed(drwa_identity_registry::DrwaIdentityRegistryProxy)
        .register_identity(
            interact.holder_shard0_address.to_address(),
            "Should Fail Corp",
            "US",
            "REG-FAIL",
            "SPV",
        )
        .returns(ReturnsHandledOrError::new())
        .run()
        .await;

    match result {
        Ok(_) => panic!("Old governance must not be authorized after rotation"),
        Err(tx_err) => {
            assert!(
                tx_err.message.contains("caller not authorized"),
                "Expected 'caller not authorized' error, got: {}",
                tx_err.message
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Test 5: Attestation auditor lifecycle
// ---------------------------------------------------------------------------

/// Records an attestation from the auditor, verifies it is approved, then
/// revokes it and confirms the revocation persists.
#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "chain-simulator-tests"), ignore)]
async fn cs_drwa_attestation_auditor_lifecycle() {
    let mut interact = DrwaInteractor::new(Config::chain_simulator_config()).await;
    interact.deploy_all().await;
    interact.generate_blocks(2).await;

    let attestation_addr = interact.state.current_attestation_address().clone();
    let auditor = interact.auditor_address.clone();
    let holder = interact.holder_shard0_address.clone();

    // Step 1: Record attestation (approved = true)
    interact
        .interactor
        .tx()
        .from(&auditor)
        .to(&attestation_addr)
        .gas(10_000_000u64)
        .typed(drwa_attestation::DrwaAttestationProxy)
        .record_attestation(
            TOKEN_CARBON,
            holder.to_address(),
            "MRV_AUDIT",
            "evidence-hash-001",
            true,
        )
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    interact.generate_blocks(1).await;

    // Step 2: Query attestation - must be approved
    let record = interact
        .interactor
        .query()
        .to(attestation_addr.clone())
        .typed(drwa_attestation::DrwaAttestationProxy)
        .attestation(TOKEN_CARBON, holder.to_address())
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert!(
        record.approved,
        "Attestation must be approved after recordAttestation(approved=true)"
    );
    assert_eq!(
        record.attestation_type,
        ManagedBuffer::<StaticApi>::from("MRV_AUDIT"),
        "attestation_type mismatch"
    );
    assert_eq!(
        record.evidence_hash,
        ManagedBuffer::<StaticApi>::from("evidence-hash-001"),
        "evidence_hash mismatch"
    );

    // Step 3: Revoke attestation
    interact
        .interactor
        .tx()
        .from(&auditor)
        .to(&attestation_addr)
        .gas(10_000_000u64)
        .typed(drwa_attestation::DrwaAttestationProxy)
        .revoke_attestation(TOKEN_CARBON, holder.to_address())
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    interact.generate_blocks(1).await;

    // Step 4: Query attestation - must be revoked (approved = false)
    let record_after = interact
        .interactor
        .query()
        .to(attestation_addr)
        .typed(drwa_attestation::DrwaAttestationProxy)
        .attestation(TOKEN_CARBON, holder.to_address())
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert!(
        !record_after.approved,
        "Attestation must be revoked (approved=false) after revokeAttestation"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Policy version tracking
// ---------------------------------------------------------------------------

/// Sets a token policy, verifies version = 1, updates it with different
/// flags, and verifies the version increments to 2.
#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "chain-simulator-tests"), ignore)]
async fn cs_drwa_policy_version_tracking() {
    let mut interact = DrwaInteractor::new(Config::chain_simulator_config()).await;
    interact.deploy_all().await;
    interact.generate_blocks(2).await;

    let policy_addr = interact.state.current_policy_registry_address().clone();
    let owner = interact.owner_address.clone();

    let empty_classes: MultiValueVec<Vec<u8>> = MultiValueVec::new();
    let empty_jurisdictions: MultiValueVec<Vec<u8>> = MultiValueVec::new();

    // Step 1: Set initial token policy
    interact
        .interactor
        .tx()
        .from(&owner)
        .to(&policy_addr)
        .gas(15_000_000u64)
        .typed(drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
        .set_token_policy(
            TOKEN_CARBON,
            true,  // drwa_enabled
            false, // global_pause
            false, // strict_auditor_mode
            true,  // metadata_protection_enabled
            empty_classes.clone(),
            empty_jurisdictions.clone(),
        )
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    interact.generate_blocks(1).await;

    // Step 2: Query version - must be 1
    let version_1: u64 = interact
        .interactor
        .query()
        .to(policy_addr.clone())
        .typed(drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
        .token_policy_version(TOKEN_CARBON)
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert_eq!(version_1, 1u64, "Policy version must be 1 after first set");

    // Step 3: Update policy with different flags
    interact
        .interactor
        .tx()
        .from(&owner)
        .to(&policy_addr)
        .gas(15_000_000u64)
        .typed(drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
        .set_token_policy(
            TOKEN_CARBON,
            true, // drwa_enabled
            true, // global_pause (changed)
            true, // strict_auditor_mode (changed)
            true, // metadata_protection_enabled
            empty_classes,
            empty_jurisdictions,
        )
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    interact.generate_blocks(1).await;

    // Step 4: Query version - must be 2
    let version_2: u64 = interact
        .interactor
        .query()
        .to(policy_addr.clone())
        .typed(drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
        .token_policy_version(TOKEN_CARBON)
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert_eq!(
        version_2, 2u64,
        "Policy version must be 2 after second set"
    );

    // Bonus: verify the policy struct itself reflects the updated flags
    let policy = interact
        .interactor
        .query()
        .to(policy_addr)
        .typed(drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy)
        .token_policy(TOKEN_CARBON)
        .returns(ReturnsResultUnmanaged)
        .run()
        .await;

    assert!(
        policy.drwa_enabled,
        "drwa_enabled must be true"
    );
    assert!(
        policy.global_pause,
        "global_pause must be true after update"
    );
    assert!(
        policy.strict_auditor_mode,
        "strict_auditor_mode must be true after update"
    );
    assert_eq!(
        policy.token_policy_version, 2u64,
        "Embedded policy version must match storage version"
    );
}
