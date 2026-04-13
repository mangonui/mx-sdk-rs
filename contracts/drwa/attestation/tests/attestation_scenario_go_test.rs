use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn attestation_init_go() {
    world().run("scenarios/attestation-init.scen.json");
}

#[test]
#[ignore = "requires Go VM"]
fn attestation_denial_signals_go() {
    world().run("scenarios/attestation-denial-signals.scen.json");
}
