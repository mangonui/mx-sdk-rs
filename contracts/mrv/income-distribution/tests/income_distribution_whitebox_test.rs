use mrv_common::MrvGovernanceModule;
use mrv_governance::MrvGovernance;
use mrv_income_distribution::IncomeDistribution;
use multiversx_sc::types::{ManagedBuffer, TokenIdentifier};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-income-distribution");
const GOVERNANCE_SC: TestSCAddress = TestSCAddress::new("mrv-governance");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-income-distribution.mxsc.json");
const GOVERNANCE_CODE: MxscPath =
    MxscPath::new("mxsc:../../governance/output/mrv-governance.mxsc.json");
const COME_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");
const WRONG_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("FAKE-123456");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/income-distribution");
    world.register_contract(CODE_PATH, mrv_income_distribution::ContractBuilder);
    world.register_contract(GOVERNANCE_CODE, mrv_governance::ContractBuilder);
    world
}

fn deploy_income_distribution(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64))
        .esdt_nft_balance(COME_TOKEN, 1u64, BigUint::from(10_000u64), ())
        .esdt_balance(WRONG_TOKEN, BigUint::from(500_000u64));

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });
}

fn claim_leaf_hash(
    sc: &mrv_income_distribution::ContractObj<DebugApi>,
    dist_id: &[u8],
    holder: TestAddress,
    claim_amount: u64,
    total_amount: u64,
) -> [u8; 32] {
    let mut leaf_preimage = ManagedBuffer::from(b"MRV_YIELD_CLAIM_LEAF_V2");
    append_len_prefixed(&mut leaf_preimage, &ManagedBuffer::from(dist_id));
    append_len_prefixed(
        &mut leaf_preimage,
        holder.to_managed_address().as_managed_buffer(),
    );
    append_len_prefixed(
        &mut leaf_preimage,
        &BigUint::from(claim_amount).to_bytes_be_buffer(),
    );
    append_len_prefixed(
        &mut leaf_preimage,
        &BigUint::from(total_amount).to_bytes_be_buffer(),
    );
    let hash = sc.crypto().keccak256(&leaf_preimage);
    let hash_bytes = hash.as_managed_buffer().to_boxed_bytes();
    let mut root = [0u8; 32];
    root.copy_from_slice(hash_bytes.as_slice());
    root
}

fn append_len_prefixed(dest: &mut ManagedBuffer<DebugApi>, value: &ManagedBuffer<DebugApi>) {
    let len = value.len() as u32;
    dest.append_bytes(&len.to_be_bytes());
    dest.append(value);
}

/// Helper: deploys contract, funds a distribution with `dist_id`, returns the
/// merkle root bytes for a single-leaf tree containing (dist_id, holder, claim_amount).
fn deploy_and_fund_with_claim(
    world: &mut ScenarioWorld,
    dist_id: &[u8],
    holder: TestAddress,
    claim_amount: u64,
    fund_amount: u64,
) -> [u8; 32] {
    use std::cell::RefCell;

    let merkle_root_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            *merkle_root_cell.borrow_mut() =
                claim_leaf_hash(&sc, dist_id, holder, claim_amount, fund_amount);
        });

    let merkle_root = *merkle_root_cell.borrow();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, fund_amount).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(dist_id),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    merkle_root
}

// ============================================================================
// EXISTING TESTS
// ============================================================================

#[test]
fn income_distribution_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
            assert_eq!(
                sc.come_token_id().get(),
                TokenIdentifier::from("COME-abcdef")
            );
        });
}

#[test]
fn income_distribution_rejects_zero_governance_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .returns(ExpectError(4u64, "governance must not be zero"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(ManagedAddress::zero(), TokenIdentifier::from("COME-abcdef"));
        });
}

#[test]
fn income_distribution_fund_distribution_rs() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 50_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-001"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest001"),
                6_000u64,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            let dist = sc
                .get_distribution(ManagedBuffer::from(b"dist-001"))
                .into_option()
                .unwrap();
            assert_eq!(dist.total_amount_scaled, BigUint::from(50_000u64));
            assert_eq!(dist.total_claimed_scaled, BigUint::zero());
            assert_eq!(dist.expiry_epoch, 6_000u64);
            assert!(!dist.reclaimed);
            assert_eq!(
                sc.distribution_escrow(&ManagedBuffer::from(b"dist-001"))
                    .get(),
                BigUint::from(50_000u64)
            );
        });
}

