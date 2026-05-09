use mrv_buffer_pool::BufferPool;
use mrv_carbon_credit::CarbonCreditModule;
use mrv_governance::MrvGovernance;
use mrv_reserve_proof_registry::ReserveProofRegistry;
use multiversx_sc::types::{ManagedBuffer, TokenIdentifier};
use multiversx_sc_scenario::imports::*;
use std::cell::RefCell;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");

const CARBON_SC: TestSCAddress = TestSCAddress::new("mrv-carbon-credit");
const BUFFER_SC: TestSCAddress = TestSCAddress::new("mrv-buffer-pool");
const RESERVE_SC: TestSCAddress = TestSCAddress::new("mrv-reserve-proof-registry");
const GOVERNANCE_SC: TestSCAddress = TestSCAddress::new("mrv-governance");

const CARBON_CODE: MxscPath =
    MxscPath::new("mxsc:../../carbon-credit/output/mrv-carbon-credit.mxsc.json");
const BUFFER_CODE: MxscPath =
    MxscPath::new("mxsc:../../buffer-pool/output/mrv-buffer-pool.mxsc.json");
const RESERVE_CODE: MxscPath = MxscPath::new("mxsc:output/mrv-reserve-proof-registry.mxsc.json");
const GOVERNANCE_CODE: MxscPath =
    MxscPath::new("mxsc:../../governance/output/mrv-governance.mxsc.json");

const DVCU_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DVCU-123456");
const DGSC_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DGSC-123456");
const BUFFER_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DVCUBUF-123456");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/reserve-proof-registry");
    world.register_contract(CARBON_CODE, mrv_carbon_credit::ContractBuilder);
    world.register_contract(BUFFER_CODE, mrv_buffer_pool::ContractBuilder);
    world.register_contract(RESERVE_CODE, mrv_reserve_proof_registry::ContractBuilder);
    world.register_contract(GOVERNANCE_CODE, mrv_governance::ContractBuilder);
    world
}

fn deploy_tokenized_runtime(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CARBON_CODE)
        .new_address(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_SC.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(BUFFER_CODE)
        .new_address(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                CARBON_SC.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(RESERVE_CODE)
        .new_address(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.set_esdt_local_roles(
        CARBON_SC.to_address(),
        DVCU_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );
    world.set_esdt_local_roles(
        CARBON_SC.to_address(),
        DGSC_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );
    world.set_esdt_local_roles(
        BUFFER_SC.to_address(),
        BUFFER_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_dvcu_token_id(TokenIdentifier::from(DVCU_TOKEN.as_bytes()));
            sc.set_dgsc_token_id(TokenIdentifier::from(DGSC_TOKEN.as_bytes()));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.set_buffer_token_id(TokenIdentifier::from(BUFFER_TOKEN.as_bytes()));
        });

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.set_carbon_credit_addr(CARBON_SC.to_managed_address());
            sc.set_buffer_pool_addr(BUFFER_SC.to_managed_address());
        },
    );
}

fn configure_carbon_gsoc_governance(world: &mut ScenarioWorld) {
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(GOVERNANCE_CODE)
        .new_address(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_gsoc_verifier(
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"credentials-cid-reserve-proof"),
                ManagedBuffer::from(b"SG"),
            );
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_gsoc_verifier_proposal(1u64);
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_gsoc_verifier_proposal(1u64);
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_gsoc_verifier_proposal(1u64);
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC.to_managed_address());
        });
}

