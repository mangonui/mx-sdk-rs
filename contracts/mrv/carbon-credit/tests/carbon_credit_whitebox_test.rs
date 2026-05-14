use mrv_buffer_pool::BufferPool;
use mrv_carbon_credit::CarbonCreditModule;
use mrv_common::MrvGovernanceModule;
use mrv_governance::MrvGovernance;
use multiversx_sc::types::{ManagedBuffer, TokenIdentifier};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const BUFFER_POOL: TestAddress = TestAddress::new("buffer-pool");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-carbon-credit");
const BUFFER_POOL_SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-buffer-pool");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-carbon-credit.mxsc.json");
const BUFFER_POOL_CODE_PATH: MxscPath =
    MxscPath::new("mxsc:../buffer-pool/output/mrv-buffer-pool.mxsc.json");
const GOVERNANCE_SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-governance");
const GOVERNANCE_CODE_PATH: MxscPath =
    MxscPath::new("mxsc:../governance/output/mrv-governance.mxsc.json");
const DVCU_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DVCU-123456");
const DGSC_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DGSC-123456");
const BUFFER_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("BUFR-123456");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/carbon-credit");
    world.register_contract(CODE_PATH, mrv_carbon_credit::ContractBuilder);
    world.register_contract(BUFFER_POOL_CODE_PATH, mrv_buffer_pool::ContractBuilder);
    world.register_contract(GOVERNANCE_CODE_PATH, mrv_governance::ContractBuilder);
    world
}

#[test]
fn carbon_credit_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(BUFFER_POOL).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
            assert_eq!(
                sc.buffer_pool_addr().get(),
                BUFFER_POOL.to_managed_address()
            );
            assert!(sc.dvcu_token_id().is_empty());
            assert!(sc.dgsc_token_id().is_empty());
            assert_eq!(sc.total_dvcu_minted().get(), BigUint::zero());
            assert_eq!(sc.total_dvcu_burned().get(), BigUint::zero());
            assert_eq!(sc.total_dgsc_minted().get(), BigUint::zero());
            assert_eq!(sc.total_dgsc_burned().get(), BigUint::zero());
        });
}

#[test]
fn carbon_credit_token_configuration_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(BUFFER_POOL).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_dvcu_token_id(TokenIdentifier::from("DVCU-123456"));
            sc.set_dgsc_token_id(TokenIdentifier::from("DGSC-123456"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            assert_eq!(
                sc.dvcu_token_id().get(),
                TokenIdentifier::from("DVCU-123456")
            );
            assert_eq!(
                sc.dgsc_token_id().get(),
                TokenIdentifier::from("DGSC-123456")
            );
        });
}

#[test]
fn carbon_credit_token_id_pre_live_replacement_allowed_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(BUFFER_POOL).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_dvcu_token_id(TokenIdentifier::from("DVCU-123456"));
            sc.set_dvcu_token_id(TokenIdentifier::from("DVCU-654321"));
            sc.set_dgsc_token_id(TokenIdentifier::from("DGSC-123456"));
            sc.set_dgsc_token_id(TokenIdentifier::from("DGSC-654321"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            assert_eq!(
                sc.dvcu_token_id().get(),
                TokenIdentifier::from("DVCU-654321")
            );
            assert_eq!(
                sc.dgsc_token_id().get(),
                TokenIdentifier::from("DGSC-654321")
            );
        });
}

#[test]
fn carbon_credit_token_id_replacement_after_live_accounting_fails_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(BUFFER_POOL).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_dvcu_token_id(TokenIdentifier::from("DVCU-123456"));
            sc.set_dgsc_token_id(TokenIdentifier::from("DGSC-123456"));
            sc.total_dvcu_minted().set(BigUint::from(1u64));
            sc.total_dgsc_burned().set(BigUint::from(1u64));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "DVCU_TOKEN_ID_LOCKED"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_dvcu_token_id(TokenIdentifier::from("DVCU-654321"));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "DGSC_TOKEN_ID_LOCKED"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_dgsc_token_id(TokenIdentifier::from("DGSC-654321"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            assert_eq!(
                sc.dvcu_token_id().get(),
                TokenIdentifier::from("DVCU-123456")
            );
            assert_eq!(
                sc.dgsc_token_id().get(),
                TokenIdentifier::from("DGSC-123456")
            );
        });
}

#[test]
fn carbon_credit_register_ime_record_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(BUFFER_POOL).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            domain_codes.push(ManagedBuffer::from(b"SG"));
            domain_codes.push(ManagedBuffer::from(b"MY"));
            sc.register_ime_record(
                ManagedBuffer::from(b"project-001"),
                ManagedBuffer::from(b"sha256:image-digest-001"),
                ManagedBuffer::from(b"sha256:param-pack-001"),
                ManagedBuffer::from(b"sha256:calibration-001"),
                ManagedBuffer::from(b"sha256:strata-protocol-001"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let ime = sc
                .get_ime_record(ManagedBuffer::from(b"project-001"))
                .into_option()
                .unwrap();
            assert_eq!(ime.project_id.to_boxed_bytes().as_slice(), b"project-001");
            assert_eq!(
                ime.science_service_image_digest.to_boxed_bytes().as_slice(),
                b"sha256:image-digest-001"
            );
            assert!(!ime.revoked);
            assert_eq!(ime.domain_codes.len(), 2);
            assert_eq!(
                sc.get_active_ime_record_version(ManagedBuffer::from(b"project-001")),
                1u64
            );
            assert_eq!(
                sc.get_ime_record_version_count(ManagedBuffer::from(b"project-001")),
                1u64
            );
            let versioned_ime = sc
                .get_ime_record_version(ManagedBuffer::from(b"project-001"), 1u64)
                .into_option()
                .unwrap();
            assert_eq!(
                versioned_ime
                    .science_service_image_digest
                    .to_boxed_bytes()
                    .as_slice(),
                b"sha256:image-digest-001"
            );
        });
}

