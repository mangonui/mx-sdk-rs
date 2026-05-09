use drwa_auth_admin::DrwaAuthAdmin;
use drwa_common::{DrwaCallerDomain, DrwaSyncOperationType};
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
const AUTH_ADMIN_HEX_V2: &[u8] =
    b"abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
const AUTH_ADMIN_BECH32: &[u8] = b"erd1qqqqqqqqqqqqqpgqf97pgqdy0tstwauxu09kszz020hp5kgqqzzsscqtww";

// B-03 (AUD-003): test scaffolding must honor the procedure-floor 3-of-5
// and the mandatory 24h/48h timelocks. We pick a TTL (40_000 rounds)
// long enough that the 28_800-round recovery-admin timelock window
// fits inside it with margin, and a post-24h-timelock round constant
// (14_401) that most tests can advance to before calling
// `perform_action`. Tests that exercise the recovery-admin 48h window
// advance to 28_801 explicitly.
const TEST_INIT_QUORUM: usize = 3;
const TEST_INIT_TTL_ROUNDS: u64 = 40_000;
const TEST_POST_TIMELOCK_ROUND: u64 = 14_401;

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/drwa/drwa-auth-admin");
    world.register_contract(CODE_PATH, drwa_auth_admin::ContractBuilder);
    world
}

fn deploy(world: &mut ScenarioWorld) {
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
            sc.init(TEST_INIT_QUORUM, TEST_INIT_TTL_ROUNDS, signers.into());
        });
}

/// B-03: reach the 3-of-5 quorum by signing with SIGNER_TWO and
/// SIGNER_THREE (SIGNER_ONE is already counted via propose-time
/// auto-approval). Tests that need different signer ordering call
/// `sc.sign(...)` directly instead of using this helper.
fn sign_to_reach_quorum(world: &mut ScenarioWorld, action_id: u64) {
    world
        .tx()
        .from(SIGNER_TWO)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(action_id));
    world
        .tx()
        .from(SIGNER_THREE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(action_id));
}

#[test]
fn drwa_auth_admin_update_caller_flow() {
    let mut world = world();
    deploy(&mut world);

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
            assert_eq!(action_id, 1);
        });

    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let result = sc.perform_action(action_id);
            match result {
                OptionalValue::Some(envelope) => {
                    assert!(envelope.caller_domain == DrwaCallerDomain::AuthAdmin);
                    assert_eq!(envelope.operations.len(), 1);
                    let operation = envelope.operations.get(0);
                    assert!(
                        operation.operation_type == DrwaSyncOperationType::AuthorizedCallerUpdate
                    );
                    assert_eq!(operation.version, 1);
                    assert_eq!(operation.token_id, ManagedBuffer::from(AUTH_ADMIN_DOMAIN));
                    assert_eq!(operation.body, ManagedBuffer::from(AUTH_ADMIN_HEX_V1));
                }
                OptionalValue::None => panic!("expected sync envelope"),
            }
        });

    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let domain = ManagedBuffer::from(AUTH_ADMIN_DOMAIN);
            assert_eq!(
                sc.authorized_caller(&domain).get(),
                ManagedBuffer::from(AUTH_ADMIN_HEX_V1)
            );
            assert_eq!(sc.authorized_caller_version(&domain).get(), 1);
            assert!(sc.performed_action_ids().contains(&action_id));
        });
}

#[test]
fn drwa_auth_admin_accepts_bech32_caller_address() {
    let mut world = world();
    deploy(&mut world);

    let mut action_id = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            action_id = sc.propose_update_caller_address(
                ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                ManagedBuffer::from(AUTH_ADMIN_BECH32),
            );
        });

    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let result = sc.perform_action(action_id);
            match result {
                OptionalValue::Some(envelope) => {
                    let operation = envelope.operations.get(0);
                    assert_eq!(operation.body, ManagedBuffer::from(AUTH_ADMIN_BECH32));
                }
                OptionalValue::None => panic!("expected sync envelope"),
            }
        });
}

