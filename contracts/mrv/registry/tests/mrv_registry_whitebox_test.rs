use mrv_common::MrvGovernanceModule;
use mrv_governance::MrvGovernance;
use mrv_registry::MrvRegistry;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const OTHER: TestAddress = TestAddress::new("other");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-registry.mxsc.json");
const GOVERNANCE_SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-governance");
const GOVERNANCE_CODE_PATH: MxscPath =
    MxscPath::new("mxsc:../governance/output/mrv-governance.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/registry");
    world.register_contract(CODE_PATH, mrv_registry::ContractBuilder);
    world.register_contract(GOVERNANCE_CODE_PATH, mrv_governance::ContractBuilder);
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
const METHODOLOGY_STATUS: &[u8] = b"approved_internal";
const EVIDENCE_ID: &[u8] = b"evidence-001";
const EVIDENCE_HASH: &[u8] = b"sha256:evidence-001";
const VERIFICATION_CASE_ID: &[u8] = b"verification-001";
const LOT_ID: &[u8] = b"lot-001";
const METHODOLOGY_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_methodology_record_v1";
const PROJECT_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_project_record_v1";
const EVIDENCE_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_evidence_record_v1";
const VERIFICATION_CASE_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_verification_case_record_v1";
const ISSUANCE_LOT_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_issuance_lot_record_v1";
const REPORT_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_report_proof_v1";

