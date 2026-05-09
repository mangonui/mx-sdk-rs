use multiversx_sc_scenario::*;

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new();
    blockchain.register_contract(
        "mxsc:output/mrv-governance.mxsc.json",
        mrv_governance::ContractBuilder,
    );
    blockchain
}

#[test]
#[ignore] // Requires mxsc.json artifact from sc-meta build step
fn governance_init_rs() {
    world().run("scenarios/governance-init.scen.json");
}

#[test]
#[ignore] // Requires mxsc.json artifact from sc-meta build step
fn governance_propose_accept_rs() {
    world().run("scenarios/governance-propose-accept.scen.json");
}

#[test]
#[ignore] // Requires mxsc.json artifact from sc-meta build step
fn governance_vvb_accreditation_rs() {
    world().run("scenarios/governance-vvb-accreditation.scen.json");
}
