use mrv_registry::MrvRegistry;
use mrv_common::MrvGovernanceModule;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const OTHER: TestAddress = TestAddress::new("other");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/registry");
    world.register_contract(CODE_PATH, mrv_registry::ContractBuilder);
    world
}

const REPORT_ID: &[u8] = b"report-public-001";
const TENANT_ID: &[u8] = b"tenant-public-001";
const FARM_ID: &[u8] = b"farm-public-001";
const SEASON_ID: &[u8] = b"season-public-001";
const PROJECT_ID: &[u8] = b"project-public-001";
const REPORT_HASH: &[u8] = b"sha256:report-public-001";
const HASH_ALGO: &[u8] = b"sha256";
const CANONICALIZATION: &[u8] = b"json-c14n-v1";
const EVIDENCE_MANIFEST_HASH: &[u8] = b"sha3-256:evidence-manifest-001";
const METHODOLOGY_ID: &[u8] = b"INT-EN-SOLAR-001";
const METHODOLOGY_VERSION: &[u8] = b"1.0.0";
const METHODOLOGY_DIGEST: &[u8] = b"sha256:methodology-pack-001";
const METHODOLOGY_STATUS: &[u8] = b"ready_for_review";
const EVIDENCE_ID: &[u8] = b"evidence-001";
const EVIDENCE_HASH: &[u8] = b"sha256:evidence-001";
const VERIFICATION_CASE_ID: &[u8] = b"verification-001";
const LOT_ID: &[u8] = b"lot-001";

#[test]
fn mrv_registry_whitebox_flow() {
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
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(REPORT_HASH),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                1,
                1_710_720_000,
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let proof = sc
                .get_report_proof(ManagedBuffer::from(REPORT_ID))
                .into_option()
                .unwrap();
            assert_eq!(proof.report_id.to_boxed_bytes().as_slice(), REPORT_ID);
            assert_eq!(
                proof.public_tenant_id.to_boxed_bytes().as_slice(),
                TENANT_ID
            );
            assert_eq!(proof.public_farm_id.to_boxed_bytes().as_slice(), FARM_ID);
            assert_eq!(
                proof.public_season_id.to_boxed_bytes().as_slice(),
                SEASON_ID
            );
            assert_eq!(
                proof.public_project_id.to_boxed_bytes().as_slice(),
                PROJECT_ID
            );
            assert_eq!(proof.report_hash.to_boxed_bytes().as_slice(), REPORT_HASH);
            assert_eq!(proof.hash_algo.to_boxed_bytes().as_slice(), HASH_ALGO);
            assert_eq!(
                proof.canonicalization.to_boxed_bytes().as_slice(),
                CANONICALIZATION
            );
            assert_eq!(proof.methodology_version, 1);
            assert_eq!(proof.anchored_at, 1_710_720_000);
            assert_eq!(
                proof.evidence_manifest_hash.to_boxed_bytes().as_slice(),
                EVIDENCE_MANIFEST_HASH
            );

            let season_proof = sc
                .get_report_proof_by_season(
                    ManagedBuffer::from(TENANT_ID),
                    ManagedBuffer::from(FARM_ID),
                    ManagedBuffer::from(SEASON_ID),
                )
                .into_option()
                .unwrap();
            assert_eq!(
                season_proof.report_hash.to_boxed_bytes().as_slice(),
                proof.report_hash.to_boxed_bytes().as_slice()
            );
            assert_eq!(
                season_proof
                    .evidence_manifest_hash
                    .to_boxed_bytes()
                    .as_slice(),
                proof.evidence_manifest_hash.to_boxed_bytes().as_slice()
            );

            let season_report_id = sc
                .get_report_id_by_season(
                    ManagedBuffer::from(TENANT_ID),
                    ManagedBuffer::from(FARM_ID),
                    ManagedBuffer::from(SEASON_ID),
                )
                .into_option()
                .unwrap();
            assert_eq!(season_report_id.to_boxed_bytes().as_slice(), REPORT_ID);

            assert!(sc.is_report_anchored(ManagedBuffer::from(REPORT_ID)));
            assert_eq!(sc.get_anchored_reports_count(), 1usize);
        });
}

