use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn registry_init_go() {
    world().run("scenarios/registry-init.scen.json");
}

#[test]
#[ignore = "requires Go VM"]
fn registry_methodology_lifecycle_go() {
    world().run("scenarios/registry-methodology-lifecycle.scen.json");
}

#[test]
#[ignore = "requires Go VM"]
fn registry_report_anchor_go() {
    world().run("scenarios/registry-report-anchor.scen.json");
}