#[test]
fn drwa_auth_admin_rejects_invalid_caller_address_format() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(
            4u64,
            "new address must be a 64-char hex string or erd1 bech32 address",
        ))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.propose_update_caller_address(
                ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                ManagedBuffer::from(b"not-an-address"),
            );
        });
}

#[test]
fn drwa_auth_admin_rejects_short_bech32_caller_address() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(
            4u64,
            "new address must be a 64-char hex string or erd1 bech32 address",
        ))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.propose_update_caller_address(
                ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                ManagedBuffer::from(b"erd1short"),
            );
        });
}

#[test]
fn drwa_auth_admin_rejects_bech32_caller_address_with_invalid_charset() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(
            4u64,
            "new address must be a 64-char hex string or erd1 bech32 address",
        ))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.propose_update_caller_address(
                ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                ManagedBuffer::from(
                    b"erd1qqqqqqqqqqqqqpgqf97pgqdy0tstwauxu09kszz020hp5kgqqzzsscqtwi",
                ),
            );
        });
}

#[test]
fn drwa_auth_admin_rejects_bech32_caller_address_with_bad_checksum() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(
            4u64,
            "new address must be a 64-char hex string or erd1 bech32 address",
        ))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.propose_update_caller_address(
                ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                ManagedBuffer::from(
                    b"erd1qqqqqqqqqqqqqpgqf97pgqdy0tstwauxu09kszz020hp5kgqqzzsscqtwr",
                ),
            );
        });
}

#[test]
fn drwa_auth_admin_rejects_duplicate_signer_on_init() {
    let mut world = world();
    world.account(OWNER).nonce(1);
    world.account(SIGNER_ONE).nonce(1);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(ADMIN_SC)
        .returns(ExpectError(4u64, "duplicate signer"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let mut signers = ManagedVec::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_ONE.to_managed_address());
            sc.init(1, 100, signers.into());
        });
}

#[test]
fn drwa_auth_admin_rejects_expired_action() {
    let mut world = world();
    deploy(&mut world);

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
    sign_to_reach_quorum(&mut world, action_id);
    // B-03: TTL is now TEST_INIT_TTL_ROUNDS (20_000); advance past it to
    // drive the expired-action branch.
    world.current_block().block_round(TEST_INIT_TTL_ROUNDS + 1);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "action expired"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(action_id);
        });
}

#[test]
fn drwa_auth_admin_change_quorum_guards() {
    let mut world = world();
    deploy(&mut world);

    let mut action_id = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            // B-03: test invariant "quorum exceeds signer count". With
            // 5 signers under the procedure floor, propose quorum=6 to
            // trigger the guard. Previously this test used quorum=4
            // under a 3-signer configuration.
            action_id = sc.propose_change_quorum(6);
        });
    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "quorum exceeds signer count"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(action_id);
        });
}

#[test]
fn drwa_auth_admin_remove_signer_cannot_break_quorum() {
    let mut world = world();
    deploy(&mut world);

    // B-03: with 5 signers and the new procedure floor, "below quorum"
    // is reached by (a) first raising quorum to 5 (the maximum with
    // current signer count), then (b) trying to remove one signer.
    // Removing would leave 4 < quorum=5, which triggers
    // "cannot remove signer below quorum".
    let mut change_quorum_action = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            change_quorum_action = sc.propose_change_quorum(5);
        });
    // Reach quorum=3 with SIGNER_TWO + SIGNER_THREE.
    sign_to_reach_quorum(&mut world, change_quorum_action);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(change_quorum_action);
        });

    // Quorum is now 5. Propose removing SIGNER_FIVE — leaves 4 signers
    // which is below the new quorum of 5.
    let mut remove_action = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            remove_action = sc.propose_remove_signer(SIGNER_FIVE.to_managed_address());
        });
    // Quorum is now 5 — need SIGNER_TWO, THREE, FOUR, FIVE to sign
    // (plus SIGNER_ONE auto-approval) to hit threshold. Advance block
    // for timelock afterwards.
    for signer in [SIGNER_TWO, SIGNER_THREE, SIGNER_FOUR, SIGNER_FIVE] {
        world
            .tx()
            .from(signer)
            .to(ADMIN_SC)
            .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(remove_action));
    }
    world
        .current_block()
        .block_round(TEST_POST_TIMELOCK_ROUND + TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "cannot remove signer below quorum"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(remove_action);
        });
}

