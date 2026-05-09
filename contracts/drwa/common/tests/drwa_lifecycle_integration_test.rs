// DRWA 4-contract lifecycle integration test.
//
// Deploys all four canonical DRWA contracts (policy-registry, identity-registry,
// asset-manager, attestation) in a single ScenarioWorld and exercises the full
// regulated asset lifecycle:
//
//   1. Register a token policy (policy-registry)
//   2. Register a holder identity (identity-registry)
//   3. Register an asset (asset-manager)
//   4. Sync holder compliance mirror (asset-manager)
//   5. Record an auditor attestation (attestation)
//   6. Verify cross-contract state consistency
//
// This validates that shared types from drwa-common interoperate correctly
// across all four contracts in a single simulated blockchain.

use multiversx_sc_scenario::imports::*;

use drwa_asset_manager::drwa_asset_manager_proxy::DrwaAssetManagerProxy;
use drwa_attestation::DrwaAttestation;
use drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy;
use drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy;
use drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy;

// ── Addresses ──────────────────────────────────────────────────────────

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const AUDITOR: TestAddress = TestAddress::new("auditor");
const HOLDER: TestAddress = TestAddress::new("holder");

const POLICY_SC: TestSCAddress = TestSCAddress::new("drwa-policy-registry");
const IDENTITY_SC: TestSCAddress = TestSCAddress::new("drwa-identity-registry");
const ASSET_SC: TestSCAddress = TestSCAddress::new("drwa-asset-manager");
const ATTESTATION_SC: TestSCAddress = TestSCAddress::new("drwa-attestation");

const POLICY_CODE: MxscPath =
    MxscPath::new("mxsc:../../policy-registry/output/drwa-policy-registry.mxsc.json");
const IDENTITY_CODE: MxscPath =
    MxscPath::new("mxsc:../../identity-registry/output/drwa-identity-registry.mxsc.json");
const ASSET_CODE: MxscPath =
    MxscPath::new("mxsc:../../asset-manager/output/drwa-asset-manager.mxsc.json");
const ATTESTATION_CODE: MxscPath =
    MxscPath::new("mxsc:../../attestation/output/drwa-attestation.mxsc.json");

const TOKEN_ID: &[u8] = b"CARBON-ab12cd";
const POLICY_ID: &[u8] = TOKEN_ID;

// ── World setup ────────────────────────────────────────────────────────

fn world() -> ScenarioWorld {
    let mut w = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    w.set_current_dir_from_workspace("contracts/drwa/common");
    w.register_contract(POLICY_CODE, drwa_policy_registry::ContractBuilder);
    w.register_contract(IDENTITY_CODE, drwa_identity_registry::ContractBuilder);
    w.register_contract(ASSET_CODE, drwa_asset_manager::ContractBuilder);
    w.register_contract(ATTESTATION_CODE, drwa_attestation::ContractBuilder);
    w
}

