use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/attestation");
    blockchain.register_contract(
        "mxsc:output/drwa-attestation.mxsc.json",
        drwa_attestation::ContractBuilder,
    );

    blockchain
}

#[test]
fn attestation_init_rs() {
    world().run("scenarios/attestation-init.scen.json");
}

#[test]
fn attestation_denial_signals_rs() {
    world().run("scenarios/attestation-denial-signals.scen.json");
}
