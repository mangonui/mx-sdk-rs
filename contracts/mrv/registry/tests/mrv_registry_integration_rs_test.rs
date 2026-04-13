use mrv_registry::MrvRegistry;
use mrv_common::MrvGovernanceModule;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-registry.mxsc.json");
const METHODOLOGY_ID: &[u8] = b"INT-AG-SOC-001";
const METHODOLOGY_VERSION: &[u8] = b"1.0.0";
const METHODOLOGY_DIGEST: &[u8] = b"sha256:ag-methodology-pack-001";
const PROJECT_ID: &[u8] = b"project-public-010";
const EVIDENCE_ID: &[u8] = b"evidence-public-010";
const VERIFICATION_CASE_ID: &[u8] = b"verification-public-010";
const LOT_ID: &[u8] = b"lot-public-010";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/registry");
    blockchain.register_contract(CODE_PATH, mrv_registry::ContractBuilder);
    blockchain
}

#[test]
fn mrv_registry_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.anchor_report_v2(
                ManagedBuffer::from(b"report-public-002"),
                ManagedBuffer::from(b"tenant-public-002"),
                ManagedBuffer::from(b"farm-public-002"),
                ManagedBuffer::from(b"season-public-002"),
                ManagedBuffer::from(b"project-public-002"),
                ManagedBuffer::from(b"sha256:report-public-002"),
                ManagedBuffer::from(b"sha256"),
                ManagedBuffer::from(b"json-c14n-v1"),
                2,
                1_710_730_000,
                ManagedBuffer::from(b"sha3-256:evidence-manifest-002"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            assert!(sc.is_report_anchored(ManagedBuffer::from(b"report-public-002")));
            assert_eq!(sc.get_anchored_reports_count(), 1usize);
        });
}

#[test]
fn mrv_registry_governance_acceptance_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
            assert_eq!(
                sc.pending_governance().get(),
                GOVERNANCE.to_managed_address()
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.accept_governance();
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
            assert!(sc.pending_governance().is_empty());
        });
}

#[test]
fn mrv_registry_amend_report_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.anchor_report_v2(
                ManagedBuffer::from(b"report-public-004"),
                ManagedBuffer::from(b"tenant-public-004"),
                ManagedBuffer::from(b"farm-public-004"),
                ManagedBuffer::from(b"season-public-004"),
                ManagedBuffer::from(b"project-public-004"),
                ManagedBuffer::from(b"sha256:report-public-004"),
                ManagedBuffer::from(b"sha256"),
                ManagedBuffer::from(b"json-c14n-v1"),
                1,
                1_710_750_000,
                ManagedBuffer::from(b"sha3-256:evidence-manifest-004"),
            );
            sc.amend_report_v2(
                ManagedBuffer::from(b"report-public-004"),
                ManagedBuffer::from(b"tenant-public-004"),
                ManagedBuffer::from(b"farm-public-004"),
                ManagedBuffer::from(b"season-public-004-amended"),
                ManagedBuffer::from(b"project-public-004-amended"),
                ManagedBuffer::from(b"sha256:report-public-004-amended"),
                ManagedBuffer::from(b"sha256"),
                ManagedBuffer::from(b"json-c14n-v1"),
                2,
                1_710_750_100,
                ManagedBuffer::from(b"sha3-256:evidence-manifest-004-amended"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let proof = sc
                .get_report_proof(ManagedBuffer::from(b"report-public-004"))
                .into_option()
                .unwrap();
            assert_eq!(
                proof.public_season_id.to_boxed_bytes().as_slice(),
                b"season-public-004-amended"
            );
            assert_eq!(proof.methodology_version, 2);
        });
}

#[test]
fn mrv_registry_methodology_registration_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_methodology(
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(METHODOLOGY_DIGEST),
                ManagedBuffer::from(b"ready_for_review"),
                1_735_689_600,
                0,
            );
            sc.set_methodology_approval_status(
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"approved_internal"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let record = sc
                .get_methodology_record(
                    ManagedBuffer::from(METHODOLOGY_ID),
                    ManagedBuffer::from(METHODOLOGY_VERSION),
                )
                .into_option()
                .unwrap();
            assert_eq!(
                record.approval_status.to_boxed_bytes().as_slice(),
                b"approved_internal"
            );
            assert_eq!(sc.get_methodology_records_count(), 1usize);
        });
}

#[test]
fn mrv_registry_project_evidence_and_verification_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_project(
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(b"tenant-public-010"),
                ManagedBuffer::from(b"asset-public-010"),
                ManagedBuffer::from(b"period-public-010"),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(b"active"),
            );
            sc.register_evidence(
                ManagedBuffer::from(EVIDENCE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(b"report-public-010"),
                ManagedBuffer::from(b"sha256:evidence-public-010"),
                ManagedBuffer::from(b"sha256:manifest-public-010"),
                1_735_689_600,
            );
            sc.create_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(b"report-public-010"),
                GOVERNANCE.to_managed_address(),
                1_735_689_600,
            );
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"assigned"),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_735_689_601,
            );
        });

    world.query().to(SC_ADDRESS).whitebox(mrv_registry::contract_obj, |sc| {
        assert_eq!(sc.get_project_records_count(), 1usize);
        assert_eq!(sc.get_evidence_records_count(), 1usize);
        assert_eq!(sc.get_verification_cases_count(), 1usize);
    });
}

#[test]
fn mrv_registry_issuance_lifecycle_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.create_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(b"report-public-010"),
                GOVERNANCE.to_managed_address(),
                1_735_689_600,
            );
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"assigned"),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_735_689_601,
            );
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"approved"),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::from(b"sha256:statement-public-010"),
                ManagedBuffer::from(b"drwa-attestation:token-public-010:verifier"),
                1_735_689_602,
            );
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                ManagedBuffer::from(b"15.0000"),
                ManagedBuffer::new(),
            );
            sc.retire_issuance_lot(ManagedBuffer::from(LOT_ID));
            sc.reverse_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(b"5.0000"),
                ManagedBuffer::from(b"lot-public-011"),
            );
        });

    world.query().to(SC_ADDRESS).whitebox(mrv_registry::contract_obj, |sc| {
        let lot = sc
            .get_issuance_lot(ManagedBuffer::from(LOT_ID))
            .into_option()
            .unwrap();
        assert_eq!(lot.status.to_boxed_bytes().as_slice(), b"reversed");
        assert_eq!(sc.get_issuance_lots_count(), 1usize);
    });
}
