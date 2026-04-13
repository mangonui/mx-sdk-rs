use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/identity-registry");
    blockchain.register_contract(
        "mxsc:output/drwa-identity-registry.mxsc.json",
        drwa_identity_registry::ContractBuilder,
    );

    blockchain
}

#[test]
fn identity_registry_init_rs() {
    world().run("scenarios/identity-registry-init.scen.json");
}

#[test]
fn identity_registry_denial_signals_rs() {
    world().run("scenarios/identity-registry-denial-signals.scen.json");
}
