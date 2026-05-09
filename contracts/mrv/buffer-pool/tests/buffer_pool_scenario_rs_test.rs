use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/buffer-pool");
    blockchain.register_contract(
        "mxsc:output/mrv-buffer-pool.mxsc.json",
        mrv_buffer_pool::ContractBuilder,
    );

    blockchain
}

#[test]
fn buffer_pool_init_rs() {
    world().run("scenarios/buffer-pool-init.scen.json");
}
