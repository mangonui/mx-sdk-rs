use drwa_asset_manager::DrwaAssetManager;
use drwa_common::DrwaCallerDomain;
use drwa_policy_registry::DrwaPolicyRegistry;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const HOLDER: TestAddress = TestAddress::new("holder");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-asset-manager");
const POLICY_SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-policy-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-asset-manager.mxsc.json");
const POLICY_CODE_PATH: MxscPath =
    MxscPath::new("mxsc:../policy-registry/output/drwa-policy-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/asset-manager");
    blockchain.register_contract(CODE_PATH, drwa_asset_manager::ContractBuilder);
    blockchain.register_contract(POLICY_CODE_PATH, drwa_policy_registry::ContractBuilder);

    blockchain
}

#[test]
fn asset_manager_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(POLICY_CODE_PATH)
        .new_address(POLICY_SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.set_policy_registry_address(POLICY_SC_ADDRESS.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(POLICY_SC_ADDRESS).whitebox(
        drwa_policy_registry::contract_obj,
        |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(b"HOTEL-ab12cd"),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
        },
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(b"HOTEL-ab12cd"),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );

            let envelope = sc.sync_holder_compliance(
                ManagedBuffer::from(b"HOTEL-ab12cd"),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                250,
                false,
                false,
                false,
            );

            assert!(envelope.caller_domain == DrwaCallerDomain::AssetManager);
            assert_eq!(envelope.operations.len(), 1);
            assert!(!envelope.payload_hash.is_empty());
        });
}

#[test]
fn asset_manager_denial_signals_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(POLICY_CODE_PATH)
        .new_address(POLICY_SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.set_policy_registry_address(POLICY_SC_ADDRESS.to_managed_address());
        });

    world.tx().from(GOVERNANCE).to(POLICY_SC_ADDRESS).whitebox(
        drwa_policy_registry::contract_obj,
        |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(b"HOTEL-bc23de"),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
        },
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(b"HOTEL-bc23de"),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-bc23de"),
            );

            let envelope = sc.sync_holder_compliance(
                ManagedBuffer::from(b"HOTEL-bc23de"),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"blocked"),
                ManagedBuffer::from(b"retail"),
                ManagedBuffer::from(b"US"),
                500,
                true,
                true,
                false,
            );

            assert_eq!(envelope.operations.get(0).version, 1);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(b"HOTEL-bc23de");
            let mirror = sc
                .holder_mirror(&token_id, &HOLDER.to_managed_address())
                .get();
            assert_eq!(mirror.aml_status, ManagedBuffer::from(b"blocked"));
            assert!(mirror.transfer_locked);
            assert!(mirror.receive_locked);
            assert!(!mirror.auditor_authorized);
        });
}