fn seed_vm0042_canonical_state(world: &mut ScenarioWorld) {
    deploy_tokenized_runtime(world);

    let bundle_hash: [u8; 32] = [0x11u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            domain_codes.push(ManagedBuffer::from(b"SG"));
            sc.register_ime_record(
                ManagedBuffer::from(b"project-001"),
                ManagedBuffer::from(b"sha256:image-001"),
                ManagedBuffer::from(b"sha256:param-001"),
                ManagedBuffer::from(b"sha256:cal-001"),
                ManagedBuffer::from(b"sha256:strata-001"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes,
            );
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-001"),
                1u64,
                ManagedBuffer::from(&bundle_hash[..]),
            );
            sc.issue_credits(
                ManagedBuffer::from(b"project-001"),
                ManagedBuffer::from(b"lot-001"),
                ManagedBuffer::from(b"pai-001"),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(100_000u64),
                500u64,
                mrv_carbon_credit::ExecutionBundleRef {
                    science_service_image_digest: ManagedBuffer::from(b"sha256:image-001"),
                    parameter_pack_hash: ManagedBuffer::from(b"sha256:param-001"),
                    calibration_dataset_hash: ManagedBuffer::from(b"sha256:cal-001"),
                    strata_protocol_hash: ManagedBuffer::from(b"sha256:strata-001"),
                    methodology_version: ManagedBuffer::from(b"1.0.0"),
                },
                ManagedBuffer::from(&bundle_hash[..]),
                OWNER.to_managed_address(),
            );
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-001"),
                ManagedBuffer::from(b"lot-001"),
                ManagedBuffer::from(b"project-001"),
                BigUint::from(20_000u64),
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(CARBON_SC)
        .payment(Payment::try_new(DVCU_TOKEN, 0, 20_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.confirm_retirement_burn(
                ManagedBuffer::from(b"ret-001"),
                ManagedBuffer::from(b"burn-tx-001"),
            );
        });

    world
        .tx()
        .from(CARBON_SC)
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.deposit_buffer_credits(
                ManagedBuffer::from(b"project-001"),
                BigUint::from(5_000u64),
                1u64,
            );
        });
}

fn seed_vm0042_supply_only_state(world: &mut ScenarioWorld) {
    deploy_tokenized_runtime(world);

    let bundle_hash: [u8; 32] = [0x44u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            domain_codes.push(ManagedBuffer::from(b"KE"));
            sc.register_ime_record(
                ManagedBuffer::from(b"project-proof-001"),
                ManagedBuffer::from(b"sha256:image-proof-001"),
                ManagedBuffer::from(b"sha256:param-proof-001"),
                ManagedBuffer::from(b"sha256:cal-proof-001"),
                ManagedBuffer::from(b"sha256:strata-proof-001"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes,
            );
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-proof-001"),
                1u64,
                ManagedBuffer::from(&bundle_hash[..]),
            );
            sc.issue_credits(
                ManagedBuffer::from(b"project-proof-001"),
                ManagedBuffer::from(b"lot-proof-001"),
                ManagedBuffer::from(b"pai-proof-001"),
                1u64,
                ManagedBuffer::from(b"KE"),
                BigUint::from(1_000u64),
                100u64,
                mrv_carbon_credit::ExecutionBundleRef {
                    science_service_image_digest: ManagedBuffer::from(b"sha256:image-proof-001"),
                    parameter_pack_hash: ManagedBuffer::from(b"sha256:param-proof-001"),
                    calibration_dataset_hash: ManagedBuffer::from(b"sha256:cal-proof-001"),
                    strata_protocol_hash: ManagedBuffer::from(b"sha256:strata-proof-001"),
                    methodology_version: ManagedBuffer::from(b"1.0.0"),
                },
                ManagedBuffer::from(&bundle_hash[..]),
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(CARBON_SC)
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.deposit_buffer_credits(
                ManagedBuffer::from(b"project-proof-001"),
                BigUint::from(10u64),
                1u64,
            );
        });
}

#[test]
fn reserve_proof_init_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(RESERVE_CODE)
        .new_address(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });
}

