use mrv_buffer_pool::BufferPool;
use mrv_common::MrvGovernanceModule;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const CARBON_CREDIT: TestAddress = TestAddress::new("carbon-credit");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-buffer-pool");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-buffer-pool.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/buffer-pool");
    world.register_contract(CODE_PATH, mrv_buffer_pool::ContractBuilder);
    world
}

#[test]
fn buffer_pool_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(CARBON_CREDIT).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                CARBON_CREDIT.to_managed_address(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
            assert_eq!(sc.carbon_credit_addr().get(), CARBON_CREDIT.to_managed_address());
        });
}

#[test]
fn buffer_pool_deposit_buffer_credits_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(CARBON_CREDIT).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                CARBON_CREDIT.to_managed_address(),
            );
        });

    // Deposit from the authorized carbon-credit contract address
    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.deposit_buffer_credits(
                ManagedBuffer::from(b"project-001"),
                BigUint::from(5_000u64),
                1u64,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            let record = sc
                .get_buffer_record(ManagedBuffer::from(b"project-001"))
                .into_option()
                .unwrap();
            assert_eq!(record.total_deposited, BigUint::from(5_000u64));
            assert_eq!(record.total_cancelled, BigUint::zero());
            assert_eq!(record.total_replenished, BigUint::zero());
            assert_eq!(sc.get_total_pool_balance(), BigUint::from(5_000u64));
        });
}

#[test]
fn buffer_pool_rejects_unauthorized_deposit_rs() {
    let mut world = world();

    let unauthorized: TestAddress = TestAddress::new("unauthorized");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(CARBON_CREDIT).nonce(1).balance(1_000_000u64);
    world.account(unauthorized).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                CARBON_CREDIT.to_managed_address(),
            );
        });

    world
        .tx()
        .from(unauthorized)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.deposit_buffer_credits(
                ManagedBuffer::from(b"project-001"),
                BigUint::from(1_000u64),
                1u64,
            );
        });
}

/// Helper: deploys buffer-pool and deposits 10_000 for project-010.
fn deploy_and_deposit(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(CARBON_CREDIT).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                CARBON_CREDIT.to_managed_address(),
            );
        });

    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.deposit_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(10_000u64),
                1u64,
            );
        });
}

#[test]
fn buffer_pool_cancel_buffer_credits_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.cancel_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(3_000u64),
                ManagedBuffer::from(b"bafyreason-fire-event"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            let record = sc
                .get_buffer_record(ManagedBuffer::from(b"project-010"))
                .into_option()
                .unwrap();
            assert_eq!(record.total_deposited, BigUint::from(10_000u64));
            assert_eq!(record.total_cancelled, BigUint::from(3_000u64));
            assert_eq!(sc.get_total_pool_balance(), BigUint::from(7_000u64));
        });
}

#[test]
fn buffer_pool_replenish_buffer_credits_small_amount_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

    // 10% of 10_000 = 1_000. Replenish 500 (under threshold) from authorized caller.
    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(500u64),
                ManagedBuffer::from(b"bafyjustification001"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            let record = sc
                .get_buffer_record(ManagedBuffer::from(b"project-010"))
                .into_option()
                .unwrap();
            assert_eq!(record.total_replenished, BigUint::from(500u64));
            assert_eq!(sc.get_total_pool_balance(), BigUint::from(10_500u64));
        });
}

#[test]
fn buffer_pool_replenish_above_threshold_non_governance_fails_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

    // 10% of 10_000 = 1_000. Replenish 2_000 (above threshold) from non-governance caller.
    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "replenishment exceeds 10% threshold \u{2014} governance approval required",
        ))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(2_000u64),
                ManagedBuffer::from(b"bafyjustification002"),
            );
        });
}

#[test]
fn buffer_pool_cancel_nonexistent_project_fails_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "no buffer record for project"))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.cancel_buffer_credits(
                ManagedBuffer::from(b"project-NONEXISTENT"),
                BigUint::from(1_000u64),
                ManagedBuffer::from(b"bafyreason"),
            );
        });
}

#[test]
fn buffer_pool_replenishment_cooldown_enforcement_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

    // First replenishment at epoch 0 — should succeed
    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(500u64),
                ManagedBuffer::from(b"bafyjust-cooldown-1"),
            );
        });

    // Second replenishment at epoch 100 — before cooldown (1500 epochs)
    world.current_block().block_epoch(100u64);

    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "replenishment rate limit: 1 per 90 days per project"))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(500u64),
                ManagedBuffer::from(b"bafyjust-cooldown-2"),
            );
        });
}

#[test]
fn buffer_pool_fully_depleted_governance_required_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

    // Cancel the full balance (10_000) to deplete the project
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.cancel_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(10_000u64),
                ManagedBuffer::from(b"bafyreason-deplete"),
            );
        });

    // Non-governance caller tries to replenish a fully depleted project
    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "buffer fully depleted \u{2014} governance approval required for any replenishment",
        ))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(100u64),
                ManagedBuffer::from(b"bafyjust-depleted"),
            );
        });
}
