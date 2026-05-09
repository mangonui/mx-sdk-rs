use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/policy-registry");
    blockchain.register_contract(
        "mxsc:output/drwa-policy-registry.mxsc.json",
        drwa_policy_registry::ContractBuilder,
    );
    blockchain
}

#[test]
fn policy_registry_init_rs() {
    world().run("scenarios/policy-registry-init.scen.json");
}

#[test]
fn policy_registry_denial_signals_rs() {
    world().run("scenarios/policy-registry-denial-signals.scen.json");
}
