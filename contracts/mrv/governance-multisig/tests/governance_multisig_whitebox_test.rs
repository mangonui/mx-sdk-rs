use mrv_governance_multisig::GovernanceMultisig;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-governance-multisig");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-governance-multisig.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/governance-multisig");
    world.register_contract(CODE_PATH, mrv_governance_multisig::ContractBuilder);
    world
}

#[test]
fn governance_multisig_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            // Pass initial signer so signers >= threshold at deploy
            let mut initial_signers = MultiValueEncoded::new();
            initial_signers.push(SIGNER_ONE.to_managed_address());
            sc.init(2u32, initial_signers);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            assert_eq!(sc.threshold().get(), 2u32);
            // Owner is automatically added as a signer on init
            assert!(sc.is_signer(OWNER.to_managed_address()));
        });
}

#[test]
fn governance_multisig_add_signer_rs() {
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
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            // Pass initial signer so signers >= threshold at deploy
            let mut initial_signers = MultiValueEncoded::new();
            initial_signers.push(SIGNER_ONE.to_managed_address());
            sc.init(2u32, initial_signers);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.add_signer(SIGNER_ONE.to_managed_address());
            sc.add_signer(SIGNER_TWO.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            assert!(sc.is_signer(OWNER.to_managed_address()));
            assert!(sc.is_signer(SIGNER_ONE.to_managed_address()));
            assert!(sc.is_signer(SIGNER_TWO.to_managed_address()));
        });
}

#[test]
fn governance_multisig_propose_approve_execute_rs() {
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
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            // Pass initial signer so signers >= threshold at deploy
            let mut initial_signers = MultiValueEncoded::new();
            initial_signers.push(SIGNER_ONE.to_managed_address());
            sc.init(2u32, initial_signers);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.add_signer(SIGNER_ONE.to_managed_address());
            sc.add_signer(SIGNER_TWO.to_managed_address());
        });

    // Signer one proposes (no auto-approval, count=0)
    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.propose_action(
                ManagedBuffer::from(b"prop-001"),
                ManagedBuffer::from(b"force_revert"),
                SIGNER_TWO.to_managed_address(),
                ManagedBuffer::from(b"revert-data"),
            );
        },
    );

    // Signer one explicitly approves their own proposal (count=1)
    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"prop-001"));
        },
    );

    // Signer two approves (count=2, meets threshold)
    world.tx().from(SIGNER_TWO).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"prop-001"));
        },
    );

    // Execute
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"prop-001"));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            let proposal = sc
                .get_proposal(ManagedBuffer::from(b"prop-001"))
                .into_option()
                .unwrap();
            assert!(proposal.executed);
        });
}

const SIGNER_THREE: TestAddress = TestAddress::new("signer-three");

/// Helper: deploys governance-multisig with threshold=2 and 3 signers (owner + signer_one + signer_two).
fn deploy_governance_multisig(world: &mut ScenarioWorld) {
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
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            // Pass initial signer so signers >= threshold at deploy
            let mut initial_signers = MultiValueEncoded::new();
            initial_signers.push(SIGNER_ONE.to_managed_address());
            sc.init(2u32, initial_signers);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.add_signer(SIGNER_ONE.to_managed_address());
            sc.add_signer(SIGNER_TWO.to_managed_address());
            sc.add_signer(SIGNER_THREE.to_managed_address());
        });
}

#[test]
fn governance_multisig_remove_signer_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    // 4 signers (owner + 3), threshold = 2. Can remove one.
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.remove_signer(SIGNER_THREE.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            assert!(!sc.is_signer(SIGNER_THREE.to_managed_address()));
            assert!(sc.is_signer(SIGNER_ONE.to_managed_address()));
            assert!(sc.is_signer(SIGNER_TWO.to_managed_address()));
        });
}

#[test]
fn governance_multisig_removed_signer_approval_no_longer_counts_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.propose_action(
                ManagedBuffer::from(b"stale-prop"),
                ManagedBuffer::from(b"force_revert"),
                SIGNER_TWO.to_managed_address(),
                ManagedBuffer::from(b"revert-data"),
            );
            sc.approve_proposal(ManagedBuffer::from(b"stale-prop"));
        },
    );

    world.tx().from(SIGNER_THREE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"stale-prop"));
        },
    );

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.remove_signer(SIGNER_THREE.to_managed_address());
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "insufficient approvals"))
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"stale-prop"));
        });
}

