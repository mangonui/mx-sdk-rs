use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-common-sync-gas");
const CODE_PATH: MxscPath = MxscPath::new("output/drwa-common-test-harness.mxsc.json");

const MAX_SYNC_OPERATIONS: usize = 256;
const NEAR_CAP_BODY_BYTES: usize = 4_029;
const MAX_SYNC_EMIT_GAS: u64 = 75_000_000;

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new()
        .executor_config(ExecutorConfig::Experimental)
        .gas_schedule(GasScheduleVersion::V8);
    world.set_current_dir_from_workspace("contracts/drwa/common/test-harness");
    world.register_contract(CODE_PATH, drwa_common_test_harness::ContractBuilder);
    world
}

fn deploy(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();
}

#[test]
fn common_emit_sync_envelope_max_payload_gas_headroom() {
    let mut world = world();
    deploy(&mut world);

    let body = [b'a'; NEAR_CAP_BODY_BYTES];
    let gas_used = world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .gas(600_000_000u64)
        .raw_call("testEmitMaxSyncEnvelope")
        .argument(&MAX_SYNC_OPERATIONS)
        .argument(&ManagedBuffer::<StaticApi>::from(&body[..]))
        .returns(ReturnsGasUsed)
        .run();

    println!("common_emit_sync_envelope_max_payload_gas={gas_used}");
    assert!(
        gas_used > 0 && gas_used < MAX_SYNC_EMIT_GAS,
        "max sync emit gas {gas_used} must stay below {MAX_SYNC_EMIT_GAS}"
    );
}
