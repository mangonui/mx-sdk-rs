use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const VERIFIER: TestAddress = TestAddress::new("verifier");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("gsoc-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-gsoc-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/gsoc-registry");
    world.register_contract(CODE_PATH, mrv_gsoc_registry::ContractBuilder);
    world
}

#[test]
fn gsoc_registry_reserve_register_retire_lifecycle_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(0u64);
    world.account(VERIFIER).nonce(1).balance(0u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .argument(&GOVERNANCE)
        .run();

    let serial = "KE-DH-SOC-00001";

    // Reserve
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .reserve_serial(serial)
        .run();

    // Verify reserved
    world
        .query()
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .is_serial_reserved(serial)
        .returns(ExpectValue(true))
        .run();

    // Register batch
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .register_serial_batch(
            serial,
            "PROJ-001",
            2026u32,
            "KE-DH-SOC-00001",
            "KE-DH-SOC-00001",
            1u64,
        )
        .run();

    // Authorize retirement verifier
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .add_verifier(VERIFIER)
        .run();

    // Retire
    world
        .tx()
        .from(VERIFIER)
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .record_retirement(serial, "Acme Corp", OWNER, "tx:0xabc")
        .run();

    // Verify retired
    world
        .query()
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .is_serial_retired(serial)
        .returns(ExpectValue(true))
        .run();
}

#[test]
fn gsoc_registry_cancel_reservation_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(0u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .argument(&GOVERNANCE)
        .run();

    let serial = "KE-DH-CAN-00001";

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .reserve_serial(serial)
        .run();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .cancel_reservation(serial)
        .run();

    // After cancellation, serial should not be reserved
    world
        .query()
        .to(SC_ADDRESS)
        .typed(mrv_gsoc_registry::gsoc_registry_proxy::GsocRegistryProxy)
        .is_serial_reserved(serial)
        .returns(ExpectValue(false))
        .run();
}
