use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn asset_manager_init_go() {
    world().run("scenarios/asset-manager-init.scen.json");
}

#[test]
#[ignore = "requires Go VM"]
fn asset_manager_denial_signals_go() {
    world().run("scenarios/asset-manager-denial-signals.scen.json");
}