#[test]
fn governance_multisig_submit_dispute_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.submit_dispute(
                ManagedBuffer::from(b"dispute-001"),
                ManagedBuffer::from(b"RFQ-001"),
                SIGNER_TWO.to_managed_address(),
                ManagedBuffer::from(b"bafyevidence001"),
                ManagedBuffer::from(b"FREEZE"),
            );
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            let dispute = sc
                .get_dispute(ManagedBuffer::from(b"dispute-001"))
                .into_option()
                .unwrap();
            assert_eq!(
                dispute.dispute_id.to_boxed_bytes().as_slice(),
                b"dispute-001"
            );
            assert_eq!(
                dispute.requested_action.to_boxed_bytes().as_slice(),
                b"FREEZE"
            );
            assert!(!dispute.resolved);
            assert_eq!(dispute.vote_approve, 0u32);
            assert_eq!(dispute.vote_reject, 0u32);
        });
}

#[test]
fn governance_multisig_vote_on_dispute_and_resolve_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    // Submit dispute
    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.submit_dispute(
                ManagedBuffer::from(b"dispute-002"),
                ManagedBuffer::from(b"RFQ-002"),
                SIGNER_THREE.to_managed_address(),
                ManagedBuffer::from(b"bafyevidence002"),
                ManagedBuffer::from(b"FREEZE"),
            );
        },
    );

    // Vote approve from signer_one and signer_two
    // 4 signers, required = ceil(4*2/3) = ceil(8/3) = 3
    // So we need 3 approvals for 4 signers
    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-002"), true);
        },
    );

    world.tx().from(SIGNER_TWO).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-002"), true);
        },
    );

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-002"), true);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            let dispute = sc
                .get_dispute(ManagedBuffer::from(b"dispute-002"))
                .into_option()
                .unwrap();
            assert!(dispute.resolved);
            assert_eq!(dispute.action_taken.to_boxed_bytes().as_slice(), b"FREEZE");
            assert_eq!(dispute.vote_approve, 3u32);
        });
}

#[test]
fn governance_multisig_removed_signer_dispute_vote_no_longer_counts_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.submit_dispute(
                ManagedBuffer::from(b"dispute-stale-vote"),
                ManagedBuffer::from(b"RFQ-STALE"),
                SIGNER_THREE.to_managed_address(),
                ManagedBuffer::from(b"bafystalevote"),
                ManagedBuffer::from(b"FREEZE"),
            );
        },
    );

    world.tx().from(SIGNER_THREE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-stale-vote"), true);
        },
    );

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.remove_signer(SIGNER_THREE.to_managed_address());
        });

    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-stale-vote"), true);
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            let dispute = sc
                .get_dispute(ManagedBuffer::from(b"dispute-stale-vote"))
                .into_option()
                .unwrap();
            assert!(!dispute.resolved);
            assert_eq!(dispute.vote_approve, 1u32);
        });

    world.tx().from(SIGNER_TWO).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-stale-vote"), true);
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            let dispute = sc
                .get_dispute(ManagedBuffer::from(b"dispute-stale-vote"))
                .into_option()
                .unwrap();
            assert!(!dispute.resolved);
            assert_eq!(dispute.vote_approve, 2u32);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-stale-vote"), true);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            let dispute = sc
                .get_dispute(ManagedBuffer::from(b"dispute-stale-vote"))
                .into_option()
                .unwrap();
            assert!(dispute.resolved);
            assert_eq!(dispute.vote_approve, 3u32);
            assert_eq!(dispute.action_taken.to_boxed_bytes().as_slice(), b"FREEZE");
        });
}

