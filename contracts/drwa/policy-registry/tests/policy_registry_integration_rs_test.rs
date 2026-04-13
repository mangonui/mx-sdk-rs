use drwa_common::DrwaCallerDomain;
use drwa_policy_registry::DrwaPolicyRegistry;
use multiversx_sc::types::{ManagedBuffer, ManagedVec};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const OTHER: TestAddress = TestAddress::new("other");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-policy-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-policy-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/policy-registry");
    blockchain.register_contract(CODE_PATH, drwa_policy_registry::ContractBuilder);

    blockchain
}

#[test]
fn policy_registry_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut investor_classes = ManagedVec::new();
            investor_classes.push(ManagedBuffer::from(b"ACCREDITED"));

            let mut jurisdictions = ManagedVec::new();
            jurisdictions.push(ManagedBuffer::from(b"SG"));

            let envelope = sc.set_token_policy(
                ManagedBuffer::from(b"CARBON-ab12cd"),
                true,
                false,
                true,
                true,
                investor_classes,
                jurisdictions,
            );

            assert!(envelope.caller_domain == DrwaCallerDomain::PolicyRegistry);
            assert_eq!(envelope.operations.len(), 1);
            assert!(!envelope.payload_hash.is_empty());
        });
}

#[test]
fn policy_registry_denial_signals_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut investor_classes = ManagedVec::new();
            investor_classes.push(ManagedBuffer::from(b"ACCREDITED"));

            let mut jurisdictions = ManagedVec::new();
            jurisdictions.push(ManagedBuffer::from(b"SG"));

            let envelope = sc.set_token_policy(
                ManagedBuffer::from(b"CARBON-bc23de"),
                true,
                true,
                true,
                true,
                investor_classes,
                jurisdictions,
            );

            assert_eq!(envelope.operations.get(0).version, 1);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(b"CARBON-bc23de");
            let policy = sc.token_policy(&token_id).get();
            assert!(policy.global_pause);
            assert!(policy.strict_auditor_mode);
            assert!(policy.metadata_protection_enabled);
        });
}