#[test]
fn income_distribution_reclaim_expired_rs() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xBBu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 30_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-002"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest002"),
                6_000u64,
            );
        });

    // Advance epoch past expiry
    world.current_block().block_epoch(6_001u64);

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.reclaim_expired(ManagedBuffer::from(b"dist-002"));
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            let dist = sc
                .get_distribution(ManagedBuffer::from(b"dist-002"))
                .into_option()
                .unwrap();
            assert!(dist.reclaimed);
            assert_eq!(
                sc.distribution_escrow(&ManagedBuffer::from(b"dist-002"))
                    .get(),
                BigUint::zero()
            );
        });
}

#[test]
fn income_distribution_claim_uses_distribution_escrow_not_global_balance_rs() {
    let mut world = world();
    let holder: TestAddress = TestAddress::new("holder-escrow");

    deploy_income_distribution(&mut world);
    world.account(holder).nonce(1).balance(0u64);

    let claim_amount: u64 = 1_000;
    let mut dist_a_root = [0u8; 32];
    let mut dist_b_root = [0u8; 32];
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            dist_a_root = claim_leaf_hash(&sc, b"dist-escrow-a", holder, claim_amount, 1_000);
            dist_b_root = claim_leaf_hash(&sc, b"dist-escrow-b", holder, claim_amount, 1_000);
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 1_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-escrow-a"),
                ManagedBuffer::from(&dist_a_root[..]),
                100u64,
                ManagedBuffer::from(b"bafyescrowa"),
                6_000u64,
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 1_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-escrow-b"),
                ManagedBuffer::from(&dist_b_root[..]),
                100u64,
                ManagedBuffer::from(b"bafyescrowb"),
                6_000u64,
            );
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.distribution_escrow(&ManagedBuffer::from(b"dist-escrow-a"))
                .clear();
        },
    );

    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "INSUFFICIENT_DISTRIBUTION_ESCROW: distribution does not hold enough COME for this claim",
        ))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-escrow-a"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert_eq!(
                sc.distribution_escrow(&ManagedBuffer::from(b"dist-escrow-b"))
                    .get(),
                BigUint::from(1_000u64)
            );
        });
}

#[test]
fn income_distribution_reclaim_before_expiry_fails_rs() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xCCu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 20_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-003"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest003"),
                6_000u64,
            );
        });

    // Epoch still within expiry window
    world.current_block().block_epoch(5_999u64);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "distribution not yet expired"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.reclaim_expired(ManagedBuffer::from(b"dist-003"));
        });
}

#[test]
fn income_distribution_fund_with_wrong_token_fails_rs() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xDDu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(WRONG_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "must pay with COME token"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-004"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest004"),
                6_000u64,
            );
        });
}

#[test]
fn income_distribution_fund_with_nonzero_nonce_fails_rs() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xDEu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 1, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "FUNGIBLE_ONLY: token nonce must be 0"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-nonce-001"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest-nonce"),
                6_000u64,
            );
        });
}

#[test]
fn income_distribution_claim_with_valid_proof_rs() {
    use std::cell::RefCell;

    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    // Step 1: Compute the Merkle leaf (single-leaf tree, leaf == root) inside a
    // whitebox query so we get the exact keccak256 the contract will produce.
    // leaf = keccak256(domain || len(distribution_id) || distribution_id
    //                  || len(holder_address) || holder_address
    //                  || len(amount_be) || amount_be
    //                  || len(total_amount_be) || total_amount_be)
    let merkle_root_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);
    let claim_amount: u64 = 25_000;

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            *merkle_root_cell.borrow_mut() =
                claim_leaf_hash(&sc, b"dist-claim-001", holder, claim_amount, 50_000);
        });

    let merkle_root = *merkle_root_cell.borrow();

    // Step 2: Fund the distribution with the computed Merkle root.
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 50_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-claim-001"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest-claim-001"),
                6_000u64,
            );
        });

    // Step 3: Holder claims with an empty proof (single-leaf tree: leaf IS the root).
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-claim-001"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });

    // Step 4: Verify the claim was recorded and total_claimed_scaled updated.
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            let dist = sc
                .get_distribution(ManagedBuffer::from(b"dist-claim-001"))
                .into_option()
                .unwrap();
            assert_eq!(dist.total_claimed_scaled, BigUint::from(claim_amount));
        });
}