#[test]
fn drwa_auth_admin_versions_increment() {
    let mut world = world();
    deploy(&mut world);

    for (iteration, (expected_version, payload)) in
        [(1u64, AUTH_ADMIN_HEX_V1), (2u64, AUTH_ADMIN_HEX_V2)]
            .iter()
            .enumerate()
    {
        let mut action_id = 0u64;
        world
            .tx()
            .from(SIGNER_ONE)
            .to(ADMIN_SC)
            .whitebox(drwa_auth_admin::contract_obj, |sc| {
                action_id = sc.propose_update_caller_address(
                    ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                    ManagedBuffer::from(*payload),
                );
            });
        sign_to_reach_quorum(&mut world, action_id);
        // B-03: each iteration must re-advance the block round past the
        // previous timelock window so the next action's own timelock
        // has time to elapse.
        world
            .current_block()
            .block_round(TEST_POST_TIMELOCK_ROUND + (iteration as u64) * TEST_POST_TIMELOCK_ROUND);
        world
            .tx()
            .from(SIGNER_ONE)
            .to(ADMIN_SC)
            .whitebox(drwa_auth_admin::contract_obj, |sc| {
                let result = sc.perform_action(action_id);
                match result {
                    OptionalValue::Some(envelope) => {
                        assert_eq!(envelope.operations.get(0).version, *expected_version);
                    }
                    OptionalValue::None => panic!("expected sync envelope"),
                }
            });
    }
}

#[test]
fn drwa_auth_admin_rejects_non_signer_proposal() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "caller not a signer"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.propose_update_caller_address(
                ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                ManagedBuffer::from(AUTH_ADMIN_HEX_V1),
            );
        });
}

#[test]
fn drwa_auth_admin_rejects_perform_without_quorum() {
    let mut world = world();
    deploy(&mut world);

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
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "insufficient approvals"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(action_id);
        });
}

#[test]
fn drwa_auth_admin_add_signer_flow() {
    let mut world = world();
    deploy(&mut world);

    const NEW_SIGNER: TestAddress = TestAddress::new("new-signer");
    world.account(NEW_SIGNER).nonce(1);

    let mut action_id = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            action_id = sc.propose_add_signer(NEW_SIGNER.to_managed_address());
        });
    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let result = sc.perform_action(action_id);
            assert!(matches!(result, OptionalValue::None));
        });

    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            assert!(sc.signers().contains(&NEW_SIGNER.to_managed_address()));
        });
}

#[test]
fn drwa_auth_admin_replace_signer_flow() {
    let mut world = world();
    deploy(&mut world);

    const REPLACEMENT_SIGNER: TestAddress = TestAddress::new("replacement-signer");
    world.account(REPLACEMENT_SIGNER).nonce(1);

    let mut action_id = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            action_id = sc.propose_replace_signer(
                SIGNER_THREE.to_managed_address(),
                REPLACEMENT_SIGNER.to_managed_address(),
            );
        });
    // B-03: need quorum=3; SIGNER_THREE is the proposed replaced signer
    // but can still sign (they remain a signer until perform_action
    // actually executes the removal).
    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let result = sc.perform_action(action_id);
            assert!(matches!(result, OptionalValue::None));
        });

    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            assert!(!sc.signers().contains(&SIGNER_THREE.to_managed_address()));
            assert!(
                sc.signers()
                    .contains(&REPLACEMENT_SIGNER.to_managed_address())
            );
        });
}

