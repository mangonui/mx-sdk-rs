use drwa_attestation::DrwaAttestation;
use drwa_common::{DrwaCallerDomain, DrwaSyncOperationType};
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const AUDITOR: TestAddress = TestAddress::new("auditor");
const SUBJECT: TestAddress = TestAddress::new("subject");
const OTHER: TestAddress = TestAddress::new("other");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-attestation");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-attestation.mxsc.json");
const TOKEN_ID: &[u8] = b"CARBON-ab12cd";

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/drwa/attestation");
    world.register_contract(CODE_PATH, drwa_attestation::ContractBuilder);
    world
}

#[test]
fn attestation_whitebox_flow() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-001"),
                true,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            let record = sc
                .attestation(
                    &ManagedBuffer::from(TOKEN_ID),
                    &SUBJECT.to_managed_address(),
                )
                .get();
            assert_eq!(record.token_id, ManagedBuffer::from(TOKEN_ID));
            assert_eq!(record.attestation_type, ManagedBuffer::from(b"MRV"));
            assert_eq!(record.evidence_hash, ManagedBuffer::from(b"hash-001"));
            assert!(record.approved);
        });
}

#[test]
fn attestation_rejects_non_auditor() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not auditor"))
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-001"),
                true,
            );
        });
}

#[test]
fn attestation_owner_can_rotate_auditor() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.set_auditor(OTHER.to_managed_address());
            assert_eq!(sc.pending_auditor().get(), OTHER.to_managed_address());
        });

    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.accept_auditor();
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not auditor"))
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-001"),
                true,
            );
        });

    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-rotated"),
                true,
            );
        });
}

#[test]
fn attestation_requires_pending_auditor_acceptance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.set_auditor(OTHER.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            assert_eq!(sc.auditor().get(), AUDITOR.to_managed_address());
            assert_eq!(sc.pending_auditor().get(), OTHER.to_managed_address());
        });
}

#[test]
fn attestation_rejects_expired_pending_auditor_acceptance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.set_auditor(OTHER.to_managed_address());
        });

    world.current_block().block_round(1_001);

    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "pending auditor acceptance expired"))
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.accept_auditor();
        });
}

#[test]
fn attestation_record_emits_auditor_authorization_sync_envelope() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            let envelope = sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-001"),
                true,
            );

            assert!(envelope.caller_domain == DrwaCallerDomain::Attestation);
            assert_eq!(envelope.operations.len(), 1);
            let op = envelope.operations.get(0);
            assert!(op.operation_type == DrwaSyncOperationType::HolderAuditorAuthorization);
            assert_eq!(op.token_id, ManagedBuffer::from(TOKEN_ID));
            assert_eq!(op.holder, SUBJECT.to_managed_address());
            assert_eq!(op.version, 1);
            assert!(!op.body.is_empty());
        });
}

#[test]
fn attestation_record_revocation_increments_auditor_authorization_version() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            let envelope = sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-001"),
                true,
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            let envelope = sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-002"),
                false,
            );
            let op = envelope.operations.get(0);
            assert!(op.operation_type == DrwaSyncOperationType::HolderAuditorAuthorization);
            assert_eq!(op.version, 2);
        });
}

#[test]
fn attestation_rejects_invalid_token_id_format() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id suffix must be 6 characters"))
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.record_attestation(
                ManagedBuffer::from(b"CARBON-001"),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-invalid"),
                true,
            );
        });
}

#[test]
fn attestation_revoke_attestation_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    // Record an approved attestation first
    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-revoke-001"),
                true,
            );
        });

    // Verify it is approved
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            let record = sc
                .attestation(
                    &ManagedBuffer::from(TOKEN_ID),
                    &SUBJECT.to_managed_address(),
                )
                .get();
            assert!(record.approved);
        });

    // Revoke the attestation
    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.revoke_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
            );
        });

    // Verify approved is now false
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            let record = sc
                .attestation(
                    &ManagedBuffer::from(TOKEN_ID),
                    &SUBJECT.to_managed_address(),
                )
                .get();
            assert!(!record.approved);
        });
}

#[test]
fn attestation_revoke_nonexistent_fails() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "attestation does not exist"))
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.revoke_attestation(
                ManagedBuffer::from(TOKEN_ID),
                SUBJECT.to_managed_address(),
            );
        });
}

#[test]
fn attestation_record_rejects_zero_address_subject() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "ZERO_ADDRESS: subject must not be zero"))
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.record_attestation(
                ManagedBuffer::from(TOKEN_ID),
                ManagedAddress::zero(),
                ManagedBuffer::from(b"MRV"),
                ManagedBuffer::from(b"hash-zero"),
                true,
            );
        });
}

#[test]
fn attestation_revoke_rejects_zero_address_subject() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "subject address must not be zero"))
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.revoke_attestation(
                ManagedBuffer::from(TOKEN_ID),
                ManagedAddress::zero(),
            );
        });
}

#[test]
fn attestation_get_auditor_view() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            sc.init(AUDITOR.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            assert_eq!(sc.auditor().get(), AUDITOR.to_managed_address());
        });
}