#[test]
fn reserve_proof_anchor_reconciles_canonical_vm0042_totals_rs() {
    let mut world = world();
    seed_vm0042_canonical_state(&mut world);
    let merkle_root: [u8; 32] = [0xA5u8; 32];

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                BigUint::from(75_000u64),
                BigUint::from(5_000u64),
                BigUint::from(20_000u64),
                ManagedBuffer::from(&merkle_root[..]),
                1_000u64,
            );
        },
    );

    world
        .query()
        .to(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let proof = sc
                .get_latest_reserve_proof(ManagedBuffer::from(DVCU_TOKEN.as_bytes()))
                .into_option()
                .expect("proof should exist");
            assert_eq!(proof.total_supply_scaled, BigUint::from(75_000u64));
            assert_eq!(proof.total_buffer_scaled, BigUint::from(5_000u64));
            assert_eq!(proof.total_retired_scaled, BigUint::from(20_000u64));
            assert_eq!(proof.net_circulating_scaled, BigUint::from(50_000u64));
            assert_eq!(proof.snapshot_block, 1_000u64);
        });
}

#[test]
fn reserve_proof_rejects_noncanonical_vm0042_totals_rs() {
    let mut world = world();
    seed_vm0042_canonical_state(&mut world);
    let merkle_root: [u8; 32] = [0xB6u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .returns(ExpectError(
            4u64,
            "CANONICAL_SUPPLY_MISMATCH: supplied total_supply_scaled does not match lifecycle counters",
        ))
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                BigUint::from(80_000u64),
                BigUint::from(5_000u64),
                BigUint::from(20_000u64),
                ManagedBuffer::from(&merkle_root[..]),
                1_000u64,
            );
        });
}

fn hex_encode(bytes: &[u8]) -> Vec<u8> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize]);
        out.push(HEX[(byte & 0x0f) as usize]);
    }
    out
}

fn compute_vm0042_merkle_material(world: &mut ScenarioWorld) -> ([u8; 32], [u8; 32], [u8; 32]) {
    let root_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);
    let left_leaf_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);
    let right_leaf_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    world
        .query()
        .to(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let mut left_leaf_preimage = ManagedBuffer::new();
            left_leaf_preimage.append_bytes(&[0u8]);
            left_leaf_preimage.append_bytes(b"0:erd1holderalpha:300");
            let left_hash = sc.crypto().sha256(&left_leaf_preimage);

            let mut right_leaf_preimage = ManagedBuffer::new();
            right_leaf_preimage.append_bytes(&[0u8]);
            right_leaf_preimage.append_bytes(b"1:erd1holderbeta:690");
            let right_hash = sc.crypto().sha256(&right_leaf_preimage);

            let left_bytes = left_hash.as_managed_buffer().to_boxed_bytes();
            let right_bytes = right_hash.as_managed_buffer().to_boxed_bytes();

            let mut left_leaf = [0u8; 32];
            let mut right_leaf = [0u8; 32];
            left_leaf.copy_from_slice(left_bytes.as_slice());
            right_leaf.copy_from_slice(right_bytes.as_slice());

            *left_leaf_cell.borrow_mut() = left_leaf;
            *right_leaf_cell.borrow_mut() = right_leaf;

            let left_hex = hex_encode(left_bytes.as_slice());
            let right_hex = hex_encode(right_bytes.as_slice());
            let (a, b) = if left_hex <= right_hex {
                (left_hex, right_hex)
            } else {
                (right_hex, left_hex)
            };

            let mut combined = ManagedBuffer::new();
            combined.append_bytes(&[1u8]);
            combined.append_bytes(&a);
            combined.append_bytes(&b);

            let root_hash = sc.crypto().sha256(&combined);
            let root_bytes = root_hash.as_managed_buffer().to_boxed_bytes();
            let mut root = [0u8; 32];
            root.copy_from_slice(root_bytes.as_slice());
            *root_cell.borrow_mut() = root;
        });

    (
        *root_cell.borrow(),
        *left_leaf_cell.borrow(),
        *right_leaf_cell.borrow(),
    )
}