fn append_len_prefixed_bytes<Api: multiversx_sc::api::ManagedTypeApi>(
    out: &mut ManagedBuffer<Api>,
    value: &[u8],
) {
    let len = value.len();
    out.append_bytes(&len.to_be_bytes());
    out.append(&ManagedBuffer::from(value));
}

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
fn mrv_registry_derives_canonical_ids_for_major_records() {
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
                1_710_720_000,
                0,
            );
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(FARM_ID),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"pending"),
            );
            sc.register_evidence(
                ManagedBuffer::from(EVIDENCE_ID),
                ManagedBuffer::from(b"project"),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(EVIDENCE_HASH),
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
                1,
            );
            sc.register_accredited_vvb(GOVERNANCE.to_managed_address());
            sc.create_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"project"),
                ManagedBuffer::from(PROJECT_ID),
                GOVERNANCE.to_managed_address(),
                0,
            );
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"assigned"),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                0,
            );
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"approved"),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::from(b"sha256:statement-001"),
                ManagedBuffer::from(b"drwa-attestation:token-001:verifier"),
                0,
            );
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(100_000u64),
                ManagedBuffer::new(),
            );
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
            let methodology_canonical = sc
                .get_methodology_canonical_id(
                    ManagedBuffer::from(METHODOLOGY_ID),
                    ManagedBuffer::from(METHODOLOGY_VERSION),
                )
                .into_option()
                .unwrap();
            let mut methodology_preimage = ManagedBuffer::new();
            methodology_preimage.append_bytes(METHODOLOGY_CANONICAL_ID_DOMAIN);
            methodology_preimage.append_bytes(&[0x00]);
            append_len_prefixed_bytes(&mut methodology_preimage, METHODOLOGY_ID);
            append_len_prefixed_bytes(&mut methodology_preimage, METHODOLOGY_VERSION);
            append_len_prefixed_bytes(&mut methodology_preimage, METHODOLOGY_DIGEST);
            let expected_methodology = sc
                .crypto()
                .sha256(&methodology_preimage)
                .as_managed_buffer()
                .clone();
            assert_eq!(methodology_canonical, expected_methodology);
            assert_eq!(methodology_canonical.to_boxed_bytes().len(), 32);

            let project_canonical = sc
                .get_project_canonical_id(ManagedBuffer::from(PROJECT_ID))
                .into_option()
                .unwrap();
            let mut project_preimage = ManagedBuffer::new();
            project_preimage.append_bytes(PROJECT_CANONICAL_ID_DOMAIN);
            project_preimage.append_bytes(&[0x00]);
            append_len_prefixed_bytes(&mut project_preimage, PROJECT_ID);
            append_len_prefixed_bytes(&mut project_preimage, TENANT_ID);
            append_len_prefixed_bytes(&mut project_preimage, FARM_ID);
            append_len_prefixed_bytes(&mut project_preimage, SEASON_ID);
            append_len_prefixed_bytes(&mut project_preimage, METHODOLOGY_ID);
            append_len_prefixed_bytes(&mut project_preimage, METHODOLOGY_VERSION);
            let expected_project = sc
                .crypto()
                .sha256(&project_preimage)
                .as_managed_buffer()
                .clone();
            assert_eq!(project_canonical, expected_project);
            assert_eq!(project_canonical.to_boxed_bytes().len(), 32);

            let evidence_canonical = sc
                .get_evidence_canonical_id(ManagedBuffer::from(EVIDENCE_ID))
                .into_option()
                .unwrap();
            let mut evidence_preimage = ManagedBuffer::new();
            evidence_preimage.append_bytes(EVIDENCE_CANONICAL_ID_DOMAIN);
            evidence_preimage.append_bytes(&[0x00]);
            append_len_prefixed_bytes(&mut evidence_preimage, EVIDENCE_ID);
            append_len_prefixed_bytes(&mut evidence_preimage, b"project");
            append_len_prefixed_bytes(&mut evidence_preimage, PROJECT_ID);
            append_len_prefixed_bytes(&mut evidence_preimage, EVIDENCE_HASH);
            append_len_prefixed_bytes(&mut evidence_preimage, EVIDENCE_MANIFEST_HASH);
            let expected_evidence = sc
                .crypto()
                .sha256(&evidence_preimage)
                .as_managed_buffer()
                .clone();
            assert_eq!(evidence_canonical, expected_evidence);
            assert_eq!(evidence_canonical.to_boxed_bytes().len(), 32);

            let verification_case_canonical = sc
                .get_verification_case_canonical_id(ManagedBuffer::from(VERIFICATION_CASE_ID))
                .into_option()
                .unwrap();
            let mut verification_case_preimage = ManagedBuffer::new();
            verification_case_preimage.append_bytes(VERIFICATION_CASE_CANONICAL_ID_DOMAIN);
            verification_case_preimage.append_bytes(&[0x00]);
            append_len_prefixed_bytes(&mut verification_case_preimage, VERIFICATION_CASE_ID);
            append_len_prefixed_bytes(&mut verification_case_preimage, b"project");
            append_len_prefixed_bytes(&mut verification_case_preimage, PROJECT_ID);
            let expected_verification_case = sc
                .crypto()
                .sha256(&verification_case_preimage)
                .as_managed_buffer()
                .clone();
            assert_eq!(verification_case_canonical, expected_verification_case);
            assert_eq!(verification_case_canonical.to_boxed_bytes().len(), 32);

            let issuance_lot_canonical = sc
                .get_issuance_lot_canonical_id(ManagedBuffer::from(LOT_ID))
                .into_option()
                .unwrap();
            let mut issuance_lot_preimage = ManagedBuffer::new();
            issuance_lot_preimage.append_bytes(ISSUANCE_LOT_CANONICAL_ID_DOMAIN);
            issuance_lot_preimage.append_bytes(&[0x00]);
            append_len_prefixed_bytes(&mut issuance_lot_preimage, LOT_ID);
            append_len_prefixed_bytes(&mut issuance_lot_preimage, PROJECT_ID);
            append_len_prefixed_bytes(&mut issuance_lot_preimage, VERIFICATION_CASE_ID);
            issuance_lot_preimage.append_bytes(&2026u64.to_be_bytes());
            let quantity_bytes = BigUint::from(100_000u64).to_bytes_be_buffer();
            issuance_lot_preimage.append_bytes(&quantity_bytes.len().to_be_bytes());
            issuance_lot_preimage.append(&quantity_bytes);
            append_len_prefixed_bytes(&mut issuance_lot_preimage, b"");
            let expected_issuance_lot = sc
                .crypto()
                .sha256(&issuance_lot_preimage)
                .as_managed_buffer()
                .clone();
            assert_eq!(issuance_lot_canonical, expected_issuance_lot);
            assert_eq!(issuance_lot_canonical.to_boxed_bytes().len(), 32);

            let report_canonical = sc
                .get_report_canonical_id(ManagedBuffer::from(REPORT_ID))
                .into_option()
                .unwrap();
            let mut report_preimage = ManagedBuffer::new();
            report_preimage.append_bytes(REPORT_CANONICAL_ID_DOMAIN);
            report_preimage.append_bytes(&[0x00]);
            append_len_prefixed_bytes(&mut report_preimage, REPORT_ID);
            append_len_prefixed_bytes(&mut report_preimage, TENANT_ID);
            append_len_prefixed_bytes(&mut report_preimage, FARM_ID);
            append_len_prefixed_bytes(&mut report_preimage, SEASON_ID);
            append_len_prefixed_bytes(&mut report_preimage, PROJECT_ID);
            let expected_report = sc
                .crypto()
                .sha256(&report_preimage)
                .as_managed_buffer()
                .clone();
            assert_eq!(report_canonical, expected_report);
            assert_eq!(report_canonical.to_boxed_bytes().len(), 32);
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
fn mrv_registry_rejects_owner_fallback_after_governance_is_active() {
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
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.anchor_report_v2(
                ManagedBuffer::from(b"report-owner-blocked"),
                ManagedBuffer::from(b"tenant-owner-blocked"),
                ManagedBuffer::from(b"farm-owner-blocked"),
                ManagedBuffer::from(b"season-owner-blocked"),
                ManagedBuffer::from(b"project-owner-blocked"),
                ManagedBuffer::from(b"sha256:report-owner-blocked"),
                ManagedBuffer::from(b"sha256"),
                ManagedBuffer::from(b"json-c14n-v1"),
                1,
                1_710_720_000,
                ManagedBuffer::from(b"sha3-256:evidence-owner-blocked"),
            );
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
                record.methodology_id.to_boxed_bytes().as_slice(),
                METHODOLOGY_ID
            );
            assert_eq!(
                record.version_label.to_boxed_bytes().as_slice(),
                METHODOLOGY_VERSION
            );
            assert_eq!(
                record.pack_digest.to_boxed_bytes().as_slice(),
                METHODOLOGY_DIGEST
            );
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
            sc.register_methodology(
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(b"1.1.0"),
                ManagedBuffer::from(b"sha256:methodology-pack-001-1"),
                ManagedBuffer::from(b"approved_internal"),
                1_767_225_600,
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
fn mrv_registry_rejects_methodology_reregistration_with_window_drift() {
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
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "conflicting methodology record"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_methodology(
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(METHODOLOGY_DIGEST),
                ManagedBuffer::from(METHODOLOGY_STATUS),
                1_735_689_601,
                0,
            );
        });
}

