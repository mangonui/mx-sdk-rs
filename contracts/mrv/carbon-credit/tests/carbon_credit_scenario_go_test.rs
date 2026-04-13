use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn carbon_credit_init_go() {
    world().run("scenarios/carbon-credit-init.scen.json");
}
