use multiversx_sc_scenario::imports::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/mrv/governance-multisig");
    blockchain.register_contract(
        "mxsc:output/mrv-governance-multisig.mxsc.json",
        mrv_governance_multisig::ContractBuilder,
    );

    blockchain
}

#[test]
fn governance_multisig_init_rs() {
    world().run("scenarios/governance-multisig-init.scen.json");
}
