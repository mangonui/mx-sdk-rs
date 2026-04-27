use multiversx_sc_scenario::imports::*;

use drwa_policy_registry::DrwaPolicyRegistry;
use drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-policy-registry-upgrade");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-policy-registry.mxsc.json");
const TOKEN_ID: &[u8] = b"CARBON-ab12cd";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/policy-registry");
    blockchain.register_contract(CODE_PATH, drwa_policy_registry::ContractBuilder);
    blockchain
}

#[test]
fn policy_registry_upgrade_preserves_policy_and_storage_version() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            false,
            true,
            true,
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
        )
        .run();

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .upgrade()
        .code(CODE_PATH)
        .run();

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            assert_eq!(sc.storage_version().get(), 1);

            let policy = sc.token_policy(&ManagedBuffer::from(TOKEN_ID)).get();
            assert!(policy.drwa_enabled);
            assert!(policy.strict_auditor_mode);
            assert!(policy.metadata_protection_enabled);
            assert_eq!(policy.token_policy_version, 1);
        });
}
