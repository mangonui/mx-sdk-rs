use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/aggregator");
    blockchain.register_contract(
        "mxsc:output/mrv-aggregator.mxsc.json",
        mrv_aggregator::ContractBuilder,
    );

    blockchain
}

#[test]
fn aggregator_init_rs() {
    world().run("scenarios/aggregator-init.scen.json");
}