// ============================================================================
// NEW TESTS — T1 through T28
// ============================================================================

// T1: fund_distribution_empty_id_fails — empty distribution_id
#[test]
fn fund_distribution_empty_id_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "empty distribution_id"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::new(), // empty
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });
}

// T2: fund_distribution_bad_merkle_root_length_fails — merkle_root != 32 bytes
#[test]
fn fund_distribution_bad_merkle_root_length_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "merkle_root must be 32 bytes"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-bad-root"),
                ManagedBuffer::from(&[0xFFu8; 16][..]), // 16 bytes, not 32
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });
}

// T3: fund_distribution_zero_merkle_root_fails — all-zero merkle root
#[test]
fn fund_distribution_zero_merkle_root_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "merkle_root must not be all zeros"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-zero-root"),
                ManagedBuffer::from(&[0u8; 32][..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });
}

// T4: fund_distribution_empty_manifest_cid_fails — empty manifest_cid
#[test]
fn fund_distribution_empty_manifest_cid_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "empty manifest_cid"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-empty-cid"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::new(), // empty
                6_000u64,
            );
        });
}

// T5: fund_distribution_expiry_too_soon_fails — expiry < current + 5000
#[test]
fn fund_distribution_expiry_too_soon_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    // Set current epoch to 100 — expiry must be >= 5100
    world.current_block().block_epoch(100u64);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(
            4u64,
            "expiry_epoch must be at least MINIMUM_CLAIM_WINDOW_EPOCHS from now",
        ))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-soon"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                5_099u64, // current(100) + 5000 = 5100, so 5099 is too soon
            );
        });
}

// T6: fund_distribution_duplicate_id_fails — same distribution_id twice
#[test]
fn fund_distribution_duplicate_id_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    // First funding succeeds
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-dup"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // Second funding with same id fails
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "distribution already exists"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-dup"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });
}

// T7: fund_distribution_zero_amount_fails — payment amount is 0
// The MultiversX framework rejects zero-amount ESDT payments at the
// `Payment::try_new` level (returns NonZeroError), so a zero payment
// can never reach the contract's `require!`. This test verifies the
// framework-level invariant holds.
#[test]
fn fund_distribution_zero_amount_fails() {
    let result = Payment::<StaticApi>::try_new(COME_TOKEN, 0, 0u64);
    assert!(
        result.is_err(),
        "framework must reject zero-amount ESDT payment"
    );
}

// T8: init_invalid_token_id_fails — invalid ESDT identifier
#[test]
fn init_invalid_token_id_fails() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .returns(ExpectError(4u64, "invalid COME token ID"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("INVALID"), // missing hex suffix
            );
        });
}

// T9: claim_yield_paused_fails — distribution is paused
#[test]
fn claim_yield_paused_fails() {
    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    let claim_amount: u64 = 10_000;
    let merkle_root =
        deploy_and_fund_with_claim(&mut world, b"dist-paused", holder, claim_amount, 50_000u64);
    let _ = merkle_root;

    // Pause the distribution
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.pause_distribution(ManagedBuffer::from(b"dist-paused"));
        },
    );

    // Attempt to claim — should fail
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "DISTRIBUTION_PAUSED: claims are temporarily suspended",
        ))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-paused"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });
}

// T10: claim_yield_not_found_fails — non-existent distribution
#[test]
fn claim_yield_not_found_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let holder: TestAddress = TestAddress::new("holder");
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "distribution not found"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"nonexistent"),
                BigUint::from(1_000u64),
                ManagedVec::new(),
            );
        });
}

// T11: claim_yield_expired_fails — current epoch > expiry
#[test]
fn claim_yield_expired_fails() {
    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    let claim_amount: u64 = 10_000;
    let _merkle_root =
        deploy_and_fund_with_claim(&mut world, b"dist-expired", holder, claim_amount, 50_000u64);

    // Advance past expiry
    world.current_block().block_epoch(6_001u64);

    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "DISTRIBUTION_EXPIRED"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-expired"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });
}

// T12: claim_yield_reclaimed_fails — distribution already reclaimed
#[test]
fn claim_yield_reclaimed_fails() {
    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    let claim_amount: u64 = 10_000;
    let _merkle_root = deploy_and_fund_with_claim(
        &mut world,
        b"dist-reclaimed",
        holder,
        claim_amount,
        50_000u64,
    );

    // Expire and reclaim
    world.current_block().block_epoch(6_001u64);

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.reclaim_expired(ManagedBuffer::from(b"dist-reclaimed"));
        },
    );

    // Reset epoch so it's not expired (reclaimed check comes after expiry check)
    world.current_block().block_epoch(5_000u64);

    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "distribution already reclaimed"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-reclaimed"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });
}

