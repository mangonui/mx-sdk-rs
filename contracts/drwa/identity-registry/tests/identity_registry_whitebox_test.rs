use drwa_common::{
    DrwaCallerDomain, DrwaGovernanceModule, DrwaSyncOperationType, set_drwa_sync_hook_test_result,
};
use drwa_identity_registry::DrwaIdentityRegistry;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const ISSUER: TestAddress = TestAddress::new("issuer");
const INTRUDER: TestAddress = TestAddress::new("intruder");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-identity-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-identity-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/drwa/identity-registry");
    world.register_contract(CODE_PATH, drwa_identity_registry::ContractBuilder);
    world
}

fn hash32(byte: u8) -> ManagedBuffer<DebugApi> {
    ManagedBuffer::from(&[byte; 32][..])
}

#[test]
fn identity_registry_whitebox_flow() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(INTRUDER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Carbon Ventures"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-001"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                100,
            );
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(record.legal_name, ManagedBuffer::from(b"Carbon Ventures"));
            assert_eq!(record.jurisdiction_code, ManagedBuffer::from(b"SG"));
            assert_eq!(record.registration_number, ManagedBuffer::from(b"REG-001"));
            assert_eq!(record.entity_type, ManagedBuffer::from(b"SPV"));
            assert_eq!(record.kyc_status, ManagedBuffer::from(b"approved"));
            assert_eq!(record.aml_status, ManagedBuffer::from(b"clear"));
            assert_eq!(record.investor_class, ManagedBuffer::from(b"issuer"));
            assert_eq!(record.expiry_round, 100);
        });
}

#[test]
fn identity_registry_registration_sets_future_expiry() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Carbon Ventures"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-001"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(record.expiry_round, 10_000);
        });
}

#[test]
fn identity_registry_registers_privacy_commitment_without_raw_pii() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.register_identity_commitment(
                ISSUER.to_managed_address(),
                hash32(0x11),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"SPV"),
            );

            assert!(envelope.caller_domain == DrwaCallerDomain::IdentityRegistry);
            assert_eq!(envelope.operations.len(), 1);
            let operation = envelope.operations.get(0);
            assert!(operation.operation_type == DrwaSyncOperationType::HolderProfile);
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(record.legal_name, ManagedBuffer::new());
            assert_eq!(record.registration_number, ManagedBuffer::new());
            assert_eq!(record.jurisdiction_code, ManagedBuffer::from(b"SG"));

            let commitment = sc
                .identity_privacy_commitment(&ISSUER.to_managed_address())
                .get();
            assert_eq!(commitment.identity_ref_hash, hash32(0x11));
            assert_eq!(commitment.subject, ISSUER.to_managed_address());
        });
}

#[test]
fn identity_registry_rejects_short_privacy_commitment_hash() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "IDENTITY_COMMITMENT_HASH_MUST_BE_32_BYTES",
        ))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.register_identity_commitment(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"too-short"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"SPV"),
            );
        });
}

#[test]
fn identity_registry_rejects_unauthorized_update() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(INTRUDER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(INTRUDER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Blocked"),
                ManagedBuffer::from(b"US"),
                ManagedBuffer::from(b"REG-X"),
                ManagedBuffer::from(b"SPV"),
            );
        });
}

#[test]
fn identity_registry_requires_pending_governance_acceptance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(INTRUDER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(INTRUDER.to_managed_address());
        });

    world
        .tx()
        .from(INTRUDER)
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
            assert_eq!(
                sc.pending_governance().get(),
                GOVERNANCE.to_managed_address()
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), INTRUDER.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.accept_governance();
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
        });
}

#[test]
fn identity_registry_rejects_expired_pending_governance_acceptance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(INTRUDER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(INTRUDER.to_managed_address());
        });

    world
        .tx()
        .from(INTRUDER)
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
        });

    world.current_block().block_round(1_001);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "pending governance acceptance expired"))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.accept_governance();
        });
}

#[test]
fn identity_registry_register_identity_emits_holder_profile_sync_envelope() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Carbon Ventures"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-001"),
                ManagedBuffer::from(b"SPV"),
            );

            assert!(envelope.caller_domain == DrwaCallerDomain::IdentityRegistry);
            assert_eq!(envelope.operations.len(), 1);
            let op = envelope.operations.get(0);
            assert!(op.operation_type == DrwaSyncOperationType::HolderProfile);
            assert_eq!(op.token_id, ManagedBuffer::new());
            assert_eq!(op.holder, ISSUER.to_managed_address());
            assert_eq!(op.version, 1);
            assert!(!op.body.is_empty());
        },
    );
}