#[test]
fn mrv_registry_idempotent_anchor_keeps_single_entry() {
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

    for _ in 0..2 {
        world
            .tx()
            .from(GOVERNANCE)
            .to(SC_ADDRESS)
            .whitebox(mrv_registry::contract_obj, |sc| {
                sc.anchor_report_v2(
                    ManagedBuffer::from(REPORT_ID),
                    ManagedBuffer::from(TENANT_ID),
                    ManagedBuffer::from(FARM_ID),
                    ManagedBuffer::from(SEASON_ID),
                    ManagedBuffer::from(PROJECT_ID),
                    ManagedBuffer::from(REPORT_HASH),
                    ManagedBuffer::from(HASH_ALGO),
                    ManagedBuffer::from(CANONICALIZATION),
                    1,
                    1_710_720_000,
                    ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
                );
            });
    }

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            assert_eq!(sc.get_anchored_reports_count(), 1usize);
            let proof = sc
                .get_report_proof(ManagedBuffer::from(REPORT_ID))
                .into_option()
                .unwrap();
            assert_eq!(proof.report_hash.to_boxed_bytes().as_slice(), REPORT_HASH);
            assert_eq!(
                proof.evidence_manifest_hash.to_boxed_bytes().as_slice(),
                EVIDENCE_MANIFEST_HASH
            );
        });
}

#[test]
fn mrv_registry_rejects_conflicting_anchor_payload() {
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
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(REPORT_HASH),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                1,
                1_710_720_000,
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "conflicting report proof"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.anchor_report_v2(
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(b"sha256:conflicting-report"),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                1,
                1_710_720_000,
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
            );
        });
}

#[test]
fn mrv_registry_allows_governance_to_anchor_after_acceptance() {
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
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.accept_governance();
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.anchor_report_v2(
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(REPORT_HASH),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                1,
                1_710_720_000,
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
            assert!(sc.pending_governance().is_empty());
            assert!(sc.is_report_anchored(ManagedBuffer::from(REPORT_ID)));
        });
}

#[test]
fn mrv_registry_tracks_methodology_records_and_status_changes() {
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
                ManagedBuffer::from(METHODOLOGY_STATUS),
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
            assert_eq!(record.methodology_id.to_boxed_bytes().as_slice(), METHODOLOGY_ID);
            assert_eq!(record.version_label.to_boxed_bytes().as_slice(), METHODOLOGY_VERSION);
            assert_eq!(record.pack_digest.to_boxed_bytes().as_slice(), METHODOLOGY_DIGEST);
            assert_eq!(
                record.approval_status.to_boxed_bytes().as_slice(),
                b"approved_internal"
            );
            assert_eq!(sc.get_methodology_records_count(), 1usize);
        });
}

#[test]
fn mrv_registry_supersedes_methodology_record() {
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
                ManagedBuffer::from(b"approved_internal"),
                1_735_689_600,
                0,
            );
            sc.supersede_methodology(
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"1.1.0"),
                1_767_225_600,
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
                b"superseded"
            );
            assert_eq!(record.superseded_by.to_boxed_bytes().as_slice(), b"1.1.0");
            assert_eq!(record.effective_to, 1_767_225_600);
        });
}

#[test]
fn mrv_registry_tracks_project_and_evidence_records() {
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
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(b"asset-001"),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(b"active"),
            );
            sc.register_evidence(
                ManagedBuffer::from(EVIDENCE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(EVIDENCE_HASH),
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
                1_710_720_000,
            );
        });

    world.query().to(SC_ADDRESS).whitebox(mrv_registry::contract_obj, |sc| {
        let project = sc
            .get_project_record(ManagedBuffer::from(PROJECT_ID))
            .into_option()
            .unwrap();
        assert_eq!(project.tenant_id.to_boxed_bytes().as_slice(), TENANT_ID);
        assert_eq!(project.reporting_period_id.to_boxed_bytes().as_slice(), SEASON_ID);
        assert_eq!(sc.get_project_records_count(), 1usize);

        let evidence = sc
            .get_evidence_record(ManagedBuffer::from(EVIDENCE_ID))
            .into_option()
            .unwrap();
        assert_eq!(evidence.evidence_hash.to_boxed_bytes().as_slice(), EVIDENCE_HASH);
        assert_eq!(
            evidence.manifest_hash.to_boxed_bytes().as_slice(),
            EVIDENCE_MANIFEST_HASH
        );
        assert_eq!(sc.get_evidence_records_count(), 1usize);
    });
}

#[test]
fn mrv_registry_tracks_verification_case_transitions() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);
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
                ManagedBuffer::from(REPORT_ID),
                OTHER.to_managed_address(),
                1_710_720_000,
            );
            // pending_assignment → assigned
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"assigned"),
                OTHER.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_710_720_010,
            );
            // assigned → in_review
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"in_review"),
                OTHER.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_710_720_015,
            );
            // in_review → approved
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"approved"),
                OTHER.to_managed_address(),
                ManagedBuffer::from(b"sha256:statement-001"),
                ManagedBuffer::from(b"drwa-attestation:token-001:verifier"),
                1_710_720_020,
            );
        });

    world.query().to(SC_ADDRESS).whitebox(mrv_registry::contract_obj, |sc| {
        let case_record = sc
            .get_verification_case(ManagedBuffer::from(VERIFICATION_CASE_ID))
            .into_option()
            .unwrap();
        assert_eq!(case_record.status.to_boxed_bytes().as_slice(), b"approved");
        assert_eq!(
            case_record.verifier_statement_hash.to_boxed_bytes().as_slice(),
            b"sha256:statement-001"
        );
        assert_eq!(sc.get_verification_cases_count(), 1usize);
    });
}

