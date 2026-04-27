use multiversx_sc_scenario::executor::debug::ContractContainer;
use multiversx_sc_scenario::imports::*;
use multiversx_sc_scenario::multiversx_sc::contract_base::CallableContractBuilder;

use drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const ISSUER: TestAddress = TestAddress::new("issuer");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-identity-registry-gas");
const CODE_PATH: MxscPath = MxscPath::new("output/drwa-identity-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new()
        .executor_config(ExecutorConfig::Experimental)
        .gas_schedule(GasScheduleVersion::V8);
    blockchain.set_current_dir_from_workspace("contracts/drwa/identity-registry");
    blockchain.register_contract(CODE_PATH, drwa_identity_registry::ContractBuilder);
    blockchain
}

fn world_with_panic_messages() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new()
        .executor_config(ExecutorConfig::Experimental)
        .gas_schedule(GasScheduleVersion::V8);
    blockchain.set_current_dir_from_workspace("contracts/drwa/identity-registry");
    blockchain.register_contract_container(
        CODE_PATH,
        ContractContainer::new(
            drwa_identity_registry::ContractBuilder.new_contract_obj::<DebugApi>(),
            None,
            true,
        ),
    );
    blockchain
}

#[test]
#[ignore = "requires experimental gas executor"]
fn identity_registry_gas_smoke_for_register_and_update() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(ISSUER).nonce(1).balance(1_000_000u64);

    let deploy_gas = world
        .tx()
        .from(OWNER)
        .typed(DrwaIdentityRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .returns(ReturnsGasUsed)
        .run();
    assert!(deploy_gas > 0);

    let register_gas = world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaIdentityRegistryProxy)
        .register_identity(
            ISSUER.to_managed_address(),
            "Gas Corp",
            "SG",
            "REG-GAS",
            "SPV",
        )
        .returns(ReturnsGasUsed)
        .run();
    assert!(register_gas > 0);

    let update_gas = world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaIdentityRegistryProxy)
        .update_compliance_status(
            ISSUER.to_managed_address(),
            "approved",
            "clear",
            "QIB",
            100u64,
        )
        .returns(ReturnsGasUsed)
        .run();
    assert!(update_gas > 0);
}

#[test]
#[ignore = "diagnostic helper for experimental executor runtime failures"]
fn identity_registry_gas_smoke_diagnostics() {
    let mut world = world_with_panic_messages();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(ISSUER).nonce(1).balance(1_000_000u64);

    let (deploy_status, deploy_message) = world
        .tx()
        .from(OWNER)
        .typed(DrwaIdentityRegistryProxy)
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

    let (register_status, register_message) = world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaIdentityRegistryProxy)
        .register_identity(
            ISSUER.to_managed_address(),
            "Gas Corp",
            "SG",
            "REG-GAS",
            "SPV",
        )
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();
    println!("register_status={register_status} register_message={register_message}");

    let (update_status, update_message) = world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaIdentityRegistryProxy)
        .update_compliance_status(
            ISSUER.to_managed_address(),
            "approved",
            "clear",
            "QIB",
            100u64,
        )
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();
    println!("update_status={update_status} update_message={update_message}");
}
