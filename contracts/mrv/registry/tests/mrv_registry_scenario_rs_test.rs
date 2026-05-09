use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new();
    blockchain.register_contract(
        "mxsc:output/mrv-registry.mxsc.json",
        mrv_registry::ContractBuilder,
    );
    blockchain
}

#[test]
#[ignore] // Requires mxsc.json artifact from sc-meta build step
fn registry_init_rs() {
    world().run("scenarios/registry-init.scen.json");
}

#[test]
#[ignore] // Requires mxsc.json artifact from sc-meta build step
fn registry_methodology_lifecycle_rs() {
    world().run("scenarios/registry-methodology-lifecycle.scen.json");
}

#[test]
#[ignore] // Requires mxsc.json artifact from sc-meta build step
fn registry_report_anchor_rs() {
    world().run("scenarios/registry-report-anchor.scen.json");
}
