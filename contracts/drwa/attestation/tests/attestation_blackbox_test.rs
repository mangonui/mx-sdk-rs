use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const AUDITOR: TestAddress = TestAddress::new("auditor");
const NEW_AUDITOR: TestAddress = TestAddress::new("new_auditor");
const SUBJECT: TestAddress = TestAddress::new("subject");
const OTHER: TestAddress = TestAddress::new("other");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-attestation");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-attestation.mxsc.json");
const TOKEN_ID: &str = "CARBON-ab12cd";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/attestation");
    blockchain.register_contract(CODE_PATH, drwa_attestation::ContractBuilder);
    blockchain
}

/// Deploy the contract and set up standard accounts.
/// Returns the world instance ready for endpoint calls.
fn deploy() -> ScenarioWorld {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(NEW_AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(SUBJECT).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .argument(&AUDITOR)
        .run();

    world
}

#[test]
fn attestation_blackbox_deploy_and_record() {
    let mut world = deploy();

    // Record an attestation from the auditor
    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            SUBJECT.to_managed_address(),
            "MRV",
            "hash-001",
            true,
        )
        .run();

    // Query the attestation and verify fields
    let record = world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .attestation(TOKEN_ID, SUBJECT.to_managed_address())
        .returns(ReturnsResult)
        .run();

    assert_eq!(
        record.token_id,
        ManagedBuffer::<StaticApi>::from(TOKEN_ID),
        "token_id mismatch"
    );
    assert_eq!(
        record.subject,
        SUBJECT.to_managed_address(),
        "subject mismatch"
    );
    assert_eq!(
        record.attestation_type,
        ManagedBuffer::<StaticApi>::from("MRV"),
        "attestation_type mismatch"
    );
    assert_eq!(
        record.evidence_hash,
        ManagedBuffer::<StaticApi>::from("hash-001"),
        "evidence_hash mismatch"
    );
    assert!(record.approved, "attestation should be approved");
}

#[test]
fn attestation_blackbox_non_auditor_rejected() {
    let mut world = deploy();

    // Attempt to record from a non-auditor address
    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            SUBJECT.to_managed_address(),
            "MRV",
            "hash-001",
            true,
        )
        .with_result(ExpectError(4u64, "caller not auditor"))
        .run();
}

#[test]
fn attestation_blackbox_revoke() {
    let mut world = deploy();

    // Record an approved attestation
    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            SUBJECT.to_managed_address(),
            "MRV",
            "hash-revoke",
            true,
        )
        .run();

    // Verify it is approved
    let record = world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .attestation(TOKEN_ID, SUBJECT.to_managed_address())
        .returns(ReturnsResult)
        .run();
    assert!(record.approved, "attestation should be approved before revocation");

    // Revoke the attestation
    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .revoke_attestation(TOKEN_ID, SUBJECT.to_managed_address())
        .run();

    // Verify approved is now false
    let record = world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .attestation(TOKEN_ID, SUBJECT.to_managed_address())
        .returns(ReturnsResult)
        .run();
    assert!(!record.approved, "attestation should be revoked");
}

#[test]
fn attestation_blackbox_auditor_rotation() {
    let mut world = deploy();

    // Owner proposes a new auditor
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .set_auditor(NEW_AUDITOR.to_managed_address())
        .run();

    // New auditor accepts the role
    world
        .tx()
        .from(NEW_AUDITOR)
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .accept_auditor()
        .run();

    // Old auditor can no longer record attestations
    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            SUBJECT.to_managed_address(),
            "MRV",
            "hash-old",
            true,
        )
        .with_result(ExpectError(4u64, "caller not auditor"))
        .run();

    // New auditor can record attestations
    world
        .tx()
        .from(NEW_AUDITOR)
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            SUBJECT.to_managed_address(),
            "MRV",
            "hash-new",
            true,
        )
        .run();

    // Verify the attestation was recorded by the new auditor
    let record = world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy)
        .attestation(TOKEN_ID, SUBJECT.to_managed_address())
        .returns(ReturnsResult)
        .run();
    assert_eq!(
        record.evidence_hash,
        ManagedBuffer::<StaticApi>::from("hash-new"),
        "evidence_hash should reflect the new auditor's attestation"
    );
    assert!(record.approved, "attestation from new auditor should be approved");
}