/// Deploys all four contracts and sets up standard accounts.
fn deploy_all() -> ScenarioWorld {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(10_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(10_000_000u64);
    world.account(AUDITOR).nonce(1).balance(10_000_000u64);
    world.account(HOLDER).nonce(1).balance(10_000_000u64);

    // Deploy policy-registry (governance = GOVERNANCE)
    world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(POLICY_CODE)
        .new_address(POLICY_SC)
        .run();

    // Deploy identity-registry (governance = GOVERNANCE)
    world
        .tx()
        .from(OWNER)
        .typed(DrwaIdentityRegistryProxy)
        .init(GOVERNANCE)
        .code(IDENTITY_CODE)
        .new_address(IDENTITY_SC)
        .run();

    // Deploy asset-manager (governance = GOVERNANCE)
    world
        .tx()
        .from(OWNER)
        .typed(DrwaAssetManagerProxy)
        .init(GOVERNANCE)
        .code(ASSET_CODE)
        .new_address(ASSET_SC)
        .run();

    world
        .tx()
        .from(GOVERNANCE)
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .set_policy_registry_address(POLICY_SC)
        .run();

    // Deploy attestation (auditor = AUDITOR)
    world
        .tx()
        .from(OWNER)
        .typed(DrwaAttestationProxy)
        .init(AUDITOR)
        .code(ATTESTATION_CODE)
        .new_address(ATTESTATION_SC)
        .run();

    world
}

// ── Full lifecycle test ────────────────────────────────────────────────

/// Exercises the complete regulated asset lifecycle across all four contracts
/// and verifies cross-contract state consistency.
#[test]
fn drwa_full_lifecycle_four_contracts() {
    let mut world = deploy_all();

    // ── Step 1: Register token policy ──────────────────────────────
    let mut investor_classes: ManagedVec<StaticApi, ManagedBuffer<StaticApi>> = ManagedVec::new();
    investor_classes.push(ManagedBuffer::from(b"ACCREDITED"));

    let mut jurisdictions: ManagedVec<StaticApi, ManagedBuffer<StaticApi>> = ManagedVec::new();
    jurisdictions.push(ManagedBuffer::from(b"SG"));
    jurisdictions.push(ManagedBuffer::from(b"US"));

    world
        .tx()
        .from(GOVERNANCE)
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,  // drwa_enabled
            false, // global_pause
            true,  // strict_auditor_mode
            true,  // metadata_protection_enabled
            investor_classes,
            jurisdictions,
        )
        .run();

    // Verify policy was persisted with version 1
    let policy_version: u64 = world
        .query()
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy_version(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        policy_version, 1u64,
        "policy version should be 1 after first set"
    );

    let policy: drwa_common::DrwaTokenPolicy<StaticApi> = world
        .query()
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert!(policy.drwa_enabled, "policy drwa_enabled should be true");
    assert!(
        policy.strict_auditor_mode,
        "policy strict_auditor_mode should be true"
    );
    assert_eq!(policy.allowed_investor_classes.len(), 1);
    assert_eq!(policy.allowed_jurisdictions.len(), 2);

    // ── Step 2: Register holder identity ───────────────────────────
    world
        .tx()
        .from(GOVERNANCE)
        .to(IDENTITY_SC)
        .typed(DrwaIdentityRegistryProxy)
        .register_identity(
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"Acme Corp"),
            ManagedBuffer::from(b"SG"),
            ManagedBuffer::from(b"REG-123456"),
            ManagedBuffer::from(b"CORPORATE"),
        )
        .run();

    // Verify identity was registered with pending KYC/AML
    let identity: drwa_identity_registry::IdentityRecord<StaticApi> = world
        .query()
        .to(IDENTITY_SC)
        .typed(DrwaIdentityRegistryProxy)
        .identity(HOLDER.to_managed_address())
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        identity.subject,
        HOLDER.to_managed_address(),
        "identity subject mismatch"
    );
    assert_eq!(
        identity.jurisdiction_code,
        ManagedBuffer::<StaticApi>::from(b"SG"),
        "identity jurisdiction mismatch"
    );
    assert_eq!(
        identity.kyc_status,
        ManagedBuffer::<StaticApi>::from(b"pending"),
        "identity kyc_status should be pending on registration"
    );
    assert_eq!(
        identity.aml_status,
        ManagedBuffer::<StaticApi>::from(b"pending"),
        "identity aml_status should be pending on registration"
    );

    // Approve the holder's KYC/AML
    world
        .tx()
        .from(GOVERNANCE)
        .to(IDENTITY_SC)
        .typed(DrwaIdentityRegistryProxy)
        .update_compliance_status(
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"approved"),
            ManagedBuffer::from(b"clear"),
            ManagedBuffer::from(b"ACCREDITED"),
            0u64, // permanent
        )
        .run();

    // Verify updated compliance status
    let identity_updated: drwa_identity_registry::IdentityRecord<StaticApi> = world
        .query()
        .to(IDENTITY_SC)
        .typed(DrwaIdentityRegistryProxy)
        .identity(HOLDER.to_managed_address())
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        identity_updated.kyc_status,
        ManagedBuffer::<StaticApi>::from(b"approved"),
        "kyc_status should be approved after update"
    );
    assert_eq!(
        identity_updated.aml_status,
        ManagedBuffer::<StaticApi>::from(b"clear"),
        "aml_status should be clear after update"
    );
    assert_eq!(
        identity_updated.investor_class,
        ManagedBuffer::<StaticApi>::from(b"ACCREDITED"),
        "investor_class should be ACCREDITED after update"
    );

    // ── Step 3: Register the asset ─────────────────────────────────
    world
        .tx()
        .from(GOVERNANCE)
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"CARBON_CREDIT"),
            ManagedBuffer::from(POLICY_ID),
        )
        .run();

    // Verify asset record
    let asset: drwa_asset_manager::AssetRecord<StaticApi> = world
        .query()
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .asset(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        asset.token_id,
        ManagedBuffer::<StaticApi>::from(TOKEN_ID),
        "asset token_id mismatch"
    );
    assert_eq!(
        asset.policy_id,
        ManagedBuffer::<StaticApi>::from(POLICY_ID),
        "asset policy_id mismatch"
    );
    assert!(asset.regulated, "asset should be regulated");
    assert!(
        !asset.wind_down_initiated,
        "wind_down should not be initiated"
    );

    // ── Step 4: Sync holder compliance mirror ──────────────────────
    world
        .tx()
        .from(GOVERNANCE)
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .sync_holder_compliance(
            ManagedBuffer::from(TOKEN_ID),
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"approved"),   // kyc_status
            ManagedBuffer::from(b"clear"),      // aml_status
            ManagedBuffer::from(b"ACCREDITED"), // investor_class
            ManagedBuffer::from(b"SG"),         // jurisdiction_code
            0u64,                               // expiry_round (permanent)
            false,                              // transfer_locked
            false,                              // receive_locked
            false,                              // auditor_authorized (not yet attested)
        )
        .run();

    // Verify holder mirror
    let mirror: drwa_common::DrwaHolderMirror<StaticApi> = world
        .query()
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .get_holder_mirror(
            ManagedBuffer::<StaticApi>::from(TOKEN_ID),
            HOLDER.to_managed_address(),
        )
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        mirror.holder_policy_version, 1u64,
        "holder mirror version should be 1"
    );
    assert_eq!(
        mirror.kyc_status,
        ManagedBuffer::<StaticApi>::from(b"approved"),
        "holder mirror kyc_status mismatch"
    );
    assert_eq!(
        mirror.aml_status,
        ManagedBuffer::<StaticApi>::from(b"clear"),
        "holder mirror aml_status mismatch"
    );
    assert_eq!(
        mirror.investor_class,
        ManagedBuffer::<StaticApi>::from(b"ACCREDITED"),
        "holder mirror investor_class mismatch"
    );
    assert_eq!(
        mirror.jurisdiction_code,
        ManagedBuffer::<StaticApi>::from(b"SG"),
        "holder mirror jurisdiction mismatch"
    );
    assert!(
        !mirror.transfer_locked,
        "holder should not be transfer locked"
    );
    assert!(
        !mirror.receive_locked,
        "holder should not be receive locked"
    );
    assert!(
        !mirror.auditor_authorized,
        "holder should not be auditor authorized yet"
    );

    // ── Step 5: Record auditor attestation ─────────────────────────
    world
        .tx()
        .from(AUDITOR)
        .to(ATTESTATION_SC)
        .typed(DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            HOLDER.to_managed_address(),
            "MRV",
            "evidence-hash-001",
            true,
        )
        .run();

    // Verify attestation record
    let attestation: drwa_attestation::AttestationRecord<StaticApi> = world
        .query()
        .to(ATTESTATION_SC)
        .typed(DrwaAttestationProxy)
        .attestation(
            ManagedBuffer::<StaticApi>::from(TOKEN_ID),
            HOLDER.to_managed_address(),
        )
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        attestation.token_id,
        ManagedBuffer::<StaticApi>::from(TOKEN_ID),
        "attestation token_id mismatch"
    );
    assert_eq!(
        attestation.subject,
        HOLDER.to_managed_address(),
        "attestation subject mismatch"
    );
    assert_eq!(
        attestation.attestation_type,
        ManagedBuffer::<StaticApi>::from(b"MRV"),
        "attestation type mismatch"
    );
    assert!(attestation.approved, "attestation should be approved");

    // ── Step 6: Re-sync holder compliance without touching attestation-owned state ─
    // The attestation contract is the only authority allowed to control
    // auditor authorization. Asset-manager may refresh compliance fields,
    // but it must not be able to promote the holder into that state.
    world
        .tx()
        .from(GOVERNANCE)
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .sync_holder_compliance(
            ManagedBuffer::from(TOKEN_ID),
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"approved"),
            ManagedBuffer::from(b"clear"),
            ManagedBuffer::from(b"ACCREDITED"),
            ManagedBuffer::from(b"SG"),
            0u64,
            false,
            false,
            false, // attestation-owned; asset-manager must not set this
        )
        .run();

    // Verify the updated holder mirror keeps the asset-manager-owned
    // compliance fields while leaving auditor authorization to attestation.
    // Because the second asset-manager sync is byte-identical to the first
    // asset-manager-owned state, it should be a no-op and must not bump the
    // holder-mirror version just because attestation state exists elsewhere.
    let mirror_final: drwa_common::DrwaHolderMirror<StaticApi> = world
        .query()
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .get_holder_mirror(
            ManagedBuffer::<StaticApi>::from(TOKEN_ID),
            HOLDER.to_managed_address(),
        )
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        mirror_final.holder_policy_version, 1u64,
        "holder mirror version should remain 1 when the second sync is a no-op"
    );
    assert!(
        !mirror_final.auditor_authorized,
        "asset-manager mirror must not claim auditor authorization"
    );

    // ── Cross-contract consistency verification ────────────────────
    // The token policy, identity, asset record, holder mirror, and
    // attestation all reference the same token_id and holder. Verify
    // consistency:

    // Policy: drwa_enabled=true, strict_auditor_mode=true
    // Identity: kyc=approved, aml=clear, investor_class=ACCREDITED, jurisdiction=SG
    // Asset: regulated=true, policy_id equals token_id.
    // Mirror: kyc=approved, aml=clear, investor_class=ACCREDITED, jurisdiction=SG
    // Attestation: approved=true, type=MRV

    // Verify jurisdiction alignment: identity jurisdiction matches mirror jurisdiction
    assert_eq!(
        identity_updated.jurisdiction_code, mirror_final.jurisdiction_code,
        "jurisdiction mismatch between identity-registry and asset-manager mirror"
    );

    // Verify investor class alignment: identity investor_class matches mirror investor_class
    assert_eq!(
        identity_updated.investor_class, mirror_final.investor_class,
        "investor_class mismatch between identity-registry and asset-manager mirror"
    );

    // Verify KYC alignment
    assert_eq!(
        identity_updated.kyc_status, mirror_final.kyc_status,
        "kyc_status mismatch between identity-registry and asset-manager mirror"
    );

    // Verify AML alignment
    assert_eq!(
        identity_updated.aml_status, mirror_final.aml_status,
        "aml_status mismatch between identity-registry and asset-manager mirror"
    );

    // Verify the attestation subject matches the holder in the mirror
    assert_eq!(
        attestation.subject,
        HOLDER.to_managed_address(),
        "attestation subject should match holder address"
    );

    // Verify attestation remains the independent source of truth for auditor authorization.
    assert!(attestation.approved, "attestation should remain approved");
}

