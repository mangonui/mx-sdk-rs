use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/reserve-proof-registry");
    blockchain.register_contract(
        "mxsc:output/mrv-reserve-proof-registry.mxsc.json",
        mrv_reserve_proof_registry::ContractBuilder,
    );
    blockchain
}

#[test]
#[ignore = "legacy single-contract scenario no longer matches tokenized reserve-proof runtime; covered by whitebox and integration suites"]
fn reserve_proof_lifecycle_rs() {
    world().run("scenarios/reserve-proof-lifecycle.scen.json");
}