#[test]
fn carbon_credit_rejects_unbounded_ime_domain_codes_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "too many IME domain codes"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            for i in 0..65u8 {
                domain_codes.push(ManagedBuffer::from(&[b'A', i][..]));
            }
            sc.register_ime_record(
                ManagedBuffer::from(b"project-001"),
                ManagedBuffer::from(b"sha256:image-digest-001"),
                ManagedBuffer::from(b"sha256:param-pack-001"),
                ManagedBuffer::from(b"sha256:calibration-001"),
                ManagedBuffer::from(b"sha256:strata-protocol-001"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes,
            );
        });
}

#[test]
fn carbon_credit_register_ime_record_versions_are_append_only_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(BUFFER_POOL).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes_v1 = MultiValueEncoded::new();
            domain_codes_v1.push(ManagedBuffer::from(b"SG"));
            sc.register_ime_record(
                ManagedBuffer::from(b"project-versioned"),
                ManagedBuffer::from(b"sha256:image-v1"),
                ManagedBuffer::from(b"sha256:param-v1"),
                ManagedBuffer::from(b"sha256:cal-v1"),
                ManagedBuffer::from(b"sha256:strata-v1"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes_v1,
            );

            let mut domain_codes_v2 = MultiValueEncoded::new();
            domain_codes_v2.push(ManagedBuffer::from(b"SG"));
            domain_codes_v2.push(ManagedBuffer::from(b"MY"));
            sc.register_ime_record(
                ManagedBuffer::from(b"project-versioned"),
                ManagedBuffer::from(b"sha256:image-v2"),
                ManagedBuffer::from(b"sha256:param-v2"),
                ManagedBuffer::from(b"sha256:cal-v2"),
                ManagedBuffer::from(b"sha256:strata-v2"),
                ManagedBuffer::from(b"2.0.0"),
                9_999_999_999u64,
                domain_codes_v2,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            assert_eq!(
                sc.get_active_ime_record_version(ManagedBuffer::from(b"project-versioned")),
                2u64
            );
            assert_eq!(
                sc.get_ime_record_version_count(ManagedBuffer::from(b"project-versioned")),
                2u64
            );
            let v1 = sc
                .get_ime_record_version(ManagedBuffer::from(b"project-versioned"), 1u64)
                .into_option()
                .unwrap();
            let v2 = sc
                .get_ime_record_version(ManagedBuffer::from(b"project-versioned"), 2u64)
                .into_option()
                .unwrap();
            assert_eq!(
                v1.science_service_image_digest.to_boxed_bytes().as_slice(),
                b"sha256:image-v1"
            );
            assert_eq!(
                v2.science_service_image_digest.to_boxed_bytes().as_slice(),
                b"sha256:image-v2"
            );
            assert_eq!(
                sc.get_ime_record(ManagedBuffer::from(b"project-versioned"))
                    .into_option()
                    .unwrap()
                    .science_service_image_digest
                    .to_boxed_bytes()
                    .as_slice(),
                b"sha256:image-v2"
            );
        });
}

#[test]
fn carbon_credit_revoke_ime_record_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(BUFFER_POOL).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            domain_codes.push(ManagedBuffer::from(b"SG"));
            sc.register_ime_record(
                ManagedBuffer::from(b"project-002"),
                ManagedBuffer::from(b"sha256:image-002"),
                ManagedBuffer::from(b"sha256:param-002"),
                ManagedBuffer::from(b"sha256:cal-002"),
                ManagedBuffer::from(b"sha256:strata-002"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes,
            );
            sc.revoke_ime_record(ManagedBuffer::from(b"project-002"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let ime = sc
                .get_ime_record(ManagedBuffer::from(b"project-002"))
                .into_option()
                .unwrap();
            assert!(ime.revoked);
        });
}

/// Helper: deploys carbon-credit and registers a valid IME for project-010.
fn deploy_and_register_ime(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(BUFFER_POOL_CODE_PATH)
        .new_address(BUFFER_POOL_SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                SC_ADDRESS.to_managed_address(),
            );
        });

    world.set_esdt_local_roles(
        BUFFER_POOL_SC_ADDRESS.to_address(),
        BUFFER_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(BUFFER_POOL_SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.set_buffer_token_id(TokenIdentifier::from(BUFFER_TOKEN.as_bytes()));
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL_SC_ADDRESS.to_managed_address(),
            );
        });

    world.set_esdt_local_roles(
        SC_ADDRESS.to_address(),
        DVCU_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );
    world.set_esdt_local_roles(
        SC_ADDRESS.to_address(),
        DGSC_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_dvcu_token_id(TokenIdentifier::from(DVCU_TOKEN.as_bytes()));
            sc.set_dgsc_token_id(TokenIdentifier::from(DGSC_TOKEN.as_bytes()));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            domain_codes.push(ManagedBuffer::from(b"SG"));
            sc.register_ime_record(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"sha256:image-010"),
                ManagedBuffer::from(b"sha256:param-010"),
                ManagedBuffer::from(b"sha256:cal-010"),
                ManagedBuffer::from(b"sha256:strata-010"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes,
            );
        });
}

