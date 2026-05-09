use mrv_buffer_pool::BufferPool;
use mrv_common::MrvGovernanceModule;
use mrv_governance::MrvGovernance;
use multiversx_sc::types::{ManagedBuffer, TokenIdentifier};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const CARBON_CREDIT: TestAddress = TestAddress::new("carbon-credit");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-buffer-pool");
const GOVERNANCE_SC: TestSCAddress = TestSCAddress::new("mrv-governance");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-buffer-pool.mxsc.json");
const GOVERNANCE_CODE: MxscPath =
    MxscPath::new("mxsc:../../governance/output/mrv-governance.mxsc.json");
const BUFFER_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DVCUBUF-123456");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/buffer-pool");
    world.register_contract(CODE_PATH, mrv_buffer_pool::ContractBuilder);
    world.register_contract(GOVERNANCE_CODE, mrv_governance::ContractBuilder);
    world
}

fn configure_buffer_runtime(world: &mut ScenarioWorld) {
    world.set_esdt_local_roles(
        SC_ADDRESS.to_address(),
        BUFFER_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.set_buffer_token_id(TokenIdentifier::from(BUFFER_TOKEN.as_bytes()));
        });
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
            assert_eq!(
                sc.carbon_credit_addr().get(),
                CARBON_CREDIT.to_managed_address()
            );
            assert!(sc.buffer_token_id().is_empty());
            assert_eq!(sc.total_buffer_minted().get(), BigUint::zero());
            assert_eq!(sc.total_buffer_burned().get(), BigUint::zero());
        });
}

#[test]
fn buffer_pool_token_configuration_rs() {
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
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.set_buffer_token_id(TokenIdentifier::from("DVCUBUF-123456"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            assert_eq!(
                sc.buffer_token_id().get(),
                TokenIdentifier::from("DVCUBUF-123456")
            );
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

    configure_buffer_runtime(&mut world);

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
            assert_eq!(sc.total_buffer_minted().get(), BigUint::from(5_000u64));
            assert_eq!(sc.total_buffer_burned().get(), BigUint::zero());
        });

    world
        .check_account(SC_ADDRESS)
        .esdt_balance(BUFFER_TOKEN, BigUint::from(5_000u64));
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

    configure_buffer_runtime(world);

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
            assert_eq!(sc.total_buffer_minted().get(), BigUint::from(10_000u64));
            assert_eq!(sc.total_buffer_burned().get(), BigUint::from(3_000u64));
        });

    world
        .check_account(SC_ADDRESS)
        .esdt_balance(BUFFER_TOKEN, BigUint::from(7_000u64));
}

#[test]
fn buffer_pool_replenish_buffer_credits_small_amount_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

    world.current_block().block_epoch(1_500u64);

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
            assert_eq!(sc.total_buffer_minted().get(), BigUint::from(10_500u64));
            assert_eq!(sc.total_buffer_burned().get(), BigUint::zero());
        });

    world
        .check_account(SC_ADDRESS)
        .esdt_balance(BUFFER_TOKEN, BigUint::from(10_500u64));
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
            "replenishment exceeds 10% cumulative threshold \u{2014} governance approval required",
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
fn buffer_pool_replenish_non_governance_cumulative_threshold_fails_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

    world.current_block().block_epoch(1_500u64);

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

    world.current_block().block_epoch(3_000u64);

    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "replenishment exceeds 10% cumulative threshold \u{2014} governance approval required",
        ))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(600u64),
                ManagedBuffer::from(b"bafyjustification002"),
            );
        });

    world.current_block().block_epoch(4_500u64);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(600u64),
                ManagedBuffer::from(b"bafygovernancejustification"),
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
            assert_eq!(record.total_replenished, BigUint::from(1_100u64));
            assert_eq!(sc.get_total_pool_balance(), BigUint::from(11_100u64));
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

    // First replenishment at epoch 0 is still rate-limited from the record's
    // initial epoch and must wait for the configured cooldown.
    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "replenishment rate limit: 1 per 90 days per project",
        ))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(500u64),
                ManagedBuffer::from(b"bafyjust-cooldown-early"),
            );
        });

    world.current_block().block_epoch(1_500u64);

    // First replenishment after cooldown should succeed.
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
        .returns(ExpectError(
            4u64,
            "replenishment rate limit: 1 per 90 days per project",
        ))
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

#[test]
fn buffer_pool_governance_pause_blocks_deposit_and_replenish_rs() {
    let mut world = world();
    deploy_and_deposit(&mut world);

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
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-buffer-001"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-buffer-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-buffer-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-buffer-001"));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC.to_managed_address());
        });

    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.deposit_buffer_credits(
                ManagedBuffer::from(b"project-paused-001"),
                BigUint::from(100u64),
                1u64,
            );
        });

    world
        .tx()
        .from(CARBON_CREDIT)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"project-010"),
                BigUint::from(100u64),
                ManagedBuffer::from(b"paused-replenish"),
            );
        });
}