#[test]
fn drwa_auth_admin_removed_signer_signature_no_longer_counts() {
    let mut world = world();
    deploy(&mut world);

    let mut guarded_action = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            guarded_action = sc.propose_update_caller_address(
                ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                ManagedBuffer::from(AUTH_ADMIN_HEX_V1),
            );
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(guarded_action));
    world
        .tx()
        .from(SIGNER_FIVE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(guarded_action));

    const REPLACEMENT_FOR_FIVE: TestAddress = TestAddress::new("replacement-for-five");
    world.account(REPLACEMENT_FOR_FIVE).nonce(1);

    let mut replace_action = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            replace_action = sc.propose_replace_signer(
                SIGNER_FIVE.to_managed_address(),
                REPLACEMENT_FOR_FIVE.to_managed_address(),
            );
        });
    sign_to_reach_quorum(&mut world, replace_action);

    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(replace_action);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "insufficient approvals"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(guarded_action);
        });

    world
        .tx()
        .from(SIGNER_THREE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(guarded_action));

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(
            4u64,
            "timelock not elapsed: must wait 24h after quorum (48h for recovery-admin)",
        ))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(guarded_action);
        });
}

#[test]
fn drwa_auth_admin_stale_signer_cleanup_uses_pending_action_index() {
    let mut world = world();
    deploy(&mut world);

    let mut guarded_action = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            guarded_action = sc.propose_update_caller_address(
                ManagedBuffer::from(AUTH_ADMIN_DOMAIN),
                ManagedBuffer::from(AUTH_ADMIN_HEX_V1),
            );
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(guarded_action));
    world
        .tx()
        .from(SIGNER_FIVE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(guarded_action));

    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            assert!(
                sc.signer_pending_action_ids(&SIGNER_FIVE.to_managed_address())
                    .contains(&guarded_action)
            );
        });

    const REPLACEMENT_FOR_FIVE_INDEX: TestAddress = TestAddress::new("replacement-index-five");
    world.account(REPLACEMENT_FOR_FIVE_INDEX).nonce(1);

    let mut replace_action = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            replace_action = sc.propose_replace_signer(
                SIGNER_FIVE.to_managed_address(),
                REPLACEMENT_FOR_FIVE_INDEX.to_managed_address(),
            );
        });
    sign_to_reach_quorum(&mut world, replace_action);

    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(replace_action);
        });

    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            assert!(
                !sc.signer_pending_action_ids(&SIGNER_FIVE.to_managed_address())
                    .contains(&guarded_action)
            );
            assert!(
                !sc.action_signers(guarded_action)
                    .contains(&SIGNER_FIVE.to_managed_address())
            );
            assert!(sc.action_approved_at_round(guarded_action).is_empty());
            assert!(
                !sc.signer_pending_action_ids(&SIGNER_ONE.to_managed_address())
                    .contains(&replace_action)
            );
            assert!(
                !sc.signer_pending_action_ids(&SIGNER_TWO.to_managed_address())
                    .contains(&replace_action)
            );
            assert!(
                !sc.signer_pending_action_ids(&SIGNER_THREE.to_managed_address())
                    .contains(&replace_action)
            );
        });
}

