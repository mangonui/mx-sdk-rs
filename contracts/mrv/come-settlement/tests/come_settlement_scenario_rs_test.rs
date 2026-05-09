use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/come-settlement");
    blockchain.register_contract(
        "mxsc:output/mrv-come-settlement.mxsc.json",
        mrv_come_settlement::ContractBuilder,
    );

    blockchain
}

#[test]
fn come_settlement_init_rs() {
    world().run("scenarios/come-settlement-init.scen.json");
}
