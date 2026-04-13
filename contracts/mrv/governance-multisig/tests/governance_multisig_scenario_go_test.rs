use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn governance_multisig_init_go() {
    world().run("scenarios/governance-multisig-init.scen.json");
}