// T13: claim_yield_already_claimed_fails — same holder claims twice
#[test]
fn claim_yield_already_claimed_fails() {
    use std::cell::RefCell;

    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    let claim_amount: u64 = 10_000;
    let merkle_root_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            *merkle_root_cell.borrow_mut() =
                claim_leaf_hash(&sc, b"dist-double-claim", holder, claim_amount, 50_000);
        });

    let merkle_root = *merkle_root_cell.borrow();

    // Fund
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 50_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-double-claim"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // First claim succeeds
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-double-claim"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });

    // Second claim fails
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "ALREADY_CLAIMED"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-double-claim"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });
}

// T14: claim_yield_invalid_proof_fails — wrong merkle proof
#[test]
fn claim_yield_invalid_proof_fails() {
    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    // Fund with a known root that does NOT match the claim params
    let merkle_root: [u8; 32] = [0xFFu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 50_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-bad-proof"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // Claim with empty proof — leaf hash won't match the 0xFF root
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "INVALID_MERKLE_PROOF"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-bad-proof"),
                BigUint::from(10_000u64),
                ManagedVec::new(),
            );
        });
}

// T15: claim_yield_exceeds_funded_fails — claim more than funded
#[test]
fn claim_yield_exceeds_funded_fails() {
    use std::cell::RefCell;

    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    // Fund with only 5_000 but create a merkle root for a 10_000 claim
    let claim_amount: u64 = 10_000;
    let fund_amount: u64 = 5_000;
    let merkle_root_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            *merkle_root_cell.borrow_mut() =
                claim_leaf_hash(&sc, b"dist-exceed", holder, claim_amount, fund_amount);
        });

    let merkle_root = *merkle_root_cell.borrow();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, fund_amount).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-exceed"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // Claim 10_000 against 5_000 funded
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "CLAIMS_EXCEED_FUNDED: cumulative claims would exceed distribution total",
        ))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-exceed"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });
}

#[test]
fn claim_yield_rejects_root_bound_to_different_distribution_total() {
    use std::cell::RefCell;

    let mut world = world();
    let holder: TestAddress = TestAddress::new("holder-total-bound");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    let claim_amount: u64 = 1_000;
    let funded_amount: u64 = 5_000;
    let tree_declared_total: u64 = 10_000;
    let merkle_root_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            *merkle_root_cell.borrow_mut() = claim_leaf_hash(
                &sc,
                b"dist-total-bound",
                holder,
                claim_amount,
                tree_declared_total,
            );
        });

    let merkle_root = *merkle_root_cell.borrow();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, funded_amount).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-total-bound"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafytotalbound"),
                6_000u64,
            );
        });

    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "INVALID_MERKLE_PROOF"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-total-bound"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });
}

// T16: pause_distribution_and_unpause — happy path for both
#[test]
fn pause_distribution_and_unpause() {
    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    let claim_amount: u64 = 10_000;
    let _merkle_root = deploy_and_fund_with_claim(
        &mut world,
        b"dist-pause-test",
        holder,
        claim_amount,
        50_000u64,
    );

    // Pause
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.pause_distribution(ManagedBuffer::from(b"dist-pause-test"));
        },
    );

    // Verify paused
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert!(
                sc.distribution_paused(&ManagedBuffer::from(b"dist-pause-test"))
                    .get()
            );
        });

    // Claim while paused fails
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "DISTRIBUTION_PAUSED: claims are temporarily suspended",
        ))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-pause-test"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });

    // Unpause
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.unpause_distribution(ManagedBuffer::from(b"dist-pause-test"));
        },
    );

    // Verify unpaused
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert!(
                !sc.distribution_paused(&ManagedBuffer::from(b"dist-pause-test"))
                    .get()
            );
        });

    // Claim after unpause succeeds (with valid proof)
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-pause-test"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });
}

// T17: reclaim_expired_not_found_fails
#[test]
fn reclaim_expired_not_found_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    world.current_block().block_epoch(99_999u64);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "distribution not found"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.reclaim_expired(ManagedBuffer::from(b"nonexistent"));
        });
}