/// Validates that the lifecycle correctly handles deactivation flows:
/// deactivate identity, deactivate token policy, and attestation revocation.
#[test]
fn drwa_lifecycle_deactivation_flow() {
    let mut world = deploy_all();

    let empty_classes = ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new();
    let empty_jurisdictions = ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new();

    // Set up: register token policy
    world
        .tx()
        .from(GOVERNANCE)
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            false,
            false,
            false,
            empty_classes,
            empty_jurisdictions,
        )
        .run();

    // Set up: register identity
    world
        .tx()
        .from(GOVERNANCE)
        .to(IDENTITY_SC)
        .typed(DrwaIdentityRegistryProxy)
        .register_identity(
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"Test User"),
            ManagedBuffer::from(b"US"),
            ManagedBuffer::from(b"REG-999"),
            ManagedBuffer::from(b"INDIVIDUAL"),
        )
        .run();

    // Set up: register asset
    world
        .tx()
        .from(GOVERNANCE)
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"BOND"),
            ManagedBuffer::from(POLICY_ID),
        )
        .run();

    // Set up: record attestation
    world
        .tx()
        .from(AUDITOR)
        .to(ATTESTATION_SC)
        .typed(DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            HOLDER.to_managed_address(),
            "AUDIT",
            "evidence-hash-002",
            true,
        )
        .run();

    // ── Revoke attestation ─────────────────────────────────────────
    world
        .tx()
        .from(AUDITOR)
        .to(ATTESTATION_SC)
        .typed(DrwaAttestationProxy)
        .revoke_attestation(
            ManagedBuffer::<StaticApi>::from(TOKEN_ID),
            HOLDER.to_managed_address(),
        )
        .run();

    let attestation: drwa_attestation::AttestationRecord<StaticApi> = world
        .query()
        .to(ATTESTATION_SC)
        .typed(DrwaAttestationProxy)
        .attestation(
            ManagedBuffer::<StaticApi>::from(TOKEN_ID),
            HOLDER.to_managed_address(),
        )
        .returns(ReturnsResult)
        .run();
    assert!(!attestation.approved, "attestation should be revoked");

    // ── Deactivate identity ────────────────────────────────────────
    world
        .tx()
        .from(GOVERNANCE)
        .to(IDENTITY_SC)
        .typed(DrwaIdentityRegistryProxy)
        .deactivate_identity(HOLDER.to_managed_address())
        .run();

    let identity: drwa_identity_registry::IdentityRecord<StaticApi> = world
        .query()
        .to(IDENTITY_SC)
        .typed(DrwaIdentityRegistryProxy)
        .identity(HOLDER.to_managed_address())
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        identity.kyc_status,
        ManagedBuffer::<StaticApi>::from(b"deactivated"),
        "identity kyc_status should be deactivated"
    );
    assert_eq!(
        identity.aml_status,
        ManagedBuffer::<StaticApi>::from(b"deactivated"),
        "identity aml_status should be deactivated"
    );

    // ── Deactivate token policy ────────────────────────────────────
    world
        .tx()
        .from(GOVERNANCE)
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .deactivate_token_policy(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .run();

    let policy: drwa_common::DrwaTokenPolicy<StaticApi> = world
        .query()
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert!(
        !policy.drwa_enabled,
        "policy drwa_enabled should be false after deactivation"
    );
    assert_eq!(
        policy.token_policy_version, 2u64,
        "policy version should be 2 after deactivation"
    );
}

