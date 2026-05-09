use mrv_come_settlement::ComeSettlement;
use mrv_common::MrvGovernanceModule;
use mrv_governance::MrvGovernance;
use multiversx_sc::types::{ManagedBuffer, TokenIdentifier};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const FARMER: TestAddress = TestAddress::new("farmer");
const BUYER: TestAddress = TestAddress::new("buyer");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-come-settlement");
const GOVERNANCE_SC: TestSCAddress = TestSCAddress::new("mrv-governance");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-come-settlement.mxsc.json");
const GOVERNANCE_CODE: MxscPath =
    MxscPath::new("mxsc:../../governance/output/mrv-governance.mxsc.json");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/come-settlement");
    world.register_contract(CODE_PATH, mrv_come_settlement::ContractBuilder);
    world.register_contract(GOVERNANCE_CODE, mrv_governance::ContractBuilder);
    world
}

#[test]
fn come_settlement_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
        });
}

#[test]
fn come_settlement_create_settlement_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(FARMER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-001"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(10_000u64),
                ManagedBuffer::from(b"bafyreason001"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let record = sc
                .get_settlement(ManagedBuffer::from(b"settlement-001"))
                .into_option()
                .unwrap();
            assert_eq!(
                record.settlement_id.to_boxed_bytes().as_slice(),
                b"settlement-001"
            );
            assert_eq!(record.from, FARMER.to_managed_address());
            assert_eq!(record.to, BUYER.to_managed_address());
            assert_eq!(record.amount_scaled, BigUint::from(10_000u64));
            assert_eq!(record.status, 0u8); // STATUS_PENDING
            assert!(record.cancel_reason_cid.is_empty());
        });
}

#[test]
fn come_settlement_cancel_settlement_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(FARMER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-002"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(5_000u64),
                ManagedBuffer::from(b"bafyreason002"),
            );
            sc.cancel_settlement(
                ManagedBuffer::from(b"settlement-002"),
                ManagedBuffer::from(b"bafycancel002"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let record = sc
                .get_settlement(ManagedBuffer::from(b"settlement-002"))
                .into_option()
                .unwrap();
            assert_eq!(record.status, 3u8); // STATUS_CANCELLED
            assert_eq!(
                record.reason_cid.to_boxed_bytes().as_slice(),
                b"bafyreason002"
            );
            assert_eq!(
                record.cancel_reason_cid.to_boxed_bytes().as_slice(),
                b"bafycancel002"
            );
        });
}

#[test]
fn come_settlement_execute_non_pending_fails_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(FARMER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Create and cancel a settlement
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-003"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(8_000u64),
                ManagedBuffer::from(b"bafyreason003"),
            );
            sc.cancel_settlement(
                ManagedBuffer::from(b"settlement-003"),
                ManagedBuffer::from(b"bafycancel003"),
            );
        });

    // Attempt to execute a cancelled settlement
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "settlement not funded \u{2014} call fundSettlement first",
        ))
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.execute_settlement(ManagedBuffer::from(b"settlement-003"));
        });
}

#[test]
fn come_settlement_create_with_zero_amount_fails_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(FARMER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "amount must be positive"))
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-004"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::zero(),
                ManagedBuffer::from(b"bafyreason004"),
            );
        });
}

#[test]
fn come_settlement_fund_rejects_pre_existing_escrow_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .account(FARMER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-prefunded"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(10_000u64),
                ManagedBuffer::from(b"bafyreason-prefunded"),
            );
        });

    world
        .tx()
        .from(FARMER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0u64, 10_000u64).unwrap())
        .returns(ExpectError(4u64, "settlement escrow already funded"))
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let settlement_id = ManagedBuffer::from(b"settlement-prefunded");
            sc.settlement_escrow(&settlement_id)
                .set(BigUint::from(1u64));
            sc.fund_settlement(settlement_id);
        });
}

