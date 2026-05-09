use mrv_atomic_swap::AtomicSwap;
use mrv_governance::MrvGovernance;
use multiversx_sc::types::{ManagedBuffer, TokenIdentifier};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const BUYER: TestAddress = TestAddress::new("buyer");
const DEALER: TestAddress = TestAddress::new("dealer");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-atomic-swap");
const GOVERNANCE_SC: TestSCAddress = TestSCAddress::new("mrv-governance");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-atomic-swap.mxsc.json");
const GOVERNANCE_CODE: MxscPath =
    MxscPath::new("mxsc:../../governance/output/mrv-governance.mxsc.json");

macro_rules! allow_dvcu_token {
    ($sc:expr) => {
        $sc.allow_asset_token(TokenIdentifier::from("DVCU-123456"));
    };
}

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/atomic-swap");
    world.register_contract(CODE_PATH, mrv_atomic_swap::ContractBuilder);
    world.register_contract(GOVERNANCE_CODE, mrv_governance::ContractBuilder);
    world
}

#[test]
fn atomic_swap_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            assert_eq!(
                sc.come_token_id().get(),
                TokenIdentifier::from("COME-abcdef")
            );
        });
}

#[test]
fn atomic_swap_create_rfq_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-001"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            let rfq = sc.get_rfq(ManagedBuffer::from(b"RFQ-001"));
            assert!(rfq.is_some());
            let rfq = rfq.into_option().unwrap();
            assert_eq!(rfq.status, 0u8); // PENDING_DEPOSIT
            assert_eq!(rfq.margin_amount, BigUint::from(500u64));
        });
}

#[test]
fn atomic_swap_create_rfq_requires_allowed_asset_token_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "asset token not allowed"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-NOT-ALLOWED"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            assert!(sc.is_asset_token_allowed(TokenIdentifier::from("DVCU-123456")));
            sc.remove_asset_token(TokenIdentifier::from("DVCU-123456"));
            assert!(!sc.is_asset_token_allowed(TokenIdentifier::from("DVCU-123456")));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "asset token not allowed"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-REMOVED-ASSET"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });
}

#[test]
fn atomic_swap_create_rfq_rejects_same_buyer_and_dealer_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "buyer and dealer must be distinct"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-SELF"),
                BUYER.to_managed_address(),
                BUYER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });
}

fn deploy_atomic_swap_with_rfq(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-010"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });
}

#[test]
fn atomic_swap_deposit_margin_from_non_buyer_fails_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    // Set up accounts — dealer gets COME tokens at creation time
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(BUYER).nonce(1).balance(1_000_000u64);
    world
        .account(DEALER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-010"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    // Dealer tries to deposit margin — should fail because only buyer can
    world
        .tx()
        .from(DEALER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .returns(ExpectError(4u64, "only buyer can deposit margin"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-010"));
        });
}

#[test]
fn atomic_swap_deposit_margin_rejects_nonzero_token_nonce_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_nft_balance(come_token, 1u64, BigUint::from(500u64), ());
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-NONCE"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 1u64, 500u64).unwrap())
        .returns(ExpectError(4u64, "FUNGIBLE_ONLY: token nonce must be 0"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-NONCE"));
        });
}

#[test]
fn atomic_swap_create_duplicate_rfq_fails_rs() {
    let mut world = world();
    deploy_atomic_swap_with_rfq(&mut world);

    // Attempt to create same RFQ again
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "RFQ already exists"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-010"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });
}

#[test]
fn atomic_swap_auto_reclaim_before_expiry_fails_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-020"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    // Buyer deposits margin
    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-020"));
        });

    // Epoch is 0 (before expiry of 100_000)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "NOT_EXPIRED"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.auto_reclaim(ManagedBuffer::from(b"RFQ-020"));
        });
}

#[test]
fn atomic_swap_deposit_margin_and_settle_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");
    let rwa_token: TestTokenIdentifier = TestTokenIdentifier::new("DVCU-123456");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world
        .account(DEALER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(rwa_token, BigUint::from(10_000u64));

    // Deploy
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    // Create RFQ: buyer ↔ dealer, 1000 DVCU, 500 COME margin, 10 fee, expiry 100_000
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-SETTLE-001"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    // Buyer deposits margin
    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-SETTLE-001"));
        });

    // Verify status is DEPOSITED (1)
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            let rfq = sc
                .get_rfq(ManagedBuffer::from(b"RFQ-SETTLE-001"))
                .into_option()
                .unwrap();
            assert_eq!(rfq.status, 1u8); // RFQ_DEPOSITED
        });

    // Dealer settles: sends RWA tokens as payment
    world
        .tx()
        .from(DEALER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(rwa_token, 0, 1000u64).unwrap())
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.settle(ManagedBuffer::from(b"RFQ-SETTLE-001"));
        });

    // Verify status is COMPLETED (3)
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            let rfq = sc
                .get_rfq(ManagedBuffer::from(b"RFQ-SETTLE-001"))
                .into_option()
                .unwrap();
            assert_eq!(rfq.status, 3u8); // RFQ_COMPLETED
        });
}

