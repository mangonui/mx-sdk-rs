use drwa_auth_admin::DrwaAuthAdmin;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");
const SIGNER_THREE: TestAddress = TestAddress::new("signer-three");
const SIGNER_FOUR: TestAddress = TestAddress::new("signer-four");
const SIGNER_FIVE: TestAddress = TestAddress::new("signer-five");
const ADMIN_SC: TestSCAddress = TestSCAddress::new("drwa-auth-admin");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-auth-admin.mxsc.json");
const AUTH_ADMIN_DOMAIN: &[u8] = b"auth_admin";
const AUTH_ADMIN_HEX_V1: &[u8] =
    b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// B-03: proposal TTL must exceed the 24-hour timelock (14_400 rounds).
// Use 20_000 to give the test scenarios room to advance the block round
// past the timelock without also tripping the expiry window.
const TEST_TTL_ROUNDS: u64 = 20_000;
const TEST_TIMELOCK_ROUNDS: u64 = 14_400;

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/drwa/drwa-auth-admin");
    world.register_contract(CODE_PATH, drwa_auth_admin::ContractBuilder);
    world
}

fn deploy_with_quorum(world: &mut ScenarioWorld, quorum: usize) {
    world.account(OWNER).nonce(1);
    world.account(SIGNER_ONE).nonce(1);
    world.account(SIGNER_TWO).nonce(1);
    world.account(SIGNER_THREE).nonce(1);
    world.account(SIGNER_FOUR).nonce(1);
    world.account(SIGNER_FIVE).nonce(1);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let mut signers = ManagedVec::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            signers.push(SIGNER_THREE.to_managed_address());
            signers.push(SIGNER_FOUR.to_managed_address());
            signers.push(SIGNER_FIVE.to_managed_address());
            sc.init(quorum, TEST_TTL_ROUNDS, signers.into());
        });
}

#[test]
fn drwa_auth_admin_quorum_threshold_property() {
    // B-03 (AUD-003): the procedure floor is 3-of-5, so we exercise the
    // valid quorum range {3, 4, 5}. Configurations below 3 are now
    // rejected at `init`; that path is covered by
    // `drwa_auth_admin_rejects_init_below_procedure_floor` below.
    let signers_in_order: [&TestAddress; 5] = [
        &SIGNER_ONE,
        &SIGNER_TWO,
        &SIGNER_THREE,
        &SIGNER_FOUR,
        &SIGNER_FIVE,
    ];
    for quorum in 3..=5 {
        let mut world = world();
        deploy_with_quorum(&mut world, quorum);

        let mut action_id = 0u64;
        world
            .tx()
            .from(SIGNER_ONE)
            .to(ADMIN_SC)
            .whitebox(drwa_auth_admin::contract_obj, |sc| {
                action_id = sc.propose_update_caller_address(
                    ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                    ManagedBuffer::from(AUTH_ADMIN_HEX_V1),
                );
            });

        // Collect enough unique approvals to reach the exact quorum.
        // SIGNER_ONE already counts as the proposer's approval, so we
        // add SIGNER_TWO .. SIGNER_{quorum}.
        for signer_slot in signers_in_order.iter().take(quorum).skip(1) {
            world
                .tx()
                .from(**signer_slot)
                .to(ADMIN_SC)
                .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(action_id));
        }

        // B-03: executing before the timelock must be rejected.
        world
            .tx()
            .from(SIGNER_ONE)
            .to(ADMIN_SC)
            .returns(ExpectError(
                4u64,
                "timelock not elapsed: must wait 24h after quorum (48h for recovery-admin)",
            ))
            .whitebox(drwa_auth_admin::contract_obj, |sc| {
                let _ = sc.perform_action(action_id);
            });

        // Advance past the 24h timelock window and execute.
        world.current_block().block_round(TEST_TIMELOCK_ROUNDS + 1);

        world
            .tx()
            .from(SIGNER_ONE)
            .to(ADMIN_SC)
            .whitebox(drwa_auth_admin::contract_obj, |sc| {
                let result = sc.perform_action(action_id);
                assert!(result.is_some());
            });

        world
            .query()
            .to(ADMIN_SC)
            .whitebox(drwa_auth_admin::contract_obj, |sc| {
                let domain = ManagedBuffer::from(AUTH_ADMIN_DOMAIN);
                assert_eq!(sc.authorized_caller_version(&domain).get(), 1);
            });
    }
}

#[test]
fn drwa_auth_admin_rejects_init_below_procedure_floor_signers() {
    // B-03: init must reject any deployment that cannot honor the 3-of-5
    // promise in DRWA-Key-Rotation-Procedures.md — too few signers.
    let mut w = world();
    w.account(OWNER).nonce(1);
    w.account(SIGNER_ONE).nonce(1);
    w.account(SIGNER_TWO).nonce(1);
    w.account(SIGNER_THREE).nonce(1);
    w.account(SIGNER_FOUR).nonce(1);

    w.tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(ADMIN_SC)
        .returns(ExpectError(
            4u64,
            "signer count below procedure floor (3-of-5)",
        ))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let mut signers = ManagedVec::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            signers.push(SIGNER_THREE.to_managed_address());
            signers.push(SIGNER_FOUR.to_managed_address());
            sc.init(3, TEST_TTL_ROUNDS, signers.into());
        });
}

#[test]
fn drwa_auth_admin_rejects_init_below_procedure_floor_quorum() {
    // B-03: init must reject any deployment with quorum below 3 even
    // when the signer count satisfies the floor.
    let mut w = world();
    w.account(OWNER).nonce(1);
    w.account(SIGNER_ONE).nonce(1);
    w.account(SIGNER_TWO).nonce(1);
    w.account(SIGNER_THREE).nonce(1);
    w.account(SIGNER_FOUR).nonce(1);
    w.account(SIGNER_FIVE).nonce(1);

    w.tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(ADMIN_SC)
        .returns(ExpectError(4u64, "quorum below procedure floor (3-of-5)"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let mut signers = ManagedVec::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            signers.push(SIGNER_THREE.to_managed_address());
            signers.push(SIGNER_FOUR.to_managed_address());
            signers.push(SIGNER_FIVE.to_managed_address());
            sc.init(2, TEST_TTL_ROUNDS, signers.into());
        });
}