fn configure_gsoc_governance(world: &mut ScenarioWorld) {
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);
    world.account(GSOC_VERIFIER).nonce(1).balance(1_000_000u64);

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
            sc.propose_gsoc_verifier(
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"credentials-cid-001"),
                ManagedBuffer::from(b"SG"),
            );
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_gsoc_verifier_proposal(1u64);
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_gsoc_verifier_proposal(1u64);
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_gsoc_verifier_proposal(1u64);
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC_ADDRESS.to_managed_address());
        });
}

fn make_bundle_ref<M: multiversx_sc::api::ManagedTypeApi>()
-> mrv_carbon_credit::ExecutionBundleRef<M> {
    mrv_carbon_credit::ExecutionBundleRef {
        science_service_image_digest: ManagedBuffer::from(b"sha256:image-010"),
        parameter_pack_hash: ManagedBuffer::from(b"sha256:param-010"),
        calibration_dataset_hash: ManagedBuffer::from(b"sha256:cal-010"),
        strata_protocol_hash: ManagedBuffer::from(b"sha256:strata-010"),
        methodology_version: ManagedBuffer::from(b"1.0.0"),
    }
}

#[test]
fn carbon_credit_issue_credits_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    // 32-byte committed bundle hash
    let hash_32: [u8; 32] = [0xABu8; 32];

    // Register the committed bundle hash first
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"lot-010"),
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64, // 5%
                make_bundle_ref(),
                ManagedBuffer::from(&hash_32[..]),
                OWNER.to_managed_address(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            // net_issuable = 100_000 - (100_000 * 500 / 10_000) = 100_000 - 5_000 = 95_000
            let pk = mrv_common::period_key(1u64);
            let key = (
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-010"),
                pk,
            );
            let issuance = sc.issuances().get(&key).unwrap();
            assert_eq!(issuance, BigUint::from(95_000u64));
            assert_eq!(
                sc.issuance_lots_by_issue_key().get(&key).unwrap(),
                ManagedBuffer::from(b"lot-010")
            );
            assert_eq!(
                sc.issued_issuance_lot_projects()
                    .get(&ManagedBuffer::from(b"lot-010"))
                    .unwrap(),
                ManagedBuffer::from(b"project-010")
            );
            assert_eq!(
                sc.issued_issuance_lot_amounts()
                    .get(&ManagedBuffer::from(b"lot-010"))
                    .unwrap(),
                BigUint::from(95_000u64)
            );
            assert_eq!(
                sc.get_issued_issuance_lot_recipient(ManagedBuffer::from(b"lot-010"))
                    .into_option()
                    .unwrap(),
                OWNER.to_managed_address()
            );
            assert_eq!(
                sc.get_issuance_ime_record_version(
                    ManagedBuffer::from(b"project-010"),
                    ManagedBuffer::from(b"pai-010"),
                    1u64,
                ),
                1u64
            );
            assert_eq!(sc.total_dvcu_minted().get(), BigUint::from(95_000u64));
            assert_eq!(sc.total_dvcu_burned().get(), BigUint::zero());
            assert!(!sc.pending_buffer_deposits().contains_key(&(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-010"),
                mrv_common::period_key(1u64),
            )));
        });

    world
        .query()
        .to(BUFFER_POOL_SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            let record = sc
                .get_buffer_record(ManagedBuffer::from(b"project-010"))
                .into_option()
                .unwrap();
            assert_eq!(record.total_deposited, BigUint::from(5_000u64));
            assert_eq!(sc.get_total_pool_balance(), BigUint::from(5_000u64));
        });

    world
        .check_account(OWNER)
        .esdt_balance(DVCU_TOKEN, BigUint::from(95_000u64));
    world
        .check_account(BUFFER_POOL_SC_ADDRESS)
        .esdt_balance(BUFFER_TOKEN, BigUint::from(5_000u64));
}

#[test]
fn carbon_credit_rejects_duplicate_lot_tokenization_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    let hash_one: [u8; 32] = [0xB1u8; 32];
    let hash_two: [u8; 32] = [0xB2u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-lot-dup-1"),
                1u64,
                ManagedBuffer::from(&hash_one[..]),
            );
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"lot-duplicate"),
                ManagedBuffer::from(b"pai-lot-dup-1"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(10_000u64),
                500u64,
                make_bundle_ref(),
                ManagedBuffer::from(&hash_one[..]),
                OWNER.to_managed_address(),
            );
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-lot-dup-2"),
                2u64,
                ManagedBuffer::from(&hash_two[..]),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "issuance lot already tokenized"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"lot-duplicate"),
                ManagedBuffer::from(b"pai-lot-dup-2"),
                2u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(20_000u64),
                500u64,
                make_bundle_ref(),
                ManagedBuffer::from(&hash_two[..]),
                OWNER.to_managed_address(),
            );
        });
}