#[test]
fn mrv_registry_rejects_supersession_without_replacement_record() {
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
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "ENTITY_NOT_FOUND: replacement_methodology_record",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.supersede_methodology(
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"1.1.0"),
                1_767_225_600,
            );
        });
}

#[test]
fn mrv_registry_rejects_invalid_methodology_status_transition() {
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
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "invalid methodology transition"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_methodology_approval_status(
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"superseded"),
            );
        });
}

#[test]
fn mrv_registry_rejects_unknown_project_status() {
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
        .returns(ExpectError(4u64, "invalid project status"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(b"PRJ-INVALID-001"),
                ManagedBuffer::from(b"tenant-1"),
                ManagedBuffer::from(b"asset-1"),
                ManagedBuffer::from(b"period-1"),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"archived"),
            );
        });
}

#[test]
fn mrv_registry_rejects_project_with_unknown_methodology_record() {
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
        .returns(ExpectError(4u64, "ENTITY_NOT_FOUND: methodology_record"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_project(
                ManagedBuffer::from(b"PRJ-UNKNOWN-METHODOLOGY"),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(b"asset-001"),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"active"),
            );
        });
}

#[test]
fn mrv_registry_rejects_project_with_unapproved_methodology_record() {
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
                1_710_720_000,
                0,
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "methodology must be approved_internal"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_project(
                ManagedBuffer::from(b"PRJ-UNAPPROVED-METHODOLOGY"),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(b"asset-001"),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"active"),
            );
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
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(b"asset-001"),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
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

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let project = sc
                .get_project_record(ManagedBuffer::from(PROJECT_ID))
                .into_option()
                .unwrap();
            assert_eq!(project.tenant_id.to_boxed_bytes().as_slice(), TENANT_ID);
            assert_eq!(
                project.reporting_period_id.to_boxed_bytes().as_slice(),
                SEASON_ID
            );
            assert_eq!(
                project
                    .methodology_version_label
                    .to_boxed_bytes()
                    .as_slice(),
                METHODOLOGY_VERSION
            );
            assert_eq!(sc.get_project_records_count(), 1usize);

            let evidence = sc
                .get_evidence_record(ManagedBuffer::from(EVIDENCE_ID))
                .into_option()
                .unwrap();
            assert_eq!(
                evidence.evidence_hash.to_boxed_bytes().as_slice(),
                EVIDENCE_HASH
            );
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
            sc.register_accredited_vvb(GOVERNANCE.to_managed_address());
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
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::from(b"sha256:statement-001"),
                ManagedBuffer::from(b"drwa-attestation:token-001:verifier"),
                1_710_720_020,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let case_record = sc
                .get_verification_case(ManagedBuffer::from(VERIFICATION_CASE_ID))
                .into_option()
                .unwrap();
            assert_eq!(case_record.status.to_boxed_bytes().as_slice(), b"approved");
            assert_eq!(
                case_record
                    .verifier_statement_hash
                    .to_boxed_bytes()
                    .as_slice(),
                b"sha256:statement-001"
            );
            assert_eq!(sc.get_verification_cases_count(), 1usize);
        });
}

