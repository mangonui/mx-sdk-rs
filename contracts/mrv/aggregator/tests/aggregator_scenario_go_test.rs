use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn aggregator_init_go() {
    world().run("scenarios/aggregator-init.scen.json");
}