#[test]
fn mrv_registry_tracks_issuance_lifecycle() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);
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
                ManagedBuffer::from(REPORT_ID),
                OTHER.to_managed_address(),
                1_710_720_000,
            );
            // pending_assignment → assigned
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"assigned"),
                OTHER.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_710_720_010,
            );
            // assigned → in_review
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"in_review"),
                OTHER.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_710_720_015,
            );
            // in_review → approved
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"approved"),
                OTHER.to_managed_address(),
                ManagedBuffer::from(b"sha256:statement-001"),
                ManagedBuffer::from(b"drwa-attestation:token-001:verifier"),
                1_710_720_020,
            );
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                ManagedBuffer::from(b"10.5000"),
                ManagedBuffer::new(),
            );
            sc.retire_issuance_lot(ManagedBuffer::from(LOT_ID));
            sc.reverse_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(b"2.0000"),
                ManagedBuffer::from(b"lot-002"),
            );
        });

    world.query().to(SC_ADDRESS).whitebox(mrv_registry::contract_obj, |sc| {
        let lot = sc
            .get_issuance_lot(ManagedBuffer::from(LOT_ID))
            .into_option()
            .unwrap();
        assert_eq!(lot.status.to_boxed_bytes().as_slice(), b"reversed");
        assert_eq!(lot.vintage, 2026);
        assert_eq!(lot.quantity.to_boxed_bytes().as_slice(), b"10.5000");
        assert_eq!(lot.reversed_amount.to_boxed_bytes().as_slice(), b"2.0000");
        // replacement_for_lot_id is set at creation, not during reversal
        // The reversal's replacement_lot_id is only emitted in the event payload
        assert!(lot.replacement_for_lot_id.is_empty());
        assert_eq!(sc.get_issuance_lots_count(), 1usize);
    });
}

#[test]
fn mrv_registry_rejects_non_owner_non_governance_anchor() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);
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
        .from(OTHER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.anchor_report_v2(
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(REPORT_HASH),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                1,
                1_710_720_000,
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
            );
        });
}

// Test removed: anchor_report v1 endpoint was fully removed from the contract.

#[test]
fn mrv_registry_allows_governance_amendment_of_report_proof() {
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
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(REPORT_HASH),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                1,
                1_710_720_000,
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.amend_report_v2(
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(b"season-public-001-amended"),
                ManagedBuffer::from(b"project-public-001-amended"),
                ManagedBuffer::from(b"sha256:report-public-001-amended"),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                2,
                1_710_720_100,
                ManagedBuffer::from(b"sha3-256:evidence-manifest-001-amended"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let proof = sc
                .get_report_proof(ManagedBuffer::from(REPORT_ID))
                .into_option()
                .unwrap();
            assert_eq!(
                proof.public_season_id.to_boxed_bytes().as_slice(),
                b"season-public-001-amended"
            );
            assert_eq!(
                proof.public_project_id.to_boxed_bytes().as_slice(),
                b"project-public-001-amended"
            );
            assert_eq!(proof.methodology_version, 2);
            assert_eq!(proof.anchored_at, 1_710_720_100);
            assert_eq!(
                proof.evidence_manifest_hash.to_boxed_bytes().as_slice(),
                b"sha3-256:evidence-manifest-001-amended"
            );
            assert!(
                sc.get_report_id_by_season(
                    ManagedBuffer::from(TENANT_ID),
                    ManagedBuffer::from(FARM_ID),
                    ManagedBuffer::from(b"season-public-001-amended"),
                )
                .into_option()
                .is_some()
            );
            assert!(
                sc.get_report_id_by_season(
                    ManagedBuffer::from(TENANT_ID),
                    ManagedBuffer::from(FARM_ID),
                    ManagedBuffer::from(SEASON_ID),
                )
                .into_option()
                .is_none()
            );
        });
}

#[test]
fn mrv_registry_rejects_amendment_into_existing_season_binding() {
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
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(REPORT_HASH),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                1,
                1_710_720_000,
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
            );
            sc.anchor_report_v2(
                ManagedBuffer::from(b"report-public-002"),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(b"season-public-002"),
                ManagedBuffer::from(b"project-public-002"),
                ManagedBuffer::from(b"sha256:report-public-002"),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                1,
                1_710_720_010,
                ManagedBuffer::from(b"sha3-256:evidence-manifest-002"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "season already bound to a different report",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.amend_report_v2(
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(b"season-public-002"),
                ManagedBuffer::from(b"project-public-001-amended"),
                ManagedBuffer::from(b"sha256:report-public-001-amended"),
                ManagedBuffer::from(HASH_ALGO),
                ManagedBuffer::from(CANONICALIZATION),
                2,
                1_710_720_100,
                ManagedBuffer::from(b"sha3-256:evidence-manifest-001-amended"),
            );
        });
}

