use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/carbon-credit");
    blockchain.register_contract(
        "mxsc:output/mrv-carbon-credit.mxsc.json",
        mrv_carbon_credit::ContractBuilder,
    );

    blockchain
}

#[test]
fn carbon_credit_init_rs() {
    world().run("scenarios/carbon-credit-init.scen.json");
}
