use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn identity_registry_init_go() {
    world().run("scenarios/identity-registry-init.scen.json");
}

#[test]
#[ignore = "requires Go VM"]
fn identity_registry_denial_signals_go() {
    world().run("scenarios/identity-registry-denial-signals.scen.json");
}