#[test]
fn atomic_swap_auto_reclaim_after_expiry_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    // Create RFQ with expiry at epoch 100_000
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-RECLAIM-001"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    // Buyer deposits margin
    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-RECLAIM-001"));
        });

    // Advance epoch past expiry
    world.current_block().block_epoch(100_001u64);

    // Anyone can trigger auto-reclaim after expiry
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.auto_reclaim(ManagedBuffer::from(b"RFQ-RECLAIM-001"));
        });

    // Verify status is EXPIRED (4) — margin returned to buyer
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            let rfq = sc
                .get_rfq(ManagedBuffer::from(b"RFQ-RECLAIM-001"))
                .into_option()
                .unwrap();
            assert_eq!(rfq.status, 4u8); // RFQ_EXPIRED
        });
}

#[test]
fn atomic_swap_cancel_flow_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-CANCEL-001"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    // Buyer deposits margin
    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-CANCEL-001"));
        });

    // Buyer cancels
    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.cancel_rfq(ManagedBuffer::from(b"RFQ-CANCEL-001"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            let rfq = sc
                .get_rfq(ManagedBuffer::from(b"RFQ-CANCEL-001"))
                .into_option()
                .unwrap();
            assert_eq!(rfq.status, 5u8); // RFQ_CANCELLED
        });
}

#[test]
fn atomic_swap_cancel_by_dealer_fails_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-CANCEL-DEALER"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-CANCEL-DEALER"));
        });

    world
        .tx()
        .from(DEALER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "only buyer can cancel"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.cancel_rfq(ManagedBuffer::from(b"RFQ-CANCEL-DEALER"));
        });
}

#[test]
fn atomic_swap_cancel_rejects_unbacked_locked_balance_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world.account(DEALER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-CANCEL-DESYNC"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-CANCEL-DESYNC"));
            sc.locked_balances(&BUYER.to_managed_address())
                .set(BigUint::from(700u64));
        });

    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "LOCKED_BALANCE_NOT_BACKED: escrow balance mismatch",
        ))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.cancel_rfq(ManagedBuffer::from(b"RFQ-CANCEL-DESYNC"));
        });
}

#[test]
fn atomic_swap_settle_after_expiry_rejection_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");
    let rwa_token: TestTokenIdentifier = TestTokenIdentifier::new("DVCU-123456");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world
        .account(DEALER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(rwa_token, BigUint::from(10_000u64));

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-SETTLE-EXP"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
            );
        });

    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-SETTLE-EXP"));
        });

    // Advance epoch past expiry
    world.current_block().block_epoch(100_001u64);

    // Dealer tries to settle after expiry — should fail
    world
        .tx()
        .from(DEALER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(rwa_token, 0, 1000u64).unwrap())
        .returns(ExpectError(4u64, "EXPIRED"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.settle(ManagedBuffer::from(b"RFQ-SETTLE-EXP"));
        });
}

#[test]
fn atomic_swap_governance_pause_blocks_mutations_rs() {
    let mut world = world();

    let come_token: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(BUYER)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(come_token, BigUint::from(10_000u64));
    world.account(DEALER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.init(TokenIdentifier::from("COME-abcdef"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            allow_dvcu_token!(sc);
            sc.create_rfq(
                ManagedBuffer::from(b"RFQ-PAUSED"),
                BUYER.to_managed_address(),
                DEALER.to_managed_address(),
                TokenIdentifier::from("DVCU-123456"),
                BigUint::from(1000u64),
                BigUint::from(500u64),
                BigUint::from(10u64),
                100_000u64,
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
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-atomic-001"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-atomic-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-atomic-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-atomic-001"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC.to_managed_address());
        });

    world
        .tx()
        .from(BUYER)
        .to(SC_ADDRESS)
        .payment(Payment::try_new(come_token, 0, 500u64).unwrap())
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_atomic_swap::contract_obj, |sc| {
            sc.deposit_margin(ManagedBuffer::from(b"RFQ-PAUSED"));
        });
}