/// Validates that a revoked auditor cannot record new attestations.
/// This tests the auth boundary across the attestation contract's
/// auditor lifecycle: grant -> revoke -> attempt -> reject.
#[test]
fn drwa_lifecycle_revoked_auditor_rejected() {
    let mut world = deploy_all();
    let new_auditor = TestAddress::new("new_auditor");
    world.account(new_auditor).nonce(1).balance(1_000_000u64);

    // Record an initial attestation to prove auditor works
    world
        .tx()
        .from(AUDITOR)
        .to(ATTESTATION_SC)
        .typed(DrwaAttestationProxy)
        .record_attestation(
            b"CARBON-ab12cd",
            HOLDER.to_managed_address(),
            "MRV",
            "evidence-hash-100",
            true,
        )
        .run();

    // Owner revokes the auditor (only_owner, not in proxy — use whitebox)
    world
        .tx()
        .from(OWNER)
        .to(ATTESTATION_SC)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.revoke_auditor();
        });

    // Revoked auditor cannot record new attestations. After revocation the
    // auditor storage is cleared, so `self.auditor().get()` fails with a
    // decode error because the SingleValueMapper is empty. This is a hard
    // revert — no attestation can succeed until a new auditor is set.
    world
        .tx()
        .from(AUDITOR)
        .to(ATTESTATION_SC)
        .typed(DrwaAttestationProxy)
        .record_attestation(
            b"CARBON-ab12cd",
            HOLDER.to_managed_address(),
            "AUDIT",
            "evidence-hash-101",
            true,
        )
        .with_result(ExpectError(
            4,
            "storage decode error (key: auditor): bad array length",
        ))
        .run();
}

