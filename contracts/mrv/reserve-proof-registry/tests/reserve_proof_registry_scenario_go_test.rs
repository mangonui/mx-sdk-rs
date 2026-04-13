use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn reserve_proof_lifecycle_go() {
    world().run("scenarios/reserve-proof-lifecycle.scen.json");
}