#[test]
fn mrv_registry_rejects_approved_verification_case_without_evidence_refs() {
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
            sc.register_accredited_vvb(GOVERNANCE.to_managed_address());
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"assigned"),
                OTHER.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_710_720_010,
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "approved verification requires verifier statement hash",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"approved"),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::from(b"drwa-attestation:token-001:verifier"),
                1_710_720_020,
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "approved verification requires verifier attestation ref",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"approved"),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::from(b"sha256:statement-001"),
                ManagedBuffer::new(),
                1_710_720_020,
            );
        });
}

#[test]
fn mrv_registry_rejects_approved_case_not_submitted_by_assignee_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
            sc.create_verification_case(
                ManagedBuffer::from(b"verification-forged-approval-001"),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(REPORT_ID),
                VVB_ADDRESS.to_managed_address(),
                1_710_720_000,
            );
            sc.update_verification_case(
                ManagedBuffer::from(b"verification-forged-approval-001"),
                ManagedBuffer::from(b"assigned"),
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_710_720_010,
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "VVB_CALLER_MISMATCH: approved verification must be submitted by assignee",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.update_verification_case(
                ManagedBuffer::from(b"verification-forged-approval-001"),
                ManagedBuffer::from(b"approved"),
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"sha256:forged-statement"),
                ManagedBuffer::from(b"drwa-attestation:forged"),
                1_710_720_020,
            );
        });
}

#[test]
fn mrv_registry_uses_block_timestamp_for_evidence_and_verification_updates() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);
    world
        .current_block()
        .block_timestamp_seconds(1_710_720_777u64);

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
            sc.register_evidence(
                ManagedBuffer::from(EVIDENCE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(REPORT_ID),
                ManagedBuffer::from(EVIDENCE_HASH),
                ManagedBuffer::from(EVIDENCE_MANIFEST_HASH),
                123u64,
            );
            sc.create_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(REPORT_ID),
                OTHER.to_managed_address(),
                456u64,
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_720_888u64);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"assigned"),
                OTHER.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                789u64,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let evidence = sc
                .get_evidence_record(ManagedBuffer::from(EVIDENCE_ID))
                .into_option()
                .unwrap();
            assert_eq!(evidence.submitted_at, 123u64);

            let verification_case = sc
                .get_verification_case(ManagedBuffer::from(VERIFICATION_CASE_ID))
                .into_option()
                .unwrap();
            assert_eq!(verification_case.updated_at, 1_710_720_888u64);
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
            // B-01: project must exist before issuance; the original test
            // skipped this step which the new invariant correctly rejects.
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(b"asset-001"),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"active"),
            );
            sc.create_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(REPORT_ID),
                OTHER.to_managed_address(),
                1_710_720_000,
            );
            sc.register_accredited_vvb(GOVERNANCE.to_managed_address());
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
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::from(b"sha256:statement-001"),
                ManagedBuffer::from(b"drwa-attestation:token-001:verifier"),
                1_710_720_020,
            );
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(105_000u64),
                ManagedBuffer::new(),
            );
            sc.retire_issuance_lot(ManagedBuffer::from(LOT_ID));
            // B-01: replacement_lot_id is empty here; the full
            // forward/back pointer lineage is exercised by the dedicated
            // `mrv_registry_accepts_full_replacement_lineage` test.
            sc.reverse_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                BigUint::from(20_000u64),
                ManagedBuffer::new(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let lot = sc
                .get_issuance_lot(ManagedBuffer::from(LOT_ID))
                .into_option()
                .unwrap();
            assert_eq!(lot.status.to_boxed_bytes().as_slice(), b"reversed");
            assert_eq!(lot.vintage, 2026);
            assert_eq!(lot.quantity_scaled, BigUint::from(105_000u64));
            assert_eq!(lot.reversed_amount_scaled, BigUint::from(20_000u64));
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
            assert_eq!(
                sc.get_report_proof_amendment_count(ManagedBuffer::from(REPORT_ID)),
                1u64
            );
            let prior_proof = sc
                .get_report_proof_amendment(ManagedBuffer::from(REPORT_ID), 0u64)
                .into_option()
                .unwrap();
            assert_eq!(
                prior_proof.public_season_id.to_boxed_bytes().as_slice(),
                SEASON_ID
            );
            assert_eq!(
                prior_proof.report_hash.to_boxed_bytes().as_slice(),
                REPORT_HASH
            );
            assert_eq!(prior_proof.methodology_version, 1u64);
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

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            register_default_approved_methodology(&sc);
        });
}

