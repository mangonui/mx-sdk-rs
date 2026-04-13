use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    ScenarioWorld::new()
}

#[test]
#[ignore = "requires Go VM"]
fn governance_init_go() {
    world().run("scenarios/governance-init.scen.json");
}

#[test]
#[ignore = "requires Go VM"]
fn governance_propose_accept_go() {
    world().run("scenarios/governance-propose-accept.scen.json");
}

#[test]
#[ignore = "requires Go VM"]
fn governance_vvb_accreditation_go() {
    world().run("scenarios/governance-vvb-accreditation.scen.json");
}