/// Validates that an expired governance proposal cannot be accepted.
/// The governance transfer has a 1000-round acceptance window; advancing
/// past it must cause rejection.
#[test]
fn drwa_lifecycle_expired_governance_proposal_rejected() {
    let mut world = deploy_all();
    let new_gov = TestAddress::new("new_governance");
    world.account(new_gov).nonce(1).balance(1_000_000u64);

    // Active governance proposes a new governance address on policy-registry.
    world
        .tx()
        .from(GOVERNANCE)
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .set_governance(new_gov.to_managed_address())
        .run();

    // Advance past the 1000-round acceptance window
    world.current_block().block_round(1_001);

    // Expired proposal acceptance is rejected
    world
        .tx()
        .from(new_gov)
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .accept_governance()
        .with_result(ExpectError(4, "pending governance acceptance expired"))
        .run();
}

/// Validates that cross-contract authorization boundaries are enforced:
/// unauthorized callers are rejected by each contract independently.
#[test]
fn drwa_lifecycle_cross_contract_auth_boundaries() {
    let mut world = deploy_all();
    let other = TestAddress::new("unauthorized");
    world.account(other).nonce(1).balance(1_000_000u64);

    // Unauthorized caller rejected by policy-registry
    world
        .tx()
        .from(other)
        .to(POLICY_SC)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            false,
            false,
            false,
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
        )
        .with_result(ExpectError(4, "caller not authorized"))
        .run();

    // Unauthorized caller rejected by identity-registry
    world
        .tx()
        .from(other)
        .to(IDENTITY_SC)
        .typed(DrwaIdentityRegistryProxy)
        .register_identity(
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"Test"),
            ManagedBuffer::from(b"US"),
            ManagedBuffer::from(b"REG-001"),
            ManagedBuffer::from(b"CORP"),
        )
        .with_result(ExpectError(4, "caller not authorized"))
        .run();

    // Unauthorized caller rejected by asset-manager
    world
        .tx()
        .from(other)
        .to(ASSET_SC)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"BOND"),
            ManagedBuffer::from(POLICY_ID),
        )
        .with_result(ExpectError(4, "caller not authorized"))
        .run();

    // Unauthorized caller rejected by attestation (non-auditor)
    world
        .tx()
        .from(other)
        .to(ATTESTATION_SC)
        .typed(DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            HOLDER.to_managed_address(),
            "MRV",
            "evidence-hash-003",
            true,
        )
        .with_result(ExpectError(4, "caller not auditor"))
        .run();
}
