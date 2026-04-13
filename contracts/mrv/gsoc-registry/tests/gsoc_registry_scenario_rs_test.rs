use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/gsoc-registry");
    blockchain.register_contract(
        "mxsc:output/mrv-gsoc-registry.mxsc.json",
        mrv_gsoc_registry::ContractBuilder,
    );
    blockchain
}

#[test]
fn gsoc_registry_lifecycle_rs() {
    world().run("scenarios/gsoc-registry-lifecycle.scen.json");
}
