use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/atomic-swap");
    blockchain.register_contract(
        "mxsc:output/mrv-atomic-swap.mxsc.json",
        mrv_atomic_swap::ContractBuilder,
    );

    blockchain
}

#[test]
fn atomic_swap_init_rs() {
    world().run("scenarios/atomic-swap-init.scen.json");
}