#[test]
fn carbon_credit_initiate_retirement_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issued_issuance_lot_projects().insert(
                ManagedBuffer::from(b"lot-ret-001"),
                ManagedBuffer::from(b"project-010"),
            );
            sc.issued_issuance_lot_amounts().insert(
                ManagedBuffer::from(b"lot-ret-001"),
                BigUint::from(10_000u64),
            );
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-001"),
                ManagedBuffer::from(b"lot-ret-001"),
                ManagedBuffer::from(b"project-010"),
                BigUint::from(10_000u64),
                OWNER.to_managed_address(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let ret = sc
                .get_retirement(ManagedBuffer::from(b"ret-001"))
                .into_option()
                .unwrap();
            assert_eq!(ret.status.to_boxed_bytes().as_slice(), b"initiated");
            assert_eq!(ret.lot_id.to_boxed_bytes().as_slice(), b"lot-ret-001");
            assert_eq!(ret.amount_scaled, BigUint::from(10_000u64));
            assert_eq!(ret.beneficiary, OWNER.to_managed_address());
        });
}

#[test]
fn carbon_credit_rejects_retirement_above_issued_lot_amount_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "retirement exceeds issued lot amount"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issued_issuance_lot_projects().insert(
                ManagedBuffer::from(b"lot-small"),
                ManagedBuffer::from(b"project-010"),
            );
            sc.issued_issuance_lot_amounts()
                .insert(ManagedBuffer::from(b"lot-small"), BigUint::from(1_000u64));
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-too-large"),
                ManagedBuffer::from(b"lot-small"),
                ManagedBuffer::from(b"project-010"),
                BigUint::from(1_001u64),
                OWNER.to_managed_address(),
            );
        });
}

#[test]
fn carbon_credit_confirm_retirement_burn_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    let hash_32: [u8; 32] = [0xA1u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-ret-002"),
                1u64,
                ManagedBuffer::from(&hash_32[..]),
            );
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"lot-ret-002"),
                ManagedBuffer::from(b"pai-ret-002"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(10_000u64),
                500u64,
                make_bundle_ref(),
                ManagedBuffer::from(&hash_32[..]),
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-002"),
                ManagedBuffer::from(b"lot-ret-002"),
                ManagedBuffer::from(b"project-010"),
                BigUint::from(5_000u64),
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DVCU_TOKEN, 0, 5_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.confirm_retirement_burn(
                ManagedBuffer::from(b"ret-002"),
                ManagedBuffer::from(b"burn-tx-hash-002"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let ret = sc
                .get_retirement(ManagedBuffer::from(b"ret-002"))
                .into_option()
                .unwrap();
            assert_eq!(ret.status.to_boxed_bytes().as_slice(), b"burned");
            assert_eq!(
                ret.burn_tx_hash.to_boxed_bytes().as_slice(),
                b"burn-tx-hash-002"
            );
            assert_eq!(sc.total_dvcu_minted().get(), BigUint::from(9_500u64));
            assert_eq!(sc.total_dvcu_burned().get(), BigUint::from(5_000u64));
            assert_eq!(
                sc.retired_issuance_lot_amount(&ManagedBuffer::from(b"lot-ret-002"))
                    .get(),
                BigUint::from(5_000u64)
            );
        });

    world
        .check_account(OWNER)
        .esdt_balance(DVCU_TOKEN, BigUint::from(4_500u64));
}

#[test]
fn carbon_credit_revert_retirement_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issued_issuance_lot_projects().insert(
                ManagedBuffer::from(b"lot-ret-003"),
                ManagedBuffer::from(b"project-010"),
            );
            sc.issued_issuance_lot_amounts()
                .insert(ManagedBuffer::from(b"lot-ret-003"), BigUint::from(3_000u64));
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-003"),
                ManagedBuffer::from(b"lot-ret-003"),
                ManagedBuffer::from(b"project-010"),
                BigUint::from(3_000u64),
                OWNER.to_managed_address(),
            );
            sc.revert_retirement(ManagedBuffer::from(b"ret-003"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let ret = sc
                .get_retirement(ManagedBuffer::from(b"ret-003"))
                .into_option()
                .unwrap();
            assert_eq!(ret.status.to_boxed_bytes().as_slice(), b"reverted");
        });
}

#[test]
fn carbon_credit_records_issuance_lot_reversal_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    let hash_32: [u8; 32] = [0xA2u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-reversal-001"),
                1u64,
                ManagedBuffer::from(&hash_32[..]),
            );
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"lot-reversal-001"),
                ManagedBuffer::from(b"pai-reversal-001"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(10_000u64),
                500u64,
                make_bundle_ref(),
                ManagedBuffer::from(&hash_32[..]),
                GOVERNANCE.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DVCU_TOKEN, 0, 3_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.record_issuance_lot_reversal(
                ManagedBuffer::from(b"lot-reversal-001"),
                BigUint::from(3_000u64),
                ManagedBuffer::from(b"lot-replacement-001"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let amount = sc
                .get_recorded_issuance_lot_reversal(ManagedBuffer::from(b"lot-reversal-001"))
                .into_option()
                .unwrap();
            assert_eq!(amount, BigUint::from(3_000u64));
            assert_eq!(sc.total_dvcu_minted().get(), BigUint::from(9_500u64));
            assert_eq!(sc.total_dvcu_burned().get(), BigUint::from(3_000u64));
        });

    world
        .check_account(GOVERNANCE)
        .esdt_balance(DVCU_TOKEN, BigUint::from(6_500u64));
}