fn register_default_approved_methodology(sc: &mrv_registry::ContractObj<DebugApi>) {
    sc.register_methodology(
        ManagedBuffer::from(METHODOLOGY_ID),
        ManagedBuffer::from(METHODOLOGY_VERSION),
        ManagedBuffer::from(METHODOLOGY_DIGEST),
        ManagedBuffer::from(b"approved_internal"),
        1_710_720_000,
        0,
    );
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
            assert_eq!(
                bundle.bundle_cid.to_boxed_bytes().as_slice(),
                b"bafybundle001"
            );
            assert_eq!(bundle.bundle_hash.len(), 32);
        });
}

#[test]
fn mrv_registry_commit_execution_bundle_rejects_oversized_cid_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xABu8; 32];
    let oversized_cid = vec![b'a'; 257];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "bundle_cid exceeds maximum length"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-bundle-too-large"),
                1u64,
                ManagedBuffer::from(oversized_cid.as_slice()),
                ManagedBuffer::from(&hash_32[..]),
            );
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
        });

    world
        .tx()
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
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
fn mrv_registry_rejects_statement_not_submitted_by_named_vvb_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xB2u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-forged-stmt-001"),
                1u64,
                ManagedBuffer::from(b"bafybundle-forged-stmt-001"),
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "VVB_CALLER_MISMATCH: statement must be submitted by vvb_did",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.submit_verification_statement(
                ManagedBuffer::from(b"pai-forged-stmt-001"),
                1u64,
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"bafystmt-forged-001"),
                ManagedBuffer::from(b"approved"),
            );
        });
}

#[test]
fn mrv_registry_submit_verification_statement_uses_governance_accreditation_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(GOVERNANCE_CODE_PATH)
        .new_address(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_verifier_accreditation(
                ManagedBuffer::from(b"verifier-gov-001"),
                VVB_ADDRESS.to_managed_address(),
                true,
                ManagedBuffer::from(b"vvb"),
            );
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"verifier-gov-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"verifier-gov-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"verifier-gov-001"));
        });

    let hash_32: [u8; 32] = [0xB1u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC_ADDRESS.to_managed_address());
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-gov-stmt-001"),
                1u64,
                ManagedBuffer::from(b"bafybundle-gov-stmt-001"),
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .tx()
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.submit_verification_statement(
                ManagedBuffer::from(b"pai-gov-stmt-001"),
                1u64,
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"bafystmt-gov-001"),
                ManagedBuffer::from(b"approved"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let stmt = sc
                .get_verification_statement(ManagedBuffer::from(b"pai-gov-stmt-001"), 1u64)
                .into_option()
                .unwrap();
            assert_eq!(stmt.vvb_did, VVB_ADDRESS.to_managed_address());
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
fn mrv_registry_local_vvb_registry_disabled_in_governance_mode_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(GOVERNANCE_CODE_PATH)
        .new_address(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC_ADDRESS.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "VVB_REGISTRY_CANONICALIZED_TO_GOVERNANCE: local VVB registry mutations are disabled while governanceReadAddress is configured",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
        });
}