// T18: reclaim_expired_already_reclaimed_fails
#[test]
fn reclaim_expired_already_reclaimed_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xEEu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 20_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-reclaim-twice"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    world.current_block().block_epoch(6_001u64);

    // First reclaim succeeds
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.reclaim_expired(ManagedBuffer::from(b"dist-reclaim-twice"));
        },
    );

    // Second reclaim fails
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "already reclaimed"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.reclaim_expired(ManagedBuffer::from(b"dist-reclaim-twice"));
        });
}

// T19: recover_shortfall_happy_path — create shortfall via storage, recover it
#[test]
fn recover_shortfall_happy_path() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    // Fund a distribution
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 50_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-shortfall"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // Inject a shortfall via whitebox storage write.
    // In production this would happen when sc_balance < unclaimed during reclaim,
    // but simulating that requires draining the contract balance externally which
    // whitebox tests cannot do directly. Setting storage directly is the canonical
    // whitebox approach.
    let shortfall_amount: u64 = 15_000;
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.reclaim_shortfall(&ManagedBuffer::from(b"dist-shortfall"))
                .set(BigUint::from(shortfall_amount));
        },
    );

    // Recover the full shortfall
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, shortfall_amount).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.recover_shortfall(ManagedBuffer::from(b"dist-shortfall"));
        });

    // Verify shortfall is cleared
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            let remaining = sc
                .reclaim_shortfall(&ManagedBuffer::from(b"dist-shortfall"))
                .get();
            assert_eq!(remaining, BigUint::zero());
        });
}

// T20: recover_shortfall_no_shortfall_fails
#[test]
fn recover_shortfall_no_shortfall_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-no-shortfall"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 1_000u64).unwrap())
        .returns(ExpectError(
            4u64,
            "NO_SHORTFALL: no shortfall recorded for this distribution",
        ))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.recover_shortfall(ManagedBuffer::from(b"dist-no-shortfall"));
        });
}

// T21: recover_shortfall_wrong_token_fails
#[test]
fn recover_shortfall_wrong_token_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-wrong-recover"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // Set shortfall via storage
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.reclaim_shortfall(&ManagedBuffer::from(b"dist-wrong-recover"))
                .set(BigUint::from(5_000u64));
        },
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(WRONG_TOKEN, 0, 5_000u64).unwrap())
        .returns(ExpectError(4u64, "must pay with COME token"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.recover_shortfall(ManagedBuffer::from(b"dist-wrong-recover"));
        });
}

// T22: recover_shortfall_zero_amount_fails
#[test]
fn recover_shortfall_zero_amount_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-zero-recover"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // Set shortfall via storage
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.reclaim_shortfall(&ManagedBuffer::from(b"dist-zero-recover"))
                .set(BigUint::from(5_000u64));
        },
    );

    // The MultiversX framework rejects zero-amount ESDT payments at the
    // Payment::try_new level, so a zero payment can never reach the contract.
    let result = Payment::<StaticApi>::try_new(COME_TOKEN, 0, 0u64);
    assert!(
        result.is_err(),
        "framework must reject zero-amount ESDT payment"
    );
}

// T23: recover_shortfall_exceeds_shortfall_fails
#[test]
fn recover_shortfall_exceeds_shortfall_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-exceed-recover"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // Set shortfall = 3_000 via storage
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.reclaim_shortfall(&ManagedBuffer::from(b"dist-exceed-recover"))
                .set(BigUint::from(3_000u64));
        },
    );

    // Try to recover 5_000 against shortfall of 3_000
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 5_000u64).unwrap())
        .returns(ExpectError(
            4u64,
            "RECOVERY_EXCEEDS_SHORTFALL: payment exceeds recorded shortfall",
        ))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.recover_shortfall(ManagedBuffer::from(b"dist-exceed-recover"));
        });
}

// T24: is_claimed_true_after_claim
#[test]
fn is_claimed_true_after_claim() {
    use std::cell::RefCell;

    let mut world = world();

    let holder: TestAddress = TestAddress::new("holder");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world.account(holder).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    let claim_amount: u64 = 8_000;
    let merkle_root_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            *merkle_root_cell.borrow_mut() =
                claim_leaf_hash(&sc, b"dist-is-claimed", holder, claim_amount, 50_000);
        });

    let merkle_root = *merkle_root_cell.borrow();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 50_000u64).unwrap())
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-is-claimed"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });

    // Verify not claimed before claiming
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            let result = sc.is_claimed(
                ManagedBuffer::from(b"dist-is-claimed"),
                holder.to_managed_address(),
            );
            assert!(!result);
        });

    // Claim
    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-is-claimed"),
                BigUint::from(claim_amount),
                ManagedVec::new(),
            );
        });

    // Verify claimed after claiming
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            let result = sc.is_claimed(
                ManagedBuffer::from(b"dist-is-claimed"),
                holder.to_managed_address(),
            );
            assert!(result);
        });
}