#[test]
fn come_settlement_execute_pending_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .account(FARMER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(100_000u64));
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Create settlement: farmer → buyer, 10_000 COME
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-exec-001"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(10_000u64),
                ManagedBuffer::from(b"bafyreason-exec-001"),
            );
        });

    // Fund settlement: farmer deposits the exact COME amount
    world
        .tx()
        .from(FARMER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 10_000u64).unwrap())
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.fund_settlement(ManagedBuffer::from(b"settlement-exec-001"));
        });

    // Verify status is now "funded"
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let record = sc
                .get_settlement(ManagedBuffer::from(b"settlement-exec-001"))
                .into_option()
                .unwrap();
            assert_eq!(record.status, 1u8); // STATUS_FUNDED
        });

    // Execute settlement: governance triggers the ESDT transfer
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.execute_settlement(ManagedBuffer::from(b"settlement-exec-001"));
        });

    // Verify status is now "settled"
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let record = sc
                .get_settlement(ManagedBuffer::from(b"settlement-exec-001"))
                .into_option()
                .unwrap();
            assert_eq!(record.status, 2u8); // STATUS_SETTLED
        });
}

#[test]
fn come_settlement_expiry_flow_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .account(FARMER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(100_000u64));
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Create and fund settlement
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-exp-001"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(5_000u64),
                ManagedBuffer::from(b"bafyreason-exp"),
            );
        });

    world
        .tx()
        .from(FARMER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 5_000u64).unwrap())
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.fund_settlement(ManagedBuffer::from(b"settlement-exp-001"));
        });

    // Advance block round past expiry (current_round + 1_000_000 + 1)
    world.current_block().block_round(1_000_002u64);

    // Expire the settlement
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.expire_settlement(ManagedBuffer::from(b"settlement-exp-001"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let record = sc
                .get_settlement(ManagedBuffer::from(b"settlement-exp-001"))
                .into_option()
                .unwrap();
            assert_eq!(record.status, 4u8); // STATUS_EXPIRED
        });
}

#[test]
fn come_settlement_cancel_funded_refund_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .account(FARMER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(100_000u64));
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-cancel-funded"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(8_000u64),
                ManagedBuffer::from(b"bafyreason-cf"),
            );
        });

    world
        .tx()
        .from(FARMER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 8_000u64).unwrap())
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.fund_settlement(ManagedBuffer::from(b"settlement-cancel-funded"));
        });

    // Cancel the funded settlement — should refund farmer
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.cancel_settlement(
                ManagedBuffer::from(b"settlement-cancel-funded"),
                ManagedBuffer::from(b"bafycancel-cf"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let record = sc
                .get_settlement(ManagedBuffer::from(b"settlement-cancel-funded"))
                .into_option()
                .unwrap();
            assert_eq!(record.status, 3u8); // STATUS_CANCELLED
            assert_eq!(
                record.reason_cid.to_boxed_bytes().as_slice(),
                b"bafyreason-cf"
            );
            assert_eq!(
                record.cancel_reason_cid.to_boxed_bytes().as_slice(),
                b"bafycancel-cf"
            );
        });
}

#[test]
fn come_settlement_migrate_settlements_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(FARMER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-migrate-001"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(1_000u64),
                ManagedBuffer::from(b"bafyreason-migrate"),
            );
        });

    // Owner migrates the settlement record (re-encodes with expiry_round)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let mut ids = MultiValueEncoded::new();
            ids.push(ManagedBuffer::from(b"settlement-migrate-001"));
            sc.migrate_settlements(ids);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            let record = sc
                .get_settlement(ManagedBuffer::from(b"settlement-migrate-001"))
                .into_option()
                .unwrap();
            assert_eq!(record.expiry_round, 0u64); // Still 0 for pending
            assert_eq!(record.status, 0u8); // STATUS_PENDING
            assert!(record.cancel_reason_cid.is_empty());
        });
}

#[test]
fn come_settlement_upgrade_rejects_legacy_storage_version_without_explicit_migration() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.storage_version().set(1u32);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "unsupported legacy storage version; explicit migration required",
        ))
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.upgrade();
        });
}

#[test]
fn come_settlement_governance_pause_blocks_create_and_fund_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .account(FARMER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(100_000u64));
    world.account(BUYER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-paused-funded"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(5_000u64),
                ManagedBuffer::from(b"bafyreason-paused-funded"),
            );
        });

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
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-settlement-001"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-settlement-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-settlement-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-settlement-001"));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.create_settlement(
                ManagedBuffer::from(b"settlement-paused-create"),
                FARMER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("COME-abcdef"),
                BigUint::from(6_000u64),
                ManagedBuffer::from(b"bafyreason-paused-create"),
            );
        });

    world
        .tx()
        .from(FARMER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 5_000u64).unwrap())
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_come_settlement::contract_obj, |sc| {
            sc.fund_settlement(ManagedBuffer::from(b"settlement-paused-funded"));
        });
}