#[test]
fn carbon_credit_reversal_requires_tokenized_unretired_supply_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    let hash_32: [u8; 32] = [0xA3u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-reversal-limit-001"),
                1u64,
                ManagedBuffer::from(&hash_32[..]),
            );
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"lot-reversal-limit-001"),
                ManagedBuffer::from(b"pai-reversal-limit-001"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(10_000u64),
                500u64,
                make_bundle_ref(),
                ManagedBuffer::from(&hash_32[..]),
                GOVERNANCE.to_managed_address(),
            );
            sc.retired_issuance_lot_amount(&ManagedBuffer::from(b"lot-reversal-limit-001"))
                .set(BigUint::from(5_000u64));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DVCU_TOKEN, 0, 4_501u64).unwrap())
        .returns(ExpectError(
            4u64,
            "reversal exceeds unretired issued lot amount",
        ))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.record_issuance_lot_reversal(
                ManagedBuffer::from(b"lot-reversal-limit-001"),
                BigUint::from(4_501u64),
                ManagedBuffer::new(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DVCU_TOKEN, 0, 1u64).unwrap())
        .returns(ExpectError(4u64, "issuance lot not tokenized"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.record_issuance_lot_reversal(
                ManagedBuffer::from(b"lot-never-tokenized"),
                BigUint::from(1u64),
                ManagedBuffer::new(),
            );
        });
}

#[test]
fn carbon_credit_issue_credits_with_revoked_ime_fails_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    let hash_32: [u8; 32] = [0xBBu8; 32];

    // Register bundle hash prerequisite
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    // Revoke IME
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.revoke_ime_record(ManagedBuffer::from(b"project-010"));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "IME_REVOKED"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"lot-revoked-ime"),
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64,
                make_bundle_ref(),
                ManagedBuffer::from(&hash_32[..]),
                OWNER.to_managed_address(),
            );
        });
}

#[test]
fn carbon_credit_issue_credits_with_mismatched_image_digest_fails_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    let hash_32: [u8; 32] = [0xCCu8; 32];

    // Register bundle hash prerequisite
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "IME_IMAGE_MISMATCH"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let bad_bundle = mrv_carbon_credit::ExecutionBundleRef {
                science_service_image_digest: ManagedBuffer::from(b"sha256:WRONG-IMAGE"),
                parameter_pack_hash: ManagedBuffer::from(b"sha256:param-010"),
                calibration_dataset_hash: ManagedBuffer::from(b"sha256:cal-010"),
                strata_protocol_hash: ManagedBuffer::from(b"sha256:strata-010"),
                methodology_version: ManagedBuffer::from(b"1.0.0"),
            };
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"lot-image-mismatch"),
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64,
                bad_bundle,
                ManagedBuffer::from(&hash_32[..]),
                OWNER.to_managed_address(),
            );
        });
}

const GSOC_VERIFIER: TestAddress = TestAddress::new("gsoc-verifier");

#[test]
fn carbon_credit_gsoc_issuance_flow_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);
    configure_gsoc_governance(&mut world);

    let gsoc_hash: [u8; 32] = [0xDDu8; 32];

    // Register GSOC bundle
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
        });

    // Issue GSOC credits
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-gsoc"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"dna-ref-001"),
                ManagedBuffer::from(b"ITMO-001"),
                BigUint::from(50_000u64),
                500u64, // 5%
                OWNER.to_managed_address(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let pk = mrv_common::period_key(1u64);
            let key = (
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-gsoc"),
                pk,
            );
            let issuance = sc.gsoc_issuances().get(&key).unwrap();
            // net = 50_000 - (50_000 * 500 / 10_000) = 50_000 - 2_500 = 47_500
            assert_eq!(issuance, BigUint::from(47_500u64));
            assert_eq!(
                sc.get_gsoc_serial_recipient(ManagedBuffer::from(b"ITMO-001"))
                    .into_option()
                    .unwrap(),
                OWNER.to_managed_address()
            );
            assert_eq!(sc.total_dgsc_minted().get(), BigUint::from(47_500u64));
            assert_eq!(sc.total_dgsc_burned().get(), BigUint::zero());
        });

    world
        .check_account(OWNER)
        .esdt_balance(DGSC_TOKEN, BigUint::from(47_500u64));
}

#[test]
fn carbon_credit_gsoc_issuance_uses_governance_verifier_registry_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);
    world.account(GSOC_VERIFIER).nonce(1).balance(1_000_000u64);

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
            sc.propose_gsoc_verifier(
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"credentials-cid-001"),
                ManagedBuffer::from(b"SG"),
            );
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_gsoc_verifier_proposal(1u64);
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_gsoc_verifier_proposal(1u64);
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_gsoc_verifier_proposal(1u64);
        });

    let gsoc_hash: [u8; 32] = [0xD1u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC_ADDRESS.to_managed_address());
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-gov"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-gsoc-gov"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"dna-ref-gov-001"),
                ManagedBuffer::from(b"ITMO-GOV-001"),
                BigUint::from(50_000u64),
                500u64,
                OWNER.to_managed_address(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let pk = mrv_common::period_key(1u64);
            let key = (
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-gsoc-gov"),
                pk,
            );
            let issuance = sc.gsoc_issuances().get(&key).unwrap();
            assert_eq!(issuance, BigUint::from(47_500u64));
        });
}