// T25: upgrade_works
#[test]
fn upgrade_works() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    // Verify storage_version is set to 1 after init
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert_eq!(sc.storage_version().get(), 1u32);
        });

    // Call upgrade — should not panic and should preserve storage
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.upgrade();
        });

    // Storage version preserved (upgrade is a no-op currently)
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert_eq!(sc.storage_version().get(), 1u32);
            // Governance still intact
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
            // Token ID still intact
            assert_eq!(
                sc.come_token_id().get(),
                TokenIdentifier::from("COME-abcdef")
            );
        });
}

#[test]
fn upgrade_rejects_future_storage_version() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.storage_version().set(99u32);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "unsupported future storage version"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.upgrade();
        });
}

// T26: governance_rotation_is_irreversible — test inherited governance endpoints
#[test]
fn governance_rotation_is_irreversible() {
    let mut world = world();

    let new_gov: TestAddress = TestAddress::new("new_gov");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(new_gov).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    // Current governance proposes new governance
    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.set_governance(new_gov.to_managed_address());
        },
    );

    // Verify pending governance is set
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert_eq!(sc.pending_governance().get(), new_gov.to_managed_address());
            // Current governance is still the original
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
        });

    // New governance accepts
    world
        .tx()
        .from(new_gov)
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.accept_governance();
        });

    // Verify governance is now new_gov
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), new_gov.to_managed_address());
        });

    // Owner can no longer revoke active governance back to owner-only.
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "active governance cannot be revoked"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.revoke_governance();
        });

    // Verify governance remains active after the rejected revoke attempt.
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), new_gov.to_managed_address());
        });
}

// T27: fund_distribution_id_too_long_fails — distribution_id exceeds MAX_DISTRIBUTION_ID_LEN (128)
#[test]
fn fund_distribution_id_too_long_fails() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let merkle_root: [u8; 32] = [0xAAu8; 32];
    // 129 bytes — exceeds MAX_DISTRIBUTION_ID_LEN of 128
    let long_id = [b'X'; 129];

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "distribution_id exceeds maximum length"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(&long_id[..]),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });
}

// T28: fund_distribution_unauthorized_fails — non-governance/non-owner caller
#[test]
fn fund_distribution_unauthorized_fails() {
    let mut world = world();

    let rando: TestAddress = TestAddress::new("rando");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(1_000_000u64));
    world
        .account(rando)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, BigUint::from(100_000u64));

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
            );
        });

    let merkle_root: [u8; 32] = [0xAAu8; 32];

    world
        .tx()
        .from(rando)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-unauth"),
                ManagedBuffer::from(&merkle_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest"),
                6_000u64,
            );
        });
}

#[test]
fn income_distribution_governance_pause_blocks_funding_and_claims_rs() {
    let mut world = world();
    deploy_income_distribution(&mut world);

    let holder: TestAddress = TestAddress::new("holder-paused");
    world.account(holder).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    deploy_and_fund_with_claim(&mut world, b"dist-paused-claim", holder, 500u64, 500u64);

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
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-income-001"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-income-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-income-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-income-001"));
        });

    world.tx().from(GOVERNANCE).to(SC_ADDRESS).whitebox(
        mrv_income_distribution::contract_obj,
        |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC.to_managed_address());
        },
    );

    let paused_root: [u8; 32] = [0xABu8; 32];
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(COME_TOKEN, 0, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.fund_distribution(
                ManagedBuffer::from(b"dist-paused-fund"),
                ManagedBuffer::from(&paused_root[..]),
                100u64,
                ManagedBuffer::from(b"bafymanifest-paused"),
                6_000u64,
            );
        });

    world
        .tx()
        .from(holder)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_income_distribution::contract_obj, |sc| {
            sc.claim_yield(
                ManagedBuffer::from(b"dist-paused-claim"),
                BigUint::from(500u64),
                ManagedVec::new(),
            );
        });
}