#[test]
fn identity_registry_sync_hook_failure_reverts_identity_registration() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    set_drwa_sync_hook_test_result(7);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "native mirror sync failed"))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Rollback Corp"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-ROLLBACK"),
                ManagedBuffer::from(b"SPV"),
            );
        });
    set_drwa_sync_hook_test_result(0);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            assert!(sc.identity(&ISSUER.to_managed_address()).is_empty());
            assert!(
                sc.holder_profile_version(&ISSUER.to_managed_address())
                    .is_empty()
            );
        });
}

#[test]
fn identity_registry_update_compliance_increments_holder_profile_version() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Carbon Ventures"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-001"),
                ManagedBuffer::from(b"SPV"),
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        },
    );

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                100,
            );
            assert!(envelope.caller_domain == DrwaCallerDomain::IdentityRegistry);
            let op = envelope.operations.get(0);
            assert!(op.operation_type == DrwaSyncOperationType::HolderProfile);
            assert_eq!(op.version, 2);
        },
    );
}

#[test]
fn identity_registry_rejects_unregistered_compliance_update() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "identity not registered - call registerIdentity first",
        ))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                100,
            );
        });
}

#[test]
fn identity_registry_erase_identity_emits_holder_mirror_delete_sync() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Erase Corp"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-ERASE"),
                ManagedBuffer::from(b"SPV"),
            );
            sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                100,
            );
        },
    );

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.erase_identity(ISSUER.to_managed_address());
            assert!(envelope.caller_domain == DrwaCallerDomain::IdentityRegistry);
            assert_eq!(envelope.operations.len(), 1);
            let op = envelope.operations.get(0);
            assert!(op.operation_type == DrwaSyncOperationType::HolderMirrorDelete);
            assert_eq!(op.token_id, ManagedBuffer::new());
            assert_eq!(op.holder, ISSUER.to_managed_address());
            assert_eq!(op.version, 3);
            assert!(op.body.is_empty());
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(record.legal_name, ManagedBuffer::new());
            assert_eq!(record.registration_number, ManagedBuffer::new());
            assert_eq!(record.entity_type, ManagedBuffer::new());
            assert_eq!(record.investor_class, ManagedBuffer::new());
            assert_eq!(record.jurisdiction_code, ManagedBuffer::from(b"ERASED"));
            assert_eq!(record.kyc_status, ManagedBuffer::from(b"deactivated"));
            assert_eq!(record.aml_status, ManagedBuffer::from(b"deactivated"));
            assert_eq!(
                sc.holder_profile_version(&ISSUER.to_managed_address())
                    .get(),
                3
            );
        });
}

#[test]
fn identity_registry_erase_identity_clears_privacy_commitment() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity_commitment(
                ISSUER.to_managed_address(),
                hash32(0x44),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"SPV"),
            );
            assert!(
                !sc.identity_privacy_commitment(&ISSUER.to_managed_address())
                    .is_empty()
            );
        },
    );

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.erase_identity(ISSUER.to_managed_address());
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            assert!(
                sc.identity_privacy_commitment(&ISSUER.to_managed_address())
                    .is_empty()
            );
        });
}

#[test]
fn identity_registry_rejects_expired_compliance_update() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Carbon Ventures"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-001"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    world.current_block().block_round(10);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "expiry_round must be in the future or 0 for permanent",
        ))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                9,
            );
        });
}

#[test]
fn identity_registry_rejects_overlong_compliance_expiry() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Carbon Ventures"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-001"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    world.current_block().block_round(10);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "expiry_round exceeds maximum identity validity window",
        ))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                100_011,
            );
        });
}

#[test]
fn identity_registry_upgrade_preserves_storage() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Register an identity
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Upgrade Corp"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-UPG"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    // Call upgrade
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.upgrade();
        },
    );

    // Verify storage is preserved
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(record.legal_name, ManagedBuffer::from(b"Upgrade Corp"));
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
        });
}

#[test]
fn identity_registry_allows_empty_string_fields() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Register identity with empty registration_number and entity_type
    // (jurisdiction_code is required, so we provide it)
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Minimal Corp"),
                ManagedBuffer::from(b"US"),
                ManagedBuffer::new(), // empty registration_number
                ManagedBuffer::new(), // empty entity_type
            );
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(record.legal_name, ManagedBuffer::from(b"Minimal Corp"));
            assert!(record.registration_number.is_empty());
            assert!(record.entity_type.is_empty());
        });
}