#[test]
fn drwa_auth_admin_upgrade_migrates_pending_action_indexes_and_restarts_timelock() {
    let mut world = world();
    deploy(&mut world);

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
    sign_to_reach_quorum(&mut world, action_id);

    world.current_block().block_round(100);
    world
        .tx()
        .from(OWNER)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            sc.storage_version().clear();
            sc.action_approved_at_round(action_id).clear();
            sc.action_timelock_rounds(action_id).clear();
            sc.signer_pending_action_ids(&SIGNER_ONE.to_managed_address())
                .clear();
            sc.signer_pending_action_ids(&SIGNER_TWO.to_managed_address())
                .clear();
            sc.signer_pending_action_ids(&SIGNER_THREE.to_managed_address())
                .clear();

            let empty_signers = ManagedVec::new();
            sc.upgrade(TEST_INIT_QUORUM, TEST_INIT_TTL_ROUNDS, empty_signers.into());
        });

    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            assert_eq!(sc.storage_version().get(), 2);
            assert_eq!(
                sc.action_timelock_rounds(action_id).get(),
                TEST_POST_TIMELOCK_ROUND - 1
            );
            assert_eq!(sc.action_approved_at_round(action_id).get(), 101);
            assert!(
                sc.signer_pending_action_ids(&SIGNER_ONE.to_managed_address())
                    .contains(&action_id)
            );
            assert!(
                sc.signer_pending_action_ids(&SIGNER_TWO.to_managed_address())
                    .contains(&action_id)
            );
            assert!(
                sc.signer_pending_action_ids(&SIGNER_THREE.to_managed_address())
                    .contains(&action_id)
            );
        });

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

    world.current_block().block_round(14_500);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let result = sc.perform_action(action_id);
            assert!(matches!(result, OptionalValue::Some(_)));
        });
}

#[test]
fn drwa_auth_admin_rejects_replay_perform() {
    let mut world = world();
    deploy(&mut world);

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
    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(action_id);
        });
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "action already performed"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(action_id);
        });
}

#[test]
fn drwa_auth_admin_rejects_add_existing_signer_proposal() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "signer already exists"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.propose_add_signer(SIGNER_TWO.to_managed_address());
        });
}

#[test]
fn drwa_auth_admin_rejects_remove_unknown_signer_proposal() {
    let mut world = world();
    deploy(&mut world);

    const UNKNOWN_SIGNER: TestAddress = TestAddress::new("unknown-signer");
    world.account(UNKNOWN_SIGNER).nonce(1);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "signer not found"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.propose_remove_signer(UNKNOWN_SIGNER.to_managed_address());
        });
}

#[test]
fn drwa_auth_admin_rejects_replace_with_existing_signer() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "new signer already exists"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.propose_replace_signer(
                SIGNER_THREE.to_managed_address(),
                SIGNER_TWO.to_managed_address(),
            );
        });
}

#[test]
fn drwa_auth_admin_rejects_zero_quorum_change_on_execute() {
    let mut world = world();
    deploy(&mut world);

    let mut action_id = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            action_id = sc.propose_change_quorum(0);
        });
    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "quorum must be > 0"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(action_id);
        });
}

#[test]
fn drwa_auth_admin_discard_allows_expired_action() {
    let mut world = world();
    deploy(&mut world);

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
    // B-03: TTL is now TEST_INIT_TTL_ROUNDS; advance past it so the
    // discard path recognizes the action as expired.
    world.current_block().block_round(TEST_INIT_TTL_ROUNDS + 1);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            sc.discard_action(action_id)
        });
    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            assert!(sc.actions(action_id).is_empty());
        });
}

#[test]
fn drwa_auth_admin_rejects_discard_active_action_with_signatures() {
    let mut world = world();
    deploy(&mut world);

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
    world
        .tx()
        .from(SIGNER_TWO)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(action_id));
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "cannot discard active action"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            sc.discard_action(action_id)
        });
}

#[test]
fn drwa_auth_admin_unsign_then_discard_without_expiry() {
    let mut world = world();
    deploy(&mut world);

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
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.unsign(action_id));
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            sc.discard_action(action_id)
        });
    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            assert!(sc.actions(action_id).is_empty());
        });
}

// ── B-03 (AUD-003) timelock & procedure-floor regression tests ───────

