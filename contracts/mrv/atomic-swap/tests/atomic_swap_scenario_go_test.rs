use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn atomic_swap_init_go() {
    world().run("scenarios/atomic-swap-init.scen.json");
}
