use multiversx_sc_scenario::executor::debug::ContractContainer;
use multiversx_sc_scenario::imports::*;
use multiversx_sc_scenario::multiversx_sc::contract_base::CallableContractBuilder;

use drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const NEW_GOVERNANCE: TestAddress = TestAddress::new("new_governance");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-policy-registry-gas");
const CODE_PATH: MxscPath = MxscPath::new("output/drwa-policy-registry.mxsc.json");
const TOKEN_ID: &[u8] = b"CARBON-ab12cd";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new()
        .executor_config(ExecutorConfig::Experimental)
        .gas_schedule(GasScheduleVersion::V8);
    blockchain.set_current_dir_from_workspace("contracts/drwa/policy-registry");
    blockchain.register_contract(CODE_PATH, drwa_policy_registry::ContractBuilder);
    blockchain
}

fn world_with_panic_messages() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new()
        .executor_config(ExecutorConfig::Experimental)
        .gas_schedule(GasScheduleVersion::V8);
    blockchain.set_current_dir_from_workspace("contracts/drwa/policy-registry");
    blockchain.register_contract_container(
        CODE_PATH,
        ContractContainer::new(
            drwa_policy_registry::ContractBuilder.new_contract_obj::<DebugApi>(),
            None,
            true,
        ),
    );
    blockchain
}

#[test]
#[ignore = "requires experimental gas executor"]
fn policy_registry_gas_smoke_for_deploy_and_set_policy() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(NEW_GOVERNANCE).nonce(1).balance(1_000_000u64);

    let deploy_gas = world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .returns(ReturnsGasUsed)
        .run();
    assert!(deploy_gas > 0);

    let set_policy_gas = world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            false,
            false,
            true,
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
        )
        .returns(ReturnsGasUsed)
        .run();
    assert!(set_policy_gas > 0);
}

#[test]
#[ignore = "diagnostic helper for experimental executor runtime failures"]
fn policy_registry_gas_smoke_diagnostics() {
    let mut world = world_with_panic_messages();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    let (deploy_status, deploy_message) = world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();

    println!("deploy_status={deploy_status} deploy_message={deploy_message}");

    if deploy_status != 0 {
        return;
    }

    let governance: ManagedAddress<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .governance()
        .returns(ReturnsResult)
        .run();
    println!("queried_governance={governance:?}");

    let (set_governance_status, set_governance_message) = world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_governance(NEW_GOVERNANCE)
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();
    println!(
        "set_governance_status={set_governance_status} set_governance_message={set_governance_message}"
    );

    let (set_policy_status, set_policy_message) = world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaPolicyRegistryProxy)
        .set_token_policy(
            ManagedBuffer::from(TOKEN_ID),
            true,
            false,
            false,
            true,
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
            ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
        )
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();

    println!("set_policy_status={set_policy_status} set_policy_message={set_policy_message}");

    let valid_cid =
        ManagedBuffer::<StaticApi>::from(b"Qm123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijk");
    let (set_cid_status, set_cid_message) = world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .raw_call("setWhitePaperCid")
        .argument(&ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .argument(&valid_cid)
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();

    println!("set_cid_status={set_cid_status} set_cid_message={set_cid_message}");

    let (set_registration_status, set_registration_message) = world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .raw_call("setRegistrationStatus")
        .argument(&ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .argument(&ManagedBuffer::<StaticApi>::from(b"approved"))
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();

    println!(
        "set_registration_status={set_registration_status} set_registration_message={set_registration_message}"
    );
}