#[test]
fn carbon_credit_gsoc_retirement_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);
    configure_gsoc_governance(&mut world);

    let gsoc_hash: [u8; 32] = [0xEEu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-ret"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-gsoc-ret"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"dna-ref-002"),
                ManagedBuffer::from(b"ITMO-RET"),
                BigUint::from(100_000u64),
                500u64,
                OWNER.to_managed_address(),
            );
        });

    // Retire full net amount (95_000)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 95_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"ITMO-RET"),
                BigUint::from(95_000u64),
                ManagedBuffer::from(b"Beneficiary Corp"),
                OWNER.to_managed_address(),
            );
        });

    // Verify serial is fully retired
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            assert!(
                sc.gsoc_retired_serials()
                    .contains(&ManagedBuffer::from(b"ITMO-RET"))
            );
            assert_eq!(sc.total_dgsc_minted().get(), BigUint::from(95_000u64));
            assert_eq!(sc.total_dgsc_burned().get(), BigUint::from(95_000u64));
            assert_eq!(
                sc.project_gsoc_total_issued(&ManagedBuffer::from(b"project-010"))
                    .get(),
                BigUint::from(95_000u64)
            );
            assert_eq!(
                sc.project_gsoc_total_retired(&ManagedBuffer::from(b"project-010"))
                    .get(),
                BigUint::from(95_000u64)
            );
            assert_eq!(
                sc.project_gsoc_serial_count(&ManagedBuffer::from(b"project-010"))
                    .get(),
                1u64
            );
        });
}

#[test]
fn carbon_credit_governance_pause_blocks_mutations_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

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
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-carbon-001"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-carbon-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-carbon-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-carbon-001"));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC_ADDRESS.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-paused-001"),
                1u64,
                ManagedBuffer::from(&[0x11u8; 32][..]),
            );
        });
}

#[test]
fn carbon_credit_local_gsoc_verifier_registry_disabled_in_governance_mode_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

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
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC_ADDRESS.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "GSOC_VERIFIER_REGISTRY_CANONICALIZED_TO_GOVERNANCE: local GSOC verifier mutations are disabled while governanceReadAddress is configured",
        ))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.add_approved_gsoc_verifier(GSOC_VERIFIER.to_managed_address());
        });
}

#[test]
fn carbon_credit_gsoc_issuance_requires_governance_verifier_source_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);
    world.account(GSOC_VERIFIER).nonce(1).balance(1_000_000u64);

    let gsoc_hash: [u8; 32] = [0xD2u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-local"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.add_approved_gsoc_verifier(GSOC_VERIFIER.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "GSOC_VERIFIER_GOVERNANCE_READ_REQUIRED"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-gsoc-local"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"dna-ref-local"),
                ManagedBuffer::from(b"ITMO-LOCAL-001"),
                BigUint::from(50_000u64),
                500u64,
                OWNER.to_managed_address(),
            );
        });
}

#[test]
fn carbon_credit_expired_ime_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(BUFFER_POOL).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_POOL.to_managed_address(),
            );
        });

    // Register IME with valid_until = 5000
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            domain_codes.push(ManagedBuffer::from(b"SG"));
            sc.register_ime_record(
                ManagedBuffer::from(b"project-exp"),
                ManagedBuffer::from(b"sha256:image-exp"),
                ManagedBuffer::from(b"sha256:param-exp"),
                ManagedBuffer::from(b"sha256:cal-exp"),
                ManagedBuffer::from(b"sha256:strata-exp"),
                ManagedBuffer::from(b"1.0.0"),
                5000u64,
                domain_codes,
            );
        });

    let hash_32: [u8; 32] = [0xFFu8; 32];
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-exp"),
                1u64,
                ManagedBuffer::from(&hash_32[..]),
            );
        });

    // Advance past IME expiry
    world.current_block().block_timestamp_seconds(5001u64);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "IME_EXPIRED"))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let bundle_ref = mrv_carbon_credit::ExecutionBundleRef {
                science_service_image_digest: ManagedBuffer::from(b"sha256:image-exp"),
                parameter_pack_hash: ManagedBuffer::from(b"sha256:param-exp"),
                calibration_dataset_hash: ManagedBuffer::from(b"sha256:cal-exp"),
                strata_protocol_hash: ManagedBuffer::from(b"sha256:strata-exp"),
                methodology_version: ManagedBuffer::from(b"1.0.0"),
            };
            sc.issue_credits(
                ManagedBuffer::from(b"project-exp"),
                ManagedBuffer::from(b"lot-exp"),
                ManagedBuffer::from(b"pai-exp"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64,
                bundle_ref,
                ManagedBuffer::from(&hash_32[..]),
                OWNER.to_managed_address(),
            );
        });
}

// ── M-03 (AUD-008) append-only GSOC retirement history tests ────────

/// Shared fixture: deploy, register IME, register GSOC bundle, approve
/// the GSOC verifier, and issue 100_000 gross → 95_000 net on a
/// single ITMO serial. Leaves the world ready for partial-retirement
/// assertions.
fn deploy_and_issue_gsoc_95k(world: &mut ScenarioWorld) {
    deploy_and_register_ime(world);
    configure_gsoc_governance(world);
    let gsoc_hash: [u8; 32] = [0xEEu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-m03"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"project-m03"),
                ManagedBuffer::from(b"pai-gsoc-m03"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"dna-ref-m03"),
                ManagedBuffer::from(b"ITMO-M03"),
                BigUint::from(100_000u64),
                500u64,
                OWNER.to_managed_address(),
            );
        });
}