#[test]
fn mrv_registry_governance_pause_blocks_mutations_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(GOVERNANCE_CODE_PATH)
        .new_address(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-registry-001"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-registry-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-registry-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-registry-001"));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC_ADDRESS.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(b"PRJ-PAUSED-001"),
                ManagedBuffer::from(b"tenant-1"),
                ManagedBuffer::from(b"asset-1"),
                ManagedBuffer::from(b"period-1"),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"pending"),
            );
        });
}

#[test]
fn mrv_registry_requires_governance_read_for_smart_contract_authority_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

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
        .raw_deploy()
        .code(GOVERNANCE_CODE_PATH)
        .new_address(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE_SC_ADDRESS.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE_SC_ADDRESS)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.accept_governance();
        });

    world
        .tx()
        .from(GOVERNANCE_SC_ADDRESS)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_READ_NOT_CONFIGURED"))
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
        });

    world
        .tx()
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
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
            let count = sc
                .verifier_adjustment_count(&ManagedBuffer::from(b"pai-adj-001"))
                .get(&pk)
                .unwrap_or_default();
            assert_eq!(count, 1u64);
        });
}

#[test]
fn mrv_registry_rejects_adjustment_not_submitted_by_statement_vvb_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xC1u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
            sc.commit_execution_bundle(
                ManagedBuffer::from(b"pai-forged-adj-001"),
                1u64,
                ManagedBuffer::from(b"bafybundle-forged-adj-001"),
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .tx()
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.submit_verification_statement(
                ManagedBuffer::from(b"pai-forged-adj-001"),
                1u64,
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"bafystmt-forged-adj-001"),
                ManagedBuffer::from(b"approved"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "VVB_CALLER_MISMATCH: adjustment must be submitted by statement vvb_did",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.submit_verifier_adjustment(
                ManagedBuffer::from(b"pai-forged-adj-001"),
                1u64,
                ManagedBuffer::from(b"bafyadjustment-forged-001"),
            );
        });
}

#[test]
fn mrv_registry_submit_verifier_adjustment_cap_enforced_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xCDu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let pai_id = ManagedBuffer::from(b"pai-adj-cap-001");

            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
            sc.commit_execution_bundle(
                pai_id.clone(),
                1u64,
                ManagedBuffer::from(b"bafybundle-adj-cap-001"),
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .tx()
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let pai_id = ManagedBuffer::from(b"pai-adj-cap-001");

            sc.submit_verification_statement(
                pai_id.clone(),
                1u64,
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"bafystmt-adj-cap-001"),
                ManagedBuffer::from(b"approved"),
            );

            for idx in 1u8..=5u8 {
                let mut cid = ManagedBuffer::new();
                cid.append_bytes(b"bafyadjustment-cap-001-");
                cid.append_bytes(&[b'0' + idx]);
                sc.submit_verifier_adjustment(pai_id.clone(), 1u64, cid);
            }
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let pk = mrv_common::period_key(1u64);
            let count = sc
                .verifier_adjustment_count(&ManagedBuffer::from(b"pai-adj-cap-001"))
                .get(&pk)
                .unwrap_or_default();
            assert_eq!(count, 5u64);
        });

    world
        .tx()
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "verifier adjustment cap exceeded"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.submit_verifier_adjustment(
                ManagedBuffer::from(b"pai-adj-cap-001"),
                1u64,
                ManagedBuffer::from(b"bafyadjustment-cap-001-6"),
            );
        });
}

