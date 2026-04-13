use mrv_carbon_credit::CarbonCreditModule;
use mrv_common::MrvGovernanceModule;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const BUFFER_POOL: TestAddress = TestAddress::new("buffer-pool");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-carbon-credit");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-carbon-credit.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/carbon-credit");
    world.register_contract(CODE_PATH, mrv_carbon_credit::ContractBuilder);
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
            assert_eq!(sc.buffer_pool_addr().get(), BUFFER_POOL.to_managed_address());
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
            assert_eq!(
                ime.project_id.to_boxed_bytes().as_slice(),
                b"project-001"
            );
            assert_eq!(
                ime.science_service_image_digest.to_boxed_bytes().as_slice(),
                b"sha256:image-digest-001"
            );
            assert!(!ime.revoked);
            assert_eq!(ime.domain_codes.len(), 2);
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

fn make_bundle_ref<M: multiversx_sc::api::ManagedTypeApi>() -> mrv_carbon_credit::ExecutionBundleRef<M> {
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
            sc.register_committed_bundle(ManagedBuffer::from(b"pai-010"), 1u64, ManagedBuffer::from(&hash_32[..]));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_credits(
                ManagedBuffer::from(b"project-010"),
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64, // 5%
                make_bundle_ref(),
                ManagedBuffer::from(&hash_32[..]),
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
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-001"),
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
            assert_eq!(ret.amount_scaled, BigUint::from(10_000u64));
            assert_eq!(ret.beneficiary, OWNER.to_managed_address());
        });
}

#[test]
fn carbon_credit_confirm_retirement_burn_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-002"),
                ManagedBuffer::from(b"project-010"),
                BigUint::from(5_000u64),
                OWNER.to_managed_address(),
            );
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
        });
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
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-003"),
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
            sc.register_committed_bundle(ManagedBuffer::from(b"pai-010"), 1u64, ManagedBuffer::from(&hash_32[..]));
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
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64,
                make_bundle_ref(),
                ManagedBuffer::from(&hash_32[..]),
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
            sc.register_committed_bundle(ManagedBuffer::from(b"pai-010"), 1u64, ManagedBuffer::from(&hash_32[..]));
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
                ManagedBuffer::from(b"pai-010"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64,
                bad_bundle,
                ManagedBuffer::from(&hash_32[..]),
            );
        });
}

const GSOC_VERIFIER: TestAddress = TestAddress::new("gsoc-verifier");

#[test]
fn carbon_credit_gsoc_issuance_flow_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    world.account(GSOC_VERIFIER).nonce(1).balance(1_000_000u64);

    let gsoc_hash: [u8; 32] = [0xDDu8; 32];

    // Register GSOC bundle
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
        sc.register_gsoc_bundle(ManagedBuffer::from(b"pai-gsoc"), 1u64, ManagedBuffer::from(&gsoc_hash[..]));
    });

    // Add approved GSOC verifier (owner-only)
    world.tx().from(OWNER).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
        sc.add_approved_gsoc_verifier(GSOC_VERIFIER.to_managed_address());
    });

    // Issue GSOC credits
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
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
        );
    });

    world.query().to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
        let pk = mrv_common::period_key(1u64);
        let key = (
            ManagedBuffer::from(b"project-010"),
            ManagedBuffer::from(b"pai-gsoc"),
            pk,
        );
        let issuance = sc.gsoc_issuances().get(&key).unwrap();
        // net = 50_000 - (50_000 * 500 / 10_000) = 50_000 - 2_500 = 47_500
        assert_eq!(issuance, BigUint::from(47_500u64));
    });
}

#[test]
fn carbon_credit_gsoc_retirement_rs() {
    let mut world = world();
    deploy_and_register_ime(&mut world);

    world.account(GSOC_VERIFIER).nonce(1).balance(1_000_000u64);

    let gsoc_hash: [u8; 32] = [0xEEu8; 32];

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
        sc.register_gsoc_bundle(ManagedBuffer::from(b"pai-gsoc-ret"), 1u64, ManagedBuffer::from(&gsoc_hash[..]));
    });

    world.tx().from(OWNER).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
        sc.add_approved_gsoc_verifier(GSOC_VERIFIER.to_managed_address());
    });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
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
        );
    });

    // Retire full net amount (95_000)
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
        sc.burn_and_retire_gsoc(
            ManagedBuffer::from(b"ITMO-RET"),
            BigUint::from(95_000u64),
            ManagedBuffer::from(b"Beneficiary Corp"),
            OWNER.to_managed_address(),
        );
    });

    // Verify serial is fully retired
    world.query().to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
        assert!(sc.gsoc_retired_serials().contains(&ManagedBuffer::from(b"ITMO-RET")));
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
            sc.init(GOVERNANCE.to_managed_address(), BUFFER_POOL.to_managed_address());
        });

    // Register IME with valid_until = 5000
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
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
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(mrv_carbon_credit::contract_obj, |sc| {
        sc.register_committed_bundle(ManagedBuffer::from(b"pai-exp"), 1u64, ManagedBuffer::from(&hash_32[..]));
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
                ManagedBuffer::from(b"pai-exp"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64,
                bundle_ref,
                ManagedBuffer::from(&hash_32[..]),
            );
        });
}