#[test]
fn carbon_credit_m03_partial_retirement_preserves_initial_amount() {
    // Verifies the core M-03 invariant: `gsoc_serial_records` is NOT
    // mutated on partial retirement. The immutable initial amount
    // (95_000) must remain readable via `gsoc_serial_records` even
    // AFTER a partial retirement reduces the running `remaining` slot.
    let mut world = world();
    deploy_and_issue_gsoc_95k(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 30_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"ITMO-M03"),
                BigUint::from(30_000u64),
                ManagedBuffer::from(b"Beneficiary One"),
                OWNER.to_managed_address(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            // Immutable initial amount is preserved on records.
            let record = sc
                .gsoc_serial_records()
                .get(&ManagedBuffer::from(b"ITMO-M03"))
                .expect("serial record missing");
            assert_eq!(
                record.2,
                BigUint::from(95_000u64),
                "M-03: gsoc_serial_records must retain the immutable initial amount",
            );

            // Running remaining is tracked separately.
            assert_eq!(
                sc.gsoc_serial_remaining(&ManagedBuffer::from(b"ITMO-M03"))
                    .get(),
                BigUint::from(65_000u64),
                "running remaining must reflect initial minus retired",
            );

            // Seq count advanced exactly once.
            assert_eq!(
                sc.gsoc_retirement_seq_count(&ManagedBuffer::from(b"ITMO-M03"))
                    .get(),
                1u64,
            );

            // First event record (seq=0) has correct fields.
            let event = sc
                .gsoc_retirement_events(&ManagedBuffer::from(b"ITMO-M03"), 0)
                .get();
            assert_eq!(event.seq, 0);
            assert_eq!(event.amount_scaled, BigUint::from(30_000u64));
            assert_eq!(event.remaining_after, BigUint::from(65_000u64));

            // Serial NOT in retired set — balance remains.
            assert!(
                !sc.gsoc_retired_serials()
                    .contains(&ManagedBuffer::from(b"ITMO-M03")),
                "partially-retired serial must not be marked fully retired",
            );
        });
}

#[test]
fn carbon_credit_m03_multiple_retirements_append_to_log_in_order() {
    // Retire three times on the same serial; verify every event is
    // captured at its own seq index, balances agree at each step,
    // and the serial is only flagged fully-retired once balance hits 0.
    let mut world = world();
    deploy_and_issue_gsoc_95k(&mut world);

    let retirements: [(u64, &'static [u8]); 3] = [
        (30_000, b"Beneficiary One"),
        (30_000, b"Beneficiary Two"),
        (35_000, b"Beneficiary Three"),
    ];

    for (amount, name) in &retirements {
        world
            .tx()
            .from(OWNER)
            .to(SC_ADDRESS)
            .payment(Payment::try_new(DGSC_TOKEN, 0, *amount).unwrap())
            .whitebox(mrv_carbon_credit::contract_obj, |sc| {
                sc.burn_and_retire_gsoc(
                    ManagedBuffer::from(b"ITMO-M03"),
                    BigUint::from(*amount),
                    ManagedBuffer::from(*name),
                    OWNER.to_managed_address(),
                );
            });
    }

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let seq_count = sc
                .gsoc_retirement_seq_count(&ManagedBuffer::from(b"ITMO-M03"))
                .get();
            assert_eq!(seq_count, 3u64, "three retirements → seq count 3");

            // Validate each event's running balance.
            let expected_remaining = [65_000u64, 35_000u64, 0u64];
            for (i, (amount, name)) in retirements.iter().enumerate() {
                let event = sc
                    .gsoc_retirement_events(&ManagedBuffer::from(b"ITMO-M03"), i as u64)
                    .get();
                assert_eq!(event.seq, i as u64);
                assert_eq!(event.amount_scaled, BigUint::from(*amount));
                assert_eq!(event.remaining_after, BigUint::from(expected_remaining[i]));
                assert_eq!(event.beneficiary_name.to_boxed_bytes().as_slice(), *name,);
            }

            // Running remaining is zero; serial is fully retired.
            assert_eq!(
                sc.gsoc_serial_remaining(&ManagedBuffer::from(b"ITMO-M03"))
                    .get(),
                BigUint::from(0u64),
            );
            assert!(
                sc.gsoc_retired_serials()
                    .contains(&ManagedBuffer::from(b"ITMO-M03")),
                "zero remaining → serial flagged fully retired",
            );

            // Initial amount on records is STILL the original 95_000.
            let record = sc
                .gsoc_serial_records()
                .get(&ManagedBuffer::from(b"ITMO-M03"))
                .unwrap();
            assert_eq!(record.2, BigUint::from(95_000u64));
        });
}

#[test]
fn carbon_credit_m03_rejects_retire_exceeding_running_remaining() {
    // After a partial retirement reduces `gsoc_serial_remaining`, a
    // follow-up retirement whose amount exceeds the running balance
    // must be rejected — NOT the stale initial amount. This is the
    // accounting invariant the audit flagged as unverifiable under
    // the old in-place mutation scheme.
    let mut world = world();
    deploy_and_issue_gsoc_95k(&mut world);

    // First retirement brings remaining down to 35_000.
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 60_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"ITMO-M03"),
                BigUint::from(60_000u64),
                ManagedBuffer::from(b"First"),
                OWNER.to_managed_address(),
            );
        });

    // Second retirement asks for 40_000 but only 35_000 is left.
    // Must reject against the RUNNING remaining, not the initial.
    world.set_esdt_balance(OWNER, DGSC_TOKEN.as_bytes(), 40_000u64);
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 40_000u64).unwrap())
        .returns(ExpectError(
            4u64,
            "GSOC_AMOUNT_EXCEEDS_REMAINING: cannot retire more than remaining quantity",
        ))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"ITMO-M03"),
                BigUint::from(40_000u64),
                ManagedBuffer::from(b"Second"),
                OWNER.to_managed_address(),
            );
        });

    // Remaining slot unchanged by the failed call.
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            assert_eq!(
                sc.gsoc_serial_remaining(&ManagedBuffer::from(b"ITMO-M03"))
                    .get(),
                BigUint::from(35_000u64),
            );
            assert_eq!(
                sc.gsoc_retirement_seq_count(&ManagedBuffer::from(b"ITMO-M03"))
                    .get(),
                1u64,
                "failed retirement must not advance seq count",
            );
        });
}