#[test]
fn reserve_proof_verify_holder_snapshot_accepts_valid_merkle_proof_rs() {
    let mut world = world();
    seed_vm0042_supply_only_state(&mut world);

    let (merkle_root, _left_leaf, right_leaf) = compute_vm0042_merkle_material(&mut world);

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                BigUint::from(990u64),
                BigUint::from(10u64),
                BigUint::zero(),
                ManagedBuffer::from(&merkle_root[..]),
                2_000u64,
            );
        },
    );

    world
        .query()
        .to(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let mut merkle_proof = ManagedVec::new();
            merkle_proof.push(ManagedBuffer::from(&right_leaf[..]));

            assert!(sc.verify_holder_snapshot(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                2_000u64,
                0u64,
                ManagedBuffer::from(b"erd1holderalpha"),
                ManagedBuffer::from(b"300"),
                merkle_proof,
            ));
        });
}

#[test]
fn reserve_proof_verify_holder_snapshot_rejects_invalid_balance_or_path_rs() {
    let mut world = world();
    seed_vm0042_supply_only_state(&mut world);

    let (merkle_root, _left_leaf, right_leaf) = compute_vm0042_merkle_material(&mut world);

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                BigUint::from(990u64),
                BigUint::from(10u64),
                BigUint::zero(),
                ManagedBuffer::from(&merkle_root[..]),
                2_100u64,
            );
        },
    );

    world
        .query()
        .to(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let mut merkle_proof = ManagedVec::new();
            merkle_proof.push(ManagedBuffer::from(&right_leaf[..]));

            assert!(!sc.verify_holder_snapshot(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                2_100u64,
                0u64,
                ManagedBuffer::from(b"erd1holderalpha"),
                ManagedBuffer::from(b"301"),
                merkle_proof,
            ));
        });
}

#[test]
fn gsoc_reserve_proof_anchor_rs() {
    let mut world = world();
    deploy_tokenized_runtime(&mut world);
    configure_carbon_gsoc_governance(&mut world);
    let canonical_hash_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    let gsoc_hash: [u8; 32] = [0xA7u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-proof"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"proj-001"),
                ManagedBuffer::from(b"pai-gsoc-proof"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"dna-gsoc-proof"),
                ManagedBuffer::from(b"ITMO-GSOC-PROOF-001"),
                BigUint::from(10_000u64),
                100u64,
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(CARBON_SC)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 2_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"ITMO-GSOC-PROOF-001"),
                BigUint::from(2_000u64),
                ManagedBuffer::from(b"Beneficiary"),
                OWNER.to_managed_address(),
            );

            let canonical_hash =
                sc.get_canonical_gsoc_serial_inventory_hash(ManagedBuffer::from(b"proj-001"));
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(canonical_hash.to_boxed_bytes().as_slice());
            *canonical_hash_cell.borrow_mut() = bytes;
        });

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.anchor_gsoc_reserve_proof(
                ManagedBuffer::from(b"proj-001"),
                9_900u64,
                2_000u64,
                1u64,
                ManagedBuffer::from(&canonical_hash_cell.borrow()[..]),
                500u64,
            );
        },
    );

    world
        .query()
        .to(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let proof = sc
                .get_latest_gsoc_reserve_proof(ManagedBuffer::from(b"proj-001"))
                .into_option()
                .expect("gsoc proof should exist");
            assert_eq!(proof.net_active, 7_900u64);
            assert_eq!(proof.serial_count, 1u64);
            assert_eq!(
                proof.itmo_serial_hash,
                ManagedBuffer::from(&canonical_hash_cell.borrow()[..]),
            );
            assert!(
                sc.verify_gsoc_serial_inventory_hash(
                    ManagedBuffer::from(b"proj-001"),
                    ManagedBuffer::from(&canonical_hash_cell.borrow()[..]),
                ),
                "reserve-proof registry view must accept the canonical GSOC serial hash",
            );
        });
}

