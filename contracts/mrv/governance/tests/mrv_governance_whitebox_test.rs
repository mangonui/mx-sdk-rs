use mrv_governance::MrvGovernance;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");
const VERIFIER: TestAddress = TestAddress::new("verifier");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-governance");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-governance.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/governance");
    world.register_contract(CODE_PATH, mrv_governance::ContractBuilder);
    world
}

#[test]
fn mrv_governance_executes_emergency_pause_proposal() {
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
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-001"), true);
        });

    // Proposer approves their own proposal
    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-001"));
        });

    // Second signer approves (meets quorum of 2)
    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-001"));
        });

    // Advance past timelock (3600s)
    world.current_block().block_timestamp_seconds(3601u64);

    // Execute after timelock
    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-001"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            assert!(sc.paused().get());
        });
}

#[test]
fn mrv_governance_tracks_verifier_accreditation_state() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);
    world.account(VERIFIER).nonce(1).balance(1_000_000u64);

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
            sc.propose_verifier_accreditation(
                ManagedBuffer::from(b"verifier-001"),
                VERIFIER.to_managed_address(),
                true,
                ManagedBuffer::from(b"vvb"),
            );
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"verifier-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"verifier-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"verifier-001"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let accreditation = sc
                .get_verifier_accreditation(VERIFIER.to_managed_address())
                .into_option()
                .unwrap();
            assert!(accreditation.approved);
            assert_eq!(accreditation.role.to_boxed_bytes().as_slice(), b"vvb");
        });
}

const FARMER: TestAddress = TestAddress::new("farmer");
const SIGNER_THREE: TestAddress = TestAddress::new("signer-three");

#[test]
fn mrv_governance_timelock_enforcement_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    // Deploy with 3600s timelock, block timestamp starts at 0
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

    // Propose at timestamp 0 -> eta = 3600
    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_emergency_pause(ManagedBuffer::from(b"timelock-001"), true);
        });

    // Both signers approve
    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"timelock-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"timelock-001"));
        });

    // Attempt to execute before timelock (timestamp still 0, eta = 3600)
    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "timelock not elapsed"))
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"timelock-001"));
        });
}

#[test]
fn mrv_governance_proposal_expiry_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    // Deploy with 3600s timelock
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

    // Propose at timestamp 0 -> eta = 3600
    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_emergency_pause(ManagedBuffer::from(b"expiry-001"), true);
        });

    // Both signers approve
    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"expiry-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"expiry-001"));
        });

    // Advance past eta + 30 days (3600 + 2_592_000 = 2_595_600) + 1
    world.current_block().block_timestamp_seconds(2_595_601u64);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "PROPOSAL_EXPIRED: must be executed within 30 days of timelock expiry",
        ))
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"expiry-001"));
        });
}

#[test]
fn mrv_governance_remove_signer_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_THREE).nonce(1).balance(1_000_000u64);

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
            signers.push(SIGNER_THREE.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            assert!(sc.is_signer(SIGNER_THREE.to_managed_address()));
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_remove_signer(
                ManagedBuffer::from(b"remove-signer-001"),
                SIGNER_THREE.to_managed_address(),
            );
            sc.approve_proposal(ManagedBuffer::from(b"remove-signer-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"remove-signer-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"remove-signer-001"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            assert!(!sc.is_signer(SIGNER_THREE.to_managed_address()));
            assert!(sc.is_signer(SIGNER_ONE.to_managed_address()));
            assert!(sc.is_signer(SIGNER_TWO.to_managed_address()));
        });
}

#[test]
fn mrv_governance_removed_signer_approval_no_longer_counts_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_THREE).nonce(1).balance(1_000_000u64);

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
            signers.push(SIGNER_THREE.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_emergency_pause(ManagedBuffer::from(b"stale-pause"), true);
            sc.approve_proposal(ManagedBuffer::from(b"stale-pause"));
        });

    world
        .tx()
        .from(SIGNER_THREE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"stale-pause"));
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_remove_signer(
                ManagedBuffer::from(b"remove-stale-signer"),
                SIGNER_THREE.to_managed_address(),
            );
            sc.approve_proposal(ManagedBuffer::from(b"remove-stale-signer"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"remove-stale-signer"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"remove-stale-signer"));
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "insufficient approvals"))
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"stale-pause"));
        });
}

#[test]
fn mrv_governance_propose_badge_issuance_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);
    world.account(FARMER).nonce(1).balance(1_000_000u64);

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

    // Signer one proposes badge issuance
    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_badge_issuance(
                ManagedBuffer::from(b"badge-001"),
                FARMER.to_managed_address(),
                ManagedBuffer::from(b"sha256:badge-metadata-hash"),
            );
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"badge-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"badge-001"));
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"badge-001"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let proposal = sc
                .get_proposal(ManagedBuffer::from(b"badge-001"))
                .into_option()
                .unwrap();
            assert!(proposal.executed);

            let badge = sc
                .get_badge_issuance(FARMER.to_managed_address())
                .into_option()
                .unwrap();
            assert_eq!(
                badge.to_boxed_bytes().as_slice(),
                b"sha256:badge-metadata-hash"
            );
        });
}

#[test]
fn mrv_governance_removes_gsoc_verifier_via_timelocked_proposal_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);
    world.account(VERIFIER).nonce(1).balance(1_000_000u64);

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
            sc.propose_gsoc_verifier(
                VERIFIER.to_managed_address(),
                ManagedBuffer::from(b"ipfs://credentials"),
                ManagedBuffer::from(b"KE"),
            );
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_gsoc_verifier_proposal(1);
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_gsoc_verifier_proposal(1);
        });

    world.current_block().block_timestamp_seconds(3601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_gsoc_verifier_proposal(1);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_remove_gsoc_verifier(
                ManagedBuffer::from(b"remove-gsoc-001"),
                VERIFIER.to_managed_address(),
            );
            sc.approve_proposal(ManagedBuffer::from(b"remove-gsoc-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"remove-gsoc-001"));
        });

    world.current_block().block_timestamp_seconds(7202u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"remove-gsoc-001"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance::contract_obj, |sc| {
            assert!(!sc.is_gsoc_verifier_approved(VERIFIER.to_managed_address()));
            assert_eq!(
                sc.get_gsoc_verifier_revoked_at(VERIFIER.to_managed_address()),
                7202u64
            );
            assert!(sc.is_gsoc_verifier_review_required(VERIFIER.to_managed_address()));
        });
}
