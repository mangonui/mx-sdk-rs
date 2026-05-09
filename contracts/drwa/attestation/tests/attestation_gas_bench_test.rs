use multiversx_sc_scenario::imports::*;

use drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const AUDITOR: TestAddress = TestAddress::new("auditor");
const SUBJECT: TestAddress = TestAddress::new("subject");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-attestation-gas");
const CODE_PATH: MxscPath = MxscPath::new("output/drwa-attestation.mxsc.json");
const TOKEN_ID: &str = "CARBON-ab12cd";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new()
        .executor_config(ExecutorConfig::Experimental)
        .gas_schedule(GasScheduleVersion::V8);
    blockchain.set_current_dir_from_workspace("contracts/drwa/attestation");
    blockchain.register_contract(CODE_PATH, drwa_attestation::ContractBuilder);
    blockchain
}

#[test]
#[ignore = "requires experimental gas executor"]
fn attestation_gas_smoke_for_record_and_revoke() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(SUBJECT).nonce(1).balance(1_000_000u64);

    let deploy_gas = world
        .tx()
        .from(OWNER)
        .typed(DrwaAttestationProxy)
        .init(AUDITOR)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .returns(ReturnsGasUsed)
        .run();
    assert!(deploy_gas > 0);

    let record_gas = world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .typed(DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            SUBJECT.to_managed_address(),
            "MRV",
            "hash-gas",
            true,
        )
        .returns(ReturnsGasUsed)
        .run();
    assert!(record_gas > 0);

    let revoke_gas = world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .typed(DrwaAttestationProxy)
        .revoke_attestation(TOKEN_ID, SUBJECT.to_managed_address())
        .returns(ReturnsGasUsed)
        .run();
    assert!(revoke_gas > 0);
}
