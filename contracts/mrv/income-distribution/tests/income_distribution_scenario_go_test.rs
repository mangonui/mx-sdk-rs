use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn income_distribution_init_go() {
    world().run("scenarios/income-distribution-init.scen.json");
}