#[test]
fn gsoc_reserve_proof_anchor_rejects_mismatched_canonical_totals_rs() {
    let mut world = world();
    deploy_tokenized_runtime(&mut world);
    configure_carbon_gsoc_governance(&mut world);

    let gsoc_hash: [u8; 32] = [0xB7u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-proof"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"proj-002"),
                ManagedBuffer::from(b"pai-gsoc-proof"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"dna-gsoc-proof"),
                ManagedBuffer::from(b"ITMO-GSOC-PROOF-002"),
                BigUint::from(9_000u64),
                100u64,
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_gsoc_reserve_proof(
            ManagedBuffer::from(b"proj-002"),
            8_999u64,
            0u64,
            1u64,
            ManagedBuffer::from(&[0x02u8; 32][..]),
            501u64,
        )
        .with_result(ExpectError(
            4u64,
            "CANONICAL_GSOC_ISSUED_MISMATCH: supplied total_issued does not match lifecycle counters",
        ))
        .run();
}

#[test]
fn gsoc_reserve_proof_anchor_rejects_mismatched_canonical_hash_rs() {
    let mut world = world();
    deploy_tokenized_runtime(&mut world);
    configure_carbon_gsoc_governance(&mut world);

    let gsoc_hash: [u8; 32] = [0xC7u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-proof"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"proj-003"),
                ManagedBuffer::from(b"pai-gsoc-proof"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"dna-gsoc-proof"),
                ManagedBuffer::from(b"ITMO-GSOC-PROOF-003"),
                BigUint::from(9_000u64),
                100u64,
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_gsoc_reserve_proof(
            ManagedBuffer::from(b"proj-003"),
            8_910u64,
            0u64,
            1u64,
            ManagedBuffer::from(&[0x03u8; 32][..]),
            502u64,
        )
        .with_result(ExpectError(
            4u64,
            "CANONICAL_GSOC_SERIAL_HASH_MISMATCH: supplied hash does not match canonical serial inventory",
        ))
        .run();
}

#[test]
fn reserve_proof_governance_pause_blocks_vm0042_anchor_rs() {
    let mut world = world();
    seed_vm0042_supply_only_state(&mut world);

    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(GOVERNANCE_CODE)
        .new_address(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-reserve-vm0042"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-reserve-vm0042"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-reserve-vm0042"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-reserve-vm0042"));
        });

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC.to_managed_address());
        },
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                BigUint::from(990u64),
                BigUint::from(10u64),
                BigUint::zero(),
                ManagedBuffer::from(&[0x55u8; 32][..]),
                3_000u64,
            );
        });
}

#[test]
fn reserve_proof_governance_pause_blocks_gsoc_anchor_rs() {
    let mut world = world();
    deploy_tokenized_runtime(&mut world);
    configure_carbon_gsoc_governance(&mut world);

    let gsoc_hash: [u8; 32] = [0xA8u8; 32];
    let canonical_hash_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-paused"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
            );
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"proj-paused-001"),
                ManagedBuffer::from(b"pai-gsoc-paused"),
                1u64,
                ManagedBuffer::from(&gsoc_hash[..]),
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"dna-gsoc-paused"),
                ManagedBuffer::from(b"ITMO-GSOC-PAUSED-001"),
                BigUint::from(10_000u64),
                100u64,
                OWNER.to_managed_address(),
            );

            let canonical_hash = sc
                .get_canonical_gsoc_serial_inventory_hash(ManagedBuffer::from(b"proj-paused-001"));
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(canonical_hash.to_boxed_bytes().as_slice());
            *canonical_hash_cell.borrow_mut() = bytes;
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-reserve-gsoc"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-reserve-gsoc"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-reserve-gsoc"));
        });

    world.current_block().block_timestamp_seconds(7202u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-reserve-gsoc"));
        });

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC.to_managed_address());
        },
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_gsoc_reserve_proof(
                ManagedBuffer::from(b"proj-paused-001"),
                9_900u64,
                0u64,
                1u64,
                ManagedBuffer::from(&canonical_hash_cell.borrow()[..]),
                3_100u64,
            );
        });
}
