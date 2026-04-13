use multiversx_sc_scenario::imports::*;

use drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const NEW_GOVERNANCE: TestAddress = TestAddress::new("new_governance");
const OTHER: TestAddress = TestAddress::new("other");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-policy-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-policy-registry.mxsc.json");
const TOKEN_ID: &[u8] = b"CARBON-ab12cd";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/policy-registry");
    blockchain.register_contract(CODE_PATH, drwa_policy_registry::ContractBuilder);
    blockchain
}

/// Deploy the policy-registry, set a token policy via typed proxy, query back
/// and verify all persisted fields match.
#[test]
fn policy_registry_blackbox_deploy_and_set_policy() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    // Verify governance was set correctly
    let gov: ManagedAddress<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .governance()
        .returns(ReturnsResult)
        .run();
    assert_eq!(gov, GOVERNANCE.to_managed_address());

    // Set a token policy from the owner (owner is authorized via require_governance_or_owner)
    let mut investor_classes: ManagedVec<StaticApi, ManagedBuffer<StaticApi>> = ManagedVec::new();
    investor_classes.push(ManagedBuffer::from(b"ACCREDITED"));
    investor_classes.push(ManagedBuffer::from(b"QUALIFIED"));

    let mut jurisdictions: ManagedVec<StaticApi, ManagedBuffer<StaticApi>> = ManagedVec::new();
    jurisdictions.push(ManagedBuffer::from(b"SG"));
    jurisdictions.push(ManagedBuffer::from(b"US"));

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,  // drwa_enabled
            false, // global_pause
            true,  // strict_auditor_mode
            true,  // metadata_protection_enabled
            investor_classes,
            jurisdictions,
        )
        .run();

    // Query back the token policy version
    let version: u64 = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy_version(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert_eq!(version, 1u64);

    // Query back the full token policy struct
    let policy: drwa_common::DrwaTokenPolicy<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();

    assert!(policy.drwa_enabled);
    assert!(!policy.global_pause);
    assert!(policy.strict_auditor_mode);
    assert!(policy.metadata_protection_enabled);
    assert_eq!(policy.token_policy_version, 1u64);
    assert_eq!(policy.allowed_investor_classes.len(), 2);
    assert_eq!(policy.allowed_jurisdictions.len(), 2);
}

/// Deploy, then try to set a token policy from an unauthorized address.
/// The call must revert with "caller not authorized".
#[test]
fn policy_registry_blackbox_non_owner_rejected() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    // Attempt set_token_policy from OTHER (neither owner nor governance)
    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            false,
            false,
            false,
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
        )
        .with_result(ExpectError(4, "caller not authorized"))
        .run();
}

/// Deploy, set policy twice for the same token, verify the version incremented
/// from 1 to 2.
#[test]
fn policy_registry_blackbox_version_increments() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    let empty_classes = ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new();
    let empty_jurisdictions = ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new();

    // First set_token_policy
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            false,
            false,
            false,
            empty_classes.clone(),
            empty_jurisdictions.clone(),
        )
        .run();

    let v1: u64 = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy_version(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert_eq!(v1, 1u64);

    // Second set_token_policy — same token, different flags
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            true, // global_pause now true
            false,
            false,
            empty_classes,
            empty_jurisdictions,
        )
        .run();

    let v2: u64 = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy_version(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert_eq!(v2, 2u64);

    // Verify the updated policy reflects the second call
    let policy: drwa_common::DrwaTokenPolicy<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert!(policy.global_pause);
    assert_eq!(policy.token_policy_version, 2u64);
}

/// Deploy, owner proposes new governance, new governance accepts, then new
/// governance successfully sets a token policy.
#[test]
fn policy_registry_blackbox_governance_handoff() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(NEW_GOVERNANCE).nonce(1).balance(1_000_000u64);

    // Deploy with initial governance
    world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    // Owner proposes NEW_GOVERNANCE (setGovernance is #[only_owner])
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_governance(NEW_GOVERNANCE)
        .run();

    // Verify pending governance is set
    let pending: ManagedAddress<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .pending_governance()
        .returns(ReturnsResult)
        .run();
    assert_eq!(pending, NEW_GOVERNANCE.to_managed_address());

    // Active governance should still be the original
    let active: ManagedAddress<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .governance()
        .returns(ReturnsResult)
        .run();
    assert_eq!(active, GOVERNANCE.to_managed_address());

    // NEW_GOVERNANCE accepts
    world
        .tx()
        .from(NEW_GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .accept_governance()
        .run();

    // Active governance should now be NEW_GOVERNANCE
    let active_after: ManagedAddress<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .governance()
        .returns(ReturnsResult)
        .run();
    assert_eq!(active_after, NEW_GOVERNANCE.to_managed_address());

    // NEW_GOVERNANCE can now set a token policy
    world
        .tx()
        .from(NEW_GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            false,
            true,
            false,
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
        )
        .run();

    // Verify the policy was persisted
    let version: u64 = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .token_policy_version(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();
    assert_eq!(version, 1u64);
}