const VVB_ADDRESS: TestAddress = TestAddress::new("vvb-address");

fn deploy_registry(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);
    world.account(VVB_ADDRESS).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });
}

#[test]
fn mrv_registry_commit_execution_bundle_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-bundle-001"),
                1u64,
                ManagedBuffer::from(b"bafybundle001"),
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let bundle = sc
                .get_execution_bundle(ManagedBuffer::from(b"pai-bundle-001"), 1u64)
                .into_option()
                .unwrap();
            assert_eq!(bundle.pai_id.to_boxed_bytes().as_slice(), b"pai-bundle-001");
            assert_eq!(bundle.monitoring_period_n, 1u64);
            assert_eq!(bundle.bundle_cid.to_boxed_bytes().as_slice(), b"bafybundle001");
            assert_eq!(bundle.bundle_hash.len(), 32);
        });
}

#[test]
fn mrv_registry_submit_verification_statement_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xBBu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-stmt-001"),
                1u64,
                ManagedBuffer::from(b"bafybundle-stmt-001"),
                ManagedBuffer::from(&hash_32[..]),
            );
            sc.submit_verification_statement(
                ManagedBuffer::from(b"pai-stmt-001"),
                1u64,
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"bafystmt001"),
                ManagedBuffer::from(b"approved"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let stmt = sc
                .get_verification_statement(ManagedBuffer::from(b"pai-stmt-001"), 1u64)
                .into_option()
                .unwrap();
            assert_eq!(stmt.vvb_did, VVB_ADDRESS.to_managed_address());
            assert_eq!(stmt.outcome.to_boxed_bytes().as_slice(), b"approved");
        });
}

#[test]
fn mrv_registry_register_and_deregister_vvb_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            assert!(sc.is_vvb_accredited(VVB_ADDRESS.to_managed_address()));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.deregister_accredited_vvb(VVB_ADDRESS.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            assert!(!sc.is_vvb_accredited(VVB_ADDRESS.to_managed_address()));
        });
}

#[test]
fn mrv_registry_submit_verifier_adjustment_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xCCu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-adj-001"),
                1u64,
                ManagedBuffer::from(b"bafybundle-adj-001"),
                ManagedBuffer::from(&hash_32[..]),
            );
            sc.submit_verification_statement(
                ManagedBuffer::from(b"pai-adj-001"),
                1u64,
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"bafystmt-adj-001"),
                ManagedBuffer::from(b"approved"),
            );
            sc.submit_verifier_adjustment(
                ManagedBuffer::from(b"pai-adj-001"),
                1u64,
                ManagedBuffer::from(b"bafyadjustment001"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let pk = mrv_common::period_key(1u64);
            let count = sc.verifier_adjustment_count(&ManagedBuffer::from(b"pai-adj-001")).get(&pk).unwrap_or_default();
            assert_eq!(count, 1u64);
        });
}

#[test]
fn mrv_registry_submit_statement_unaccredited_vvb_fails_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xDDu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-unaccredited-001"),
                1u64,
                ManagedBuffer::from(b"bafybundle-ua-001"),
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "VVB_NOT_ACCREDITED: vvb_did must be registered via registerAccreditedVvb",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.submit_verification_statement(
                ManagedBuffer::from(b"pai-unaccredited-001"),
                1u64,
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"bafystmt-ua-001"),
                ManagedBuffer::from(b"approved"),
            );
        });
}

#[test]
fn mrv_registry_commit_bundle_wrong_hash_length_fails_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let short_hash: [u8; 16] = [0xEEu8; 16];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "bundle_hash must be 32 bytes (SHA-256)"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-badhash-001"),
                1u64,
                ManagedBuffer::from(b"bafybundle-bh-001"),
                ManagedBuffer::from(&short_hash[..]),
            );
        });
}

#[test]
fn mrv_registry_set_project_status_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);
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
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_project(
                ManagedBuffer::from(b"PRJ-001"),
                ManagedBuffer::from(b"tenant-1"),
                ManagedBuffer::from(b"asset-1"),
                ManagedBuffer::from(b"period-1"),
                ManagedBuffer::from(b"VM0042"),
                ManagedBuffer::from(b"pending"),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_project_status(
                ManagedBuffer::from(b"PRJ-001"),
                ManagedBuffer::from(b"active"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let project = sc.get_project_record(ManagedBuffer::from(b"PRJ-001"));
            assert!(project.is_some());
            let project = project.into_option().unwrap();
            assert_eq!(project.status, ManagedBuffer::from(b"active"));
        });
}