#[test]
fn carbon_credit_m03_rejects_retire_on_fully_retired_serial() {
    // Once the serial is in `gsoc_retired_serials`, further retirement
    // calls must fail at the early-guard level with the existing
    // "no remaining balance" error — NOT fall through to a balance
    // check that would panic on underflow.
    let mut world = world();
    deploy_and_issue_gsoc_95k(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 95_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"ITMO-M03"),
                BigUint::from(95_000u64),
                ManagedBuffer::from(b"Full"),
                OWNER.to_managed_address(),
            );
        });

    world.set_esdt_balance(OWNER, DGSC_TOKEN.as_bytes(), 1u64);
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 1u64).unwrap())
        .returns(ExpectError(
            4u64,
            "GSOC_SERIAL_FULLY_RETIRED: no remaining balance on this serial",
        ))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"ITMO-M03"),
                BigUint::from(1u64),
                ManagedBuffer::from(b"Overflow"),
                OWNER.to_managed_address(),
            );
        });
}

#[test]
fn carbon_credit_gsoc_serial_inventory_hash_matches_worker_preimage_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);
    configure_gsoc_governance(&mut world);

    let gsoc_hash: [u8; 32] = [0xC1u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-hash-002"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-hash-001"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"project-hash"),
                ManagedBuffer::from(b"pai-hash-002"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"dna-ref-hash"),
                ManagedBuffer::from(b"SER-002"),
                BigUint::from(10_000u64),
                100u64,
                OWNER.to_managed_address(),
            );
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"project-hash"),
                ManagedBuffer::from(b"pai-hash-001"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"dna-ref-hash"),
                ManagedBuffer::from(b"SER-001"),
                BigUint::from(5_000u64),
                100u64,
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 4_950u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"SER-001"),
                BigUint::from(4_950u64),
                ManagedBuffer::from(b"Beneficiary Hash"),
                OWNER.to_managed_address(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let actual =
                sc.get_canonical_gsoc_serial_inventory_hash(ManagedBuffer::from(b"project-hash"));
            let expected_preimage = ManagedBuffer::from(
                b"[{\"serial\":\"SER-001\",\"quantityTco2e\":4950,\"status\":\"retired\"},{\"serial\":\"SER-002\",\"quantityTco2e\":9900,\"status\":\"registered\"}]",
            );
            let expected = sc
                .crypto()
                .sha256(&expected_preimage)
                .as_managed_buffer()
                .clone();

            assert_eq!(
                actual, expected,
                "GSOC canonical inventory hash must match the worker JSON preimage",
            );
            assert!(
                sc.verify_canonical_gsoc_serial_inventory_hash(
                    ManagedBuffer::from(b"project-hash"),
                    expected,
                ),
                "verification view must accept the canonical GSOC inventory hash",
            );
        });
}

#[test]
fn carbon_credit_gsoc_serial_inventory_hash_accepts_biguint_quantities_above_u64_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let project_id = ManagedBuffer::from(b"project-huge-hash");
            let serial = ManagedBuffer::from(b"SER-HUGE");
            let base = BigUint::from(u32::MAX) + 1u32;
            let huge_amount = &base * &base;

            sc.project_gsoc_serial_count(&project_id).set(1u64);
            sc.project_gsoc_serials(&project_id).insert(serial.clone());
            sc.gsoc_serial_records()
                .insert(serial, (project_id.clone(), 1u64, huge_amount));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let actual = sc.get_canonical_gsoc_serial_inventory_hash(ManagedBuffer::from(
                b"project-huge-hash",
            ));
            let expected_preimage = ManagedBuffer::from(
                b"[{\"serial\":\"SER-HUGE\",\"quantityTco2e\":18446744073709551616,\"status\":\"registered\"}]",
            );
            let expected = sc
                .crypto()
                .sha256(&expected_preimage)
                .as_managed_buffer()
                .clone();

            assert_eq!(
                actual, expected,
                "GSOC canonical inventory hash must serialize BigUint quantities without u64 truncation or panic",
            );
        });
}

#[test]
fn carbon_credit_rejects_gsoc_issuance_beyond_bounded_project_serial_inventory_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);
    configure_gsoc_governance(&mut world);
    let gsoc_hash: [u8; 32] = [0xC2u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-limit"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.project_gsoc_serial_count(&ManagedBuffer::from(b"project-limit"))
                .set(1024u64);
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "GSOC_PROJECT_SERIAL_LIMIT_EXCEEDED: project serial inventory exceeds bounded canonical hash limit",
        ))
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"project-limit"),
                ManagedBuffer::from(b"pai-gsoc-limit"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                GSOC_VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"dna-ref-limit"),
                ManagedBuffer::from(b"SER-LIMIT"),
                BigUint::from(10_000u64),
                100u64,
                OWNER.to_managed_address(),
            );
        });
}