#[test]
fn governance_multisig_added_signer_after_dispute_cannot_vote_rs() {
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
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            let mut initial_signers = MultiValueEncoded::new();
            initial_signers.push(SIGNER_ONE.to_managed_address());
            initial_signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2u32, initial_signers);
        });

    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.submit_dispute(
                ManagedBuffer::from(b"dispute-added-signer"),
                ManagedBuffer::from(b"RFQ-ADDED"),
                SIGNER_TWO.to_managed_address(),
                ManagedBuffer::from(b"bafyaddedsigner"),
                ManagedBuffer::from(b"FREEZE"),
            );
        },
    );

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.add_signer(SIGNER_THREE.to_managed_address());
        });

    world
        .tx()
        .from(SIGNER_THREE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "caller was not eligible when dispute was created",
        ))
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-added-signer"), true);
        });

    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-added-signer"), true);
        },
    );

    world.tx().from(SIGNER_TWO).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-added-signer"), true);
        },
    );

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            let dispute = sc
                .get_dispute(ManagedBuffer::from(b"dispute-added-signer"))
                .into_option()
                .unwrap();
            assert!(dispute.resolved);
            assert_eq!(dispute.vote_approve, 2u32);
            assert_eq!(dispute.action_taken.to_boxed_bytes().as_slice(), b"FREEZE");
        });
}

#[test]
fn governance_multisig_vote_on_nonexistent_dispute_fails_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "dispute not found"))
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-nonexistent"), true);
        });
}

#[test]
fn governance_multisig_remove_signer_below_threshold_fails_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);

    // Deploy with threshold=2 and only 2 signers (owner + signer_one)
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            // Pass initial signer so signers >= threshold at deploy
            let mut initial_signers = MultiValueEncoded::new();
            initial_signers.push(SIGNER_ONE.to_managed_address());
            sc.init(2u32, initial_signers);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.add_signer(SIGNER_ONE.to_managed_address());
        });

    // Try to remove signer_one — would leave only owner (1 < threshold 2)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "cannot remove signer below threshold"))
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.remove_signer(SIGNER_ONE.to_managed_address());
        });
}

#[test]
fn governance_multisig_proposal_expiry_rejection_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    // Create proposal at timestamp 0
    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.propose_action(
                ManagedBuffer::from(b"prop-expiry"),
                ManagedBuffer::from(b"freeze"),
                SIGNER_TWO.to_managed_address(),
                ManagedBuffer::from(b"freeze-data"),
            );
        },
    );

    // Approve from two signers
    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"prop-expiry"));
        },
    );

    world.tx().from(SIGNER_TWO).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"prop-expiry"));
        },
    );

    // Advance past 48h expiry (172_800 + 1)
    world.current_block().block_timestamp_seconds(172_801u64);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "PROPOSAL_EXPIRED: proposal must be executed within expiry window",
        ))
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"prop-expiry"));
        });
}

#[test]
fn governance_multisig_dispute_expiry_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    // Submit dispute at timestamp 0
    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.submit_dispute(
                ManagedBuffer::from(b"dispute-expiry"),
                ManagedBuffer::from(b"RFQ-EXP"),
                SIGNER_TWO.to_managed_address(),
                ManagedBuffer::from(b"bafyevidenceexp"),
                ManagedBuffer::from(b"FREEZE"),
            );
        },
    );

    // Advance past 30 days (2_592_000 + 1)
    world.current_block().block_timestamp_seconds(2_592_001u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "DISPUTE_EXPIRED: disputes must be resolved within 30 days of creation",
        ))
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-expiry"), true);
        });
}

#[test]
fn governance_multisig_double_vote_rejection_rs() {
    let mut world = world();
    deploy_governance_multisig(&mut world);

    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.submit_dispute(
                ManagedBuffer::from(b"dispute-dv"),
                ManagedBuffer::from(b"RFQ-DV"),
                SIGNER_TWO.to_managed_address(),
                ManagedBuffer::from(b"bafyevidencedv"),
                ManagedBuffer::from(b"WARN"),
            );
        },
    );

    world.tx().from(SIGNER_ONE).to(SC_ADDRESS).whitebox(
        mrv_governance_multisig::contract_obj,
        |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-dv"), true);
        },
    );

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "already voted on this dispute"))
        .whitebox(mrv_governance_multisig::contract_obj, |sc| {
            sc.vote_on_dispute(ManagedBuffer::from(b"dispute-dv"), false);
        });
}