#[test]
fn drwa_auth_admin_b03_rejects_perform_before_timelock_elapsed() {
    let mut world = world();
    deploy(&mut world);

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
    sign_to_reach_quorum(&mut world, action_id);

    // Quorum reached at round 0 → minimum execution round = 14_400.
    // Attempt at round 14_399 must fail with the timelock guard.
    world.current_block().block_round(14_399);
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

    world.current_block().block_round(14_400);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let result = sc.perform_action(action_id);
            assert!(matches!(result, OptionalValue::Some(_)));
        });
}

#[test]
fn drwa_auth_admin_c213_exposes_no_emergency_override_policy() {
    let mut world = world();
    deploy(&mut world);

    world
        .query()
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            assert!(!sc.is_emergency_override_supported());
            assert_eq!(
                sc.get_emergency_override_policy(),
                ManagedBuffer::from(
                    b"not_supported: all auth-admin actions require quorum and timelock"
                )
            );
        });
}

#[test]
fn drwa_auth_admin_b03_recovery_admin_domain_uses_48h_timelock() {
    let mut world = world();
    deploy(&mut world);

    let mut action_id = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            action_id = sc.propose_update_caller_address(
                ManagedBuffer::from(b"recovery_admin"),
                ManagedBuffer::from(AUTH_ADMIN_HEX_V1),
            );
        });
    sign_to_reach_quorum(&mut world, action_id);

    // 24h is NOT enough for recovery-admin.
    world.current_block().block_round(14_401);
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

    // 48h window crossed → succeeds.
    world.current_block().block_round(28_801);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let result = sc.perform_action(action_id);
            assert!(matches!(result, OptionalValue::Some(_)));
        });
}

#[test]
fn drwa_auth_admin_b03_unsign_below_quorum_restarts_timelock() {
    let mut world = world();
    deploy(&mut world);

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

    // Reach quorum at round 0.
    world
        .tx()
        .from(SIGNER_TWO)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(action_id));
    world
        .tx()
        .from(SIGNER_THREE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(action_id));

    // SIGNER_TWO retracts — approvals drop to 2 < quorum=3 → storage
    // slot is cleared.
    world
        .tx()
        .from(SIGNER_TWO)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.unsign(action_id));

    // Advance 10 rounds, then re-sign. Approval round is now 10.
    world.current_block().block_round(10);
    world
        .tx()
        .from(SIGNER_TWO)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| sc.sign(action_id));

    // At round 14_409 (10 + 14_399) must still reject.
    world.current_block().block_round(14_409);
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

    // At round 14_410 (10 + 14_400) succeeds.
    world.current_block().block_round(14_410);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let result = sc.perform_action(action_id);
            assert!(matches!(result, OptionalValue::Some(_)));
        });
}

#[test]
fn drwa_auth_admin_b03_change_quorum_rejects_below_floor() {
    // B-03: ChangeQuorum rejects values below the 3-of-5 floor even
    // when the old guards ("quorum > 0" and "quorum <= signer count")
    // would pass.
    let mut world = world();
    deploy(&mut world);

    let mut action_id = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            action_id = sc.propose_change_quorum(2);
        });
    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(4u64, "quorum below procedure floor (3-of-5)"))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(action_id);
        });
}

#[test]
fn drwa_auth_admin_b03_remove_signer_rejects_below_signer_floor() {
    // B-03: RemoveSigner rejects any remove that would drop signer
    // count below DRWA_AUTH_MIN_SIGNER_COUNT.
    let mut world = world();
    deploy(&mut world);

    let mut action_id = 0u64;
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            action_id = sc.propose_remove_signer(SIGNER_FIVE.to_managed_address());
        });
    sign_to_reach_quorum(&mut world, action_id);
    world.current_block().block_round(TEST_POST_TIMELOCK_ROUND);
    world
        .tx()
        .from(SIGNER_ONE)
        .to(ADMIN_SC)
        .returns(ExpectError(
            4u64,
            "cannot drop signer count below procedure floor (3-of-5)",
        ))
        .whitebox(drwa_auth_admin::contract_obj, |sc| {
            let _ = sc.perform_action(action_id);
        });
}
