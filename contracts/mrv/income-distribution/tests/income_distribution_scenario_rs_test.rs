use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/income-distribution");
    blockchain.register_contract(
        "mxsc:output/mrv-income-distribution.mxsc.json",
        mrv_income_distribution::ContractBuilder,
    );

    blockchain
}

#[test]
fn income_distribution_init_rs() {
    world().run("scenarios/income-distribution-init.scen.json");
}