#[test]
fn identity_registry_deactivate_identity() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Register identity
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Deactivate Corp"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-DEACT"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    // Approve compliance first
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                100,
            );
        },
    );

    // Deactivate
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.deactivate_identity(ISSUER.to_managed_address());
            assert!(envelope.caller_domain == DrwaCallerDomain::IdentityRegistry);
            assert_eq!(envelope.operations.len(), 1);
            let op = envelope.operations.get(0);
            assert!(op.operation_type == DrwaSyncOperationType::HolderProfile);
            // version 1 from register, 2 from update_compliance, 3 from deactivate
            assert_eq!(op.version, 3);
        },
    );

    // Verify deactivated
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(record.kyc_status, ManagedBuffer::from(b"deactivated"));
            assert_eq!(record.aml_status, ManagedBuffer::from(b"deactivated"));
            assert_eq!(
                record.jurisdiction_code,
                ManagedBuffer::from(b"DEACTIVATED")
            );
            // Other fields preserved
            assert_eq!(record.legal_name, ManagedBuffer::from(b"Deactivate Corp"));
        });
}

#[test]
fn identity_registry_identical_update_is_noop() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Idempotent Corp"),
                ManagedBuffer::from(b"US"),
                ManagedBuffer::from(b"REG-IDEMP"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                100,
            );
            assert_eq!(envelope.operations.len(), 1);
            assert_eq!(envelope.operations.get(0).version, 2);
        },
    );

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                100,
            );
            assert_eq!(envelope.operations.len(), 0);
            assert_eq!(
                sc.holder_profile_version(&ISSUER.to_managed_address())
                    .get(),
                2
            );
        },
    );
}

#[test]
fn identity_registry_deactivate_nonexistent_fails() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "identity not registered"))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.deactivate_identity(ISSUER.to_managed_address());
        });
}

#[test]
fn identity_registry_deactivate_rejects_zero_address() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "subject address must not be zero"))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.deactivate_identity(ManagedAddress::zero());
        });
}

#[test]
fn identity_registry_set_validity_config_by_governance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Governance sets new validity config.
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.set_validity_config(5_000, 50_000);
        },
    );

    // Verify the config was persisted
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            assert_eq!(sc.default_validity_rounds().get(), 5_000);
            assert_eq!(sc.max_validity_rounds().get(), 50_000);
        });
}

#[test]
fn identity_registry_set_validity_config_affects_registration_expiry() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Update validity config to a custom default
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.set_validity_config(20_000, 200_000);
        },
    );

    // Register identity — expiry should use the updated default (20_000)
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Config Corp"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-CFG"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            // block_round defaults to 0 in test, so expiry = 0 + 20_000
            assert_eq!(record.expiry_round, 20_000);
        });
}

#[test]
fn identity_registry_registration_rejects_expiry_overflow() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.current_block().block_round(u64::MAX - 4);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "identity expiry round overflow"))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Overflow Corp"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-OVERFLOW"),
                ManagedBuffer::from(b"SPV"),
            );
        });
}

#[test]
fn identity_registry_update_rejects_max_validity_overflow() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Max Validity Corp"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-MAX"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    world.current_block().block_round(u64::MAX - 4);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "identity max validity round overflow"))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                u64::MAX,
            );
        });
}

#[test]
fn identity_registry_rejects_duplicate_registration() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Register identity for ISSUER
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Carbon Ventures"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-001"),
                ManagedBuffer::from(b"SPV"),
            );
        },
    );

    // Attempt to register identity for ISSUER again — must reject
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "IDENTITY_ALREADY_REGISTERED: use updateComplianceStatus to modify existing identity",
        ))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Duplicate Corp"),
                ManagedBuffer::from(b"US"),
                ManagedBuffer::from(b"REG-DUP"),
                ManagedBuffer::from(b"LLC"),
            );
        });
}

#[test]
fn identity_registry_permanent_identity_expiry_zero() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Step 1: Register identity (gets default expiry = block_round + default_validity_rounds)
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.register_identity(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"Permanent Corp"),
                ManagedBuffer::from(b"SG"),
                ManagedBuffer::from(b"REG-PERM"),
                ManagedBuffer::from(b"SPV"),
            );
            // First registration → holder profile version 1
            assert_eq!(envelope.operations.get(0).version, 1);
        },
    );

    // Verify default expiry is non-zero
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert!(record.expiry_round > 0, "default expiry should be non-zero");
        });

    // Step 2: Call updateComplianceStatus with expiry_round = 0 (permanent)
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        drwa_identity_registry::contract_obj,
        |sc| {
            let envelope = sc.update_compliance_status(
                ISSUER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"issuer"),
                0, // permanent identity
            );
            // Step 4: Holder profile sync version incremented to 2
            assert_eq!(envelope.operations.get(0).version, 2);
        },
    );

    // Step 3: Verify the identity record has expiry_round = 0
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(
                record.expiry_round, 0,
                "expiry_round must be 0 for permanent identity"
            );
            assert_eq!(record.kyc_status, ManagedBuffer::from(b"approved"));
            assert_eq!(record.investor_class, ManagedBuffer::from(b"issuer"));
        });
}

#[test]
fn identity_registry_set_validity_config_rejects_intruder() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(INTRUDER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(INTRUDER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            sc.set_validity_config(5_000, 50_000);
        });
}
