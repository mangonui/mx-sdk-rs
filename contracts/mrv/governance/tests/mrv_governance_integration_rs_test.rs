use mrv_governance::MrvGovernance;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-governance");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-governance.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/governance");
    world.register_contract(CODE_PATH, mrv_governance::ContractBuilder);
    world
}

#[test]
fn mrv_governance_owner_manages_signers_and_threshold_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            sc.init(1, 3600, signers);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.add_signer(SIGNER_TWO.to_managed_address());
            sc.set_approval_threshold(2);
            // minimum timelock is 3600 seconds
            sc.set_timelock_seconds(3600);
        });

    world.query().to(SC_ADDRESS).whitebox(mrv_governance::contract_obj, |sc| {
        assert!(sc.is_signer(SIGNER_ONE.to_managed_address()));
        assert!(sc.is_signer(SIGNER_TWO.to_managed_address()));
        assert_eq!(sc.approval_threshold().get(), 2);
        assert_eq!(sc.timelock_seconds().get(), 3600);
    });
}

#[test]
fn mrv_governance_requires_quorum_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-quorum"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "insufficient approvals"))
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-quorum"));
        });
}
