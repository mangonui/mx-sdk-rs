use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn come_settlement_init_go() {
    world().run("scenarios/come-settlement-init.scen.json");
}
