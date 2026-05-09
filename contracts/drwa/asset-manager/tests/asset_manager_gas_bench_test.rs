use multiversx_sc_scenario::executor::debug::ContractContainer;
use multiversx_sc_scenario::imports::*;
use multiversx_sc_scenario::multiversx_sc::contract_base::CallableContractBuilder;

use drwa_asset_manager::drwa_asset_manager_proxy::DrwaAssetManagerProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const HOLDER: TestAddress = TestAddress::new("holder");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-asset-manager-gas");
const CODE_PATH: MxscPath = MxscPath::new("output/drwa-asset-manager.mxsc.json");
const TOKEN_ID: &[u8] = b"HOTEL-ab12cd";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new()
        .executor_config(ExecutorConfig::Experimental)
        .gas_schedule(GasScheduleVersion::V8);
    blockchain.set_current_dir_from_workspace("contracts/drwa/asset-manager");
    blockchain.register_contract(CODE_PATH, drwa_asset_manager::ContractBuilder);
    blockchain
}

fn world_with_panic_messages() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new()
        .executor_config(ExecutorConfig::Experimental)
        .gas_schedule(GasScheduleVersion::V8);
    blockchain.set_current_dir_from_workspace("contracts/drwa/asset-manager");
    blockchain.register_contract_container(
        CODE_PATH,
        ContractContainer::new(
            drwa_asset_manager::ContractBuilder.new_contract_obj::<DebugApi>(),
            None,
            true,
        ),
    );
    blockchain
}

#[test]
#[ignore = "requires experimental gas executor"]
fn asset_manager_gas_smoke_for_register_and_sync() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(HOLDER).nonce(1).balance(1_000_000u64);

    let deploy_gas = world
        .tx()
        .from(OWNER)
        .typed(DrwaAssetManagerProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .returns(ReturnsGasUsed)
        .run();
    assert!(deploy_gas > 0);

    let register_gas = world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"policy-gas"),
        )
        .returns(ReturnsGasUsed)
        .run();
    assert!(register_gas > 0);

    let sync_gas = world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .sync_holder_compliance(
            ManagedBuffer::from(TOKEN_ID),
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"approved"),
            ManagedBuffer::from(b"clear"),
            ManagedBuffer::from(b"accredited"),
            ManagedBuffer::from(b"SG"),
            500u64,
            false,
            false,
            false,
        )
        .returns(ReturnsGasUsed)
        .run();
    assert!(sync_gas > 0);
}

#[test]
#[ignore = "diagnostic helper for experimental executor runtime failures"]
fn asset_manager_gas_smoke_diagnostics() {
    let mut world = world_with_panic_messages();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(HOLDER).nonce(1).balance(1_000_000u64);

    let (deploy_status, deploy_message) = world
        .tx()
        .from(OWNER)
        .typed(DrwaAssetManagerProxy)
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
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"policy-gas"),
        )
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();
    println!("register_status={register_status} register_message={register_message}");

    let (sync_status, sync_message) = world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .sync_holder_compliance(
            ManagedBuffer::from(TOKEN_ID),
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"approved"),
            ManagedBuffer::from(b"clear"),
            ManagedBuffer::from(b"accredited"),
            ManagedBuffer::from(b"SG"),
            500u64,
            false,
            false,
            false,
        )
        .returns(ReturnsStatus)
        .returns(ReturnsMessage)
        .run();
    println!("sync_status={sync_status} sync_message={sync_message}");
}