#[test]
fn mrv_registry_submit_verifier_adjustment_pathological_counter_is_still_cap_blocked_rs() {
    let mut world = world();
    deploy_registry(&mut world);

    let hash_32: [u8; 32] = [0xCEu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let pai_id = ManagedBuffer::from(b"pai-adj-overflow-001");

            sc.register_accredited_vvb(VVB_ADDRESS.to_managed_address());
            sc.commit_execution_bundle(
                pai_id.clone(),
                1u64,
                ManagedBuffer::from(b"bafybundle-adj-overflow-001"),
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .tx()
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let pai_id = ManagedBuffer::from(b"pai-adj-overflow-001");
            let pk = mrv_common::period_key(1u64);

            sc.submit_verification_statement(
                pai_id.clone(),
                1u64,
                VVB_ADDRESS.to_managed_address(),
                ManagedBuffer::from(b"bafystmt-adj-overflow-001"),
                ManagedBuffer::from(b"approved"),
            );
            sc.verifier_adjustment_count(&pai_id).insert(pk, u64::MAX);
        });

    world
        .tx()
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "verifier adjustment cap exceeded"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.submit_verifier_adjustment(
                ManagedBuffer::from(b"pai-adj-overflow-001"),
                1u64,
                ManagedBuffer::from(b"bafyadjustment-overflow-001"),
            );
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
        .from(VVB_ADDRESS)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "VVB_NOT_ACCREDITED: vvb_did must be accredited via governance or local registry",
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
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(b"PRJ-001"),
                ManagedBuffer::from(b"tenant-1"),
                ManagedBuffer::from(b"asset-1"),
                ManagedBuffer::from(b"period-1"),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"pending"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
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

#[test]
fn mrv_registry_rejects_invalid_project_status_transition() {
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
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(b"PRJ-TRANSITION-001"),
                ManagedBuffer::from(b"tenant-1"),
                ManagedBuffer::from(b"asset-1"),
                ManagedBuffer::from(b"period-1"),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"active"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "invalid project status transition"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_project_status(
                ManagedBuffer::from(b"PRJ-TRANSITION-001"),
                ManagedBuffer::from(b"pending"),
            );
        });
}

// ── B-01 (AUD-001) invariant tests ────────────────────────────────────────
//
// These tests lock in the issuance-lifecycle invariants added in the
// reconciled-audit remediation pass. They cover:
//   1. project existence requirement on `create_issuance_lot`
//   2. verification-case existence requirement on `create_issuance_lot`
//   3. verification-case status=="approved" requirement
//   4. reverse_issuance_lot with unknown replacement_lot_id
//   5. reverse_issuance_lot with replacement whose back-pointer is wrong
//   6. happy-path full replacement lineage accepts

fn deploy_with_governance(world: &mut ScenarioWorld) {
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
            register_default_approved_methodology(&sc);
        });
}

fn register_project_and_approved_case(world: &mut ScenarioWorld) {
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(b"asset-001"),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"active"),
            );
            sc.create_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(REPORT_ID),
                OTHER.to_managed_address(),
                1_710_720_000,
            );
            sc.register_accredited_vvb(GOVERNANCE.to_managed_address());
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"assigned"),
                OTHER.to_managed_address(),
                ManagedBuffer::new(),
                ManagedBuffer::new(),
                1_710_720_010,
            );
            sc.update_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"approved"),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::from(b"sha256:statement-001"),
                ManagedBuffer::from(b"drwa-attestation:token-001:verifier"),
                1_710_720_020,
            );
        });
}

fn create_test_issuance_lot(world: &mut ScenarioWorld, lot_id: &[u8]) {
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.create_issuance_lot(
                ManagedBuffer::from(lot_id),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(105_000u64),
                ManagedBuffer::new(),
            );
        });
}

#[test]
fn mrv_registry_terminal_lifecycle_requires_carbon_credit_when_configured() {
    let mut world = world();
    deploy_with_governance(&mut world);
    register_project_and_approved_case(&mut world);
    create_test_issuance_lot(&mut world, LOT_ID);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.set_carbon_credit_lifecycle_address(OTHER.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "ISSUANCE_LOT_LIFECYCLE_CANONICALIZED_TO_CARBON_CREDIT",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.retire_issuance_lot(ManagedBuffer::from(LOT_ID));
        });

    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.retire_issuance_lot(ManagedBuffer::from(LOT_ID));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let lot = sc
                .get_issuance_lot(ManagedBuffer::from(LOT_ID))
                .into_option()
                .unwrap();
            assert_eq!(lot.status.to_boxed_bytes().as_slice(), b"retired");
        });
}

#[test]
fn mrv_registry_rejects_issuance_lot_with_unknown_project() {
    let mut world = world();
    deploy_with_governance(&mut world);
    // Do NOT register the project. Create the verification case only.
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
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "ENTITY_NOT_FOUND: project"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(100_000u64),
                ManagedBuffer::new(),
            );
        });
}

#[test]
fn mrv_registry_rejects_issuance_lot_with_unknown_verification_case() {
    let mut world = world();
    deploy_with_governance(&mut world);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(b"asset-001"),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"active"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "ENTITY_NOT_FOUND: verification_case"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(100_000u64),
                ManagedBuffer::new(),
            );
        });
}

