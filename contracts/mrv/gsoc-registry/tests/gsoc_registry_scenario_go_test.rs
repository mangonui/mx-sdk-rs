use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn gsoc_registry_lifecycle_go() {
    world().run("scenarios/gsoc-registry-lifecycle.scen.json");
}
