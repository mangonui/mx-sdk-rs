use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("reserve-proof");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-reserve-proof-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/reserve-proof-registry");
    world.register_contract(CODE_PATH, mrv_reserve_proof_registry::ContractBuilder);
    world
}

#[test]
fn reserve_proof_dvcu_and_gsoc_dual_track_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(0u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .argument(&GOVERNANCE)
        .run();

    let merkle_root = [0u8; 32];

    // Anchor VM0042 dVCU proof at block 100
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_reserve_proof("CARBON-abc123", 1_000_000u64, 100_000u64, 50_000u64, &merkle_root, 100u64)
        .run();

    // Anchor second dVCU proof at block 200 (monotonic increment)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_reserve_proof("CARBON-abc123", 2_000_000u64, 200_000u64, 100_000u64, &merkle_root, 200u64)
        .run();

    // Anchor GSOC dGSC proof (separate track)
    let itmo_hash = [0u8; 32];
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_gsoc_reserve_proof("GSOC-KE-001", 500u64, 50u64, 450u64, &itmo_hash, 100u64)
        .run();
}

#[test]
fn reserve_proof_monotonic_block_guard_rs() {
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

    let merkle_root = [1u8; 32];

    // First proof at block 100
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_reserve_proof("TOKEN-001", 100u64, 10u64, 5u64, &merkle_root, 100u64)
        .run();

    // Backward block (50) should fail
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_reserve_proof("TOKEN-001", 200u64, 20u64, 10u64, &merkle_root, 50u64)
        .with_result(ExpectError(4u64, "SNAPSHOT_BLOCK_NOT_MONOTONIC: new block must be greater than current latest"))
        .run();
}