#[test]
fn mrv_registry_rejects_issuance_lot_with_non_approved_verification_case() {
    let mut world = world();
    deploy_with_governance(&mut world);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            register_default_approved_methodology(&sc);
            sc.register_project(
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(TENANT_ID),
                ManagedBuffer::from(b"asset-001"),
                ManagedBuffer::from(SEASON_ID),
                ManagedBuffer::from(METHODOLOGY_ID),
                ManagedBuffer::from(METHODOLOGY_VERSION),
                ManagedBuffer::from(b"active"),
            );
            sc.create_verification_case(
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                ManagedBuffer::from(b"report"),
                ManagedBuffer::from(REPORT_ID),
                OTHER.to_managed_address(),
                1_710_720_000,
            );
            // Leave the case in status "pending_assignment" — NOT approved.
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "VERIFICATION_CASE_NOT_APPROVED: issuance requires an approved verification case",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(100_000u64),
                ManagedBuffer::new(),
            );
        });
}

#[test]
fn mrv_registry_rejects_reverse_with_unknown_replacement_lot() {
    let mut world = world();
    deploy_with_governance(&mut world);
    register_project_and_approved_case(&mut world);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(100_000u64),
                ManagedBuffer::new(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "ENTITY_NOT_FOUND: replacement_lot"))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.reverse_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                BigUint::from(20_000u64),
                ManagedBuffer::from(b"lot-does-not-exist"),
            );
        });
}

#[test]
fn mrv_registry_rejects_reverse_amount_above_quantity() {
    let mut world = world();
    deploy_with_governance(&mut world);
    register_project_and_approved_case(&mut world);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(100_000u64),
                ManagedBuffer::new(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "reversed amount exceeds issuance lot quantity",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.reverse_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                BigUint::from(100_001u64),
                ManagedBuffer::new(),
            );
        });
}

#[test]
fn mrv_registry_rejects_reverse_with_mismatched_replacement_lineage() {
    let mut world = world();
    deploy_with_governance(&mut world);
    register_project_and_approved_case(&mut world);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            // Original lot A.
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(100_000u64),
                ManagedBuffer::new(),
            );
            // Unrelated lot B whose replacement_for_lot_id is EMPTY, so
            // it is not a valid replacement for A.
            sc.create_issuance_lot(
                ManagedBuffer::from(b"lot-unrelated"),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(30_000u64),
                ManagedBuffer::new(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "REPLACEMENT_LINEAGE_MISMATCH: replacement lot does not cite this lot as its predecessor",
        ))
        .whitebox(mrv_registry::contract_obj, |sc| {
            sc.reverse_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                BigUint::from(20_000u64),
                ManagedBuffer::from(b"lot-unrelated"),
            );
        });
}

#[test]
fn mrv_registry_accepts_full_replacement_lineage() {
    let mut world = world();
    deploy_with_governance(&mut world);
    register_project_and_approved_case(&mut world);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            // Original lot A.
            sc.create_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(100_000u64),
                ManagedBuffer::new(),
            );
            // Replacement lot B with correct back-pointer to A.
            sc.create_issuance_lot(
                ManagedBuffer::from(b"lot-replacement"),
                ManagedBuffer::from(PROJECT_ID),
                ManagedBuffer::from(VERIFICATION_CASE_ID),
                2026,
                BigUint::from(80_000u64),
                ManagedBuffer::from(LOT_ID),
            );
            // Reverse A citing B — lineage is consistent.
            sc.reverse_issuance_lot(
                ManagedBuffer::from(LOT_ID),
                BigUint::from(20_000u64),
                ManagedBuffer::from(b"lot-replacement"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_registry::contract_obj, |sc| {
            let original = sc
                .get_issuance_lot(ManagedBuffer::from(LOT_ID))
                .into_option()
                .unwrap();
            assert_eq!(original.status.to_boxed_bytes().as_slice(), b"reversed");
            assert_eq!(original.reversed_amount_scaled, BigUint::from(20_000u64));
            let replacement = sc
                .get_issuance_lot(ManagedBuffer::from(b"lot-replacement"))
                .into_option()
                .unwrap();
            assert_eq!(
                replacement
                    .replacement_for_lot_id
                    .to_boxed_bytes()
                    .as_slice(),
                LOT_ID
            );
        });
}
