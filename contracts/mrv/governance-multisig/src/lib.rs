#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use mrv_common::resolve_storage_version_upgrade;

const MIN_SIGNERS: usize = 2;
/// Proposals expire after 48 hours. This is intentionally shorter than the
/// 30-day window used by `mrv-governance` for timelocked proposals, because
/// multisig operational decisions require faster turnaround.
const PROPOSAL_EXPIRY_SECONDS: u64 = 172_800;

/// Generic governance proposal with typed action metadata.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct GovProposal<M: ManagedTypeApi> {
    pub proposal_id: ManagedBuffer<M>,
    pub proposer: ManagedAddress<M>,
    pub proposal_type: ManagedBuffer<M>,
    pub target_address: ManagedAddress<M>,
    pub action_data: ManagedBuffer<M>,
    pub executed: bool,
    pub created_at: u64,
}

/// Dispute record with vote tallies, requested action, and resolution state.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct DisputeRecord<M: ManagedTypeApi> {
    pub dispute_id: ManagedBuffer<M>,
    pub rfq_id: ManagedBuffer<M>,
    pub accused: ManagedAddress<M>,
    pub evidence_cid: ManagedBuffer<M>,
    pub requested_action: ManagedBuffer<M>,
    pub vote_approve: u32,
    pub vote_reject: u32,
    pub resolved: bool,
    pub action_taken: ManagedBuffer<M>,
    pub decided_at: u64,
    /// Signer count captured at creation time for stable dispute quorum checks.
    pub total_signers_at_creation: u32,
    /// Creation timestamp used to enforce dispute voting expiry.
    pub created_at: u64,
}

/// Operational multisig contract for Dugong-side decision recording and
/// dispute voting.
///
/// This contract is intentionally not the chain-authoritative governance
/// holder for `mrv-registry` or `mrv-gsoc-registry`. It records signer-approved
/// operational decisions and dispute outcomes on-chain, while canonical MRV
/// governance for privileged registry mutations is handled by the separate
/// `mrv-governance` contract.
#[multiversx_sc::contract]
pub trait GovernanceMultisig {
    #[init]
    fn init(&self, threshold: u32, initial_signers: MultiValueEncoded<ManagedAddress>) {
        require!(threshold >= MIN_SIGNERS as u32, "threshold must be >= 2");
        self.threshold().set(threshold);

        let owner = self.blockchain().get_owner_address();
        self.signers().insert(owner);

        for signer in initial_signers.into_iter() {
            require!(!signer.is_zero(), "signer must not be zero");
            self.signers().insert(signer);
        }

        require!(
            self.signers().len() >= threshold as usize,
            "INSUFFICIENT_INITIAL_SIGNERS: must provide enough signers to meet threshold at deploy"
        );
        self.storage_version().set(1u32);
    }

    /// Adds a governance signer.
    #[only_owner]
    #[endpoint(addSigner)]
    fn add_signer(&self, signer: ManagedAddress) {
        require!(!signer.is_zero(), "signer must not be zero");
        self.signers().insert(signer.clone());
        self.signer_added_event(&signer);
    }

    /// Removes a governance signer. Fails if the signer count would drop below threshold.
    #[only_owner]
    #[endpoint(removeSigner)]
    fn remove_signer(&self, signer: ManagedAddress) {
        require!(self.signers().contains(&signer), "not a signer");
        require!(
            self.signers().len() > self.threshold().get() as usize,
            "cannot remove signer below threshold"
        );
        self.signers().swap_remove(&signer);
        self.signer_removed_event(&signer);
    }

    /// Creates a governance proposal of type `force_revert`,
    /// `issuance_reversal`, `verifier_adjustment`, `freeze`, or `unfreeze`.
    #[endpoint(proposeAction)]
    fn propose_action(
        &self,
        proposal_id: ManagedBuffer,
        proposal_type: ManagedBuffer,
        target_address: ManagedAddress,
        action_data: ManagedBuffer,
    ) {
        let caller = self.blockchain().get_caller();
        require!(self.signers().contains(&caller), "caller not a signer");
        require!(!proposal_id.is_empty(), "empty proposal_id");
        require!(
            !self.proposals().contains_key(&proposal_id),
            "proposal already exists"
        );
        require!(
            proposal_type == b"force_revert"
                || proposal_type == b"issuance_reversal"
                || proposal_type == b"verifier_adjustment"
                || proposal_type == b"freeze"
                || proposal_type == b"unfreeze",
            "invalid proposal_type"
        );

        let proposal = GovProposal {
            proposal_id: proposal_id.clone(),
            proposer: caller.clone(),
            proposal_type,
            target_address,
            action_data,
            executed: false,
            created_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
        };

        self.proposals().insert(proposal_id.clone(), proposal);
        self.proposal_created_event(&proposal_id);
    }

    /// Records the caller's approval for a pending proposal.
    #[endpoint(approveProposal)]
    fn approve_proposal(&self, proposal_id: ManagedBuffer) {
        let caller = self.blockchain().get_caller();
        require!(self.signers().contains(&caller), "caller not a signer");
        require!(
            self.proposals().contains_key(&proposal_id),
            "proposal not found"
        );
        require!(
            !self.approvals(&proposal_id).contains(&caller),
            "already approved"
        );

        let proposal_check = self.proposals().get(&proposal_id).unwrap();
        require!(!proposal_check.executed, "proposal already executed");

        self.approvals(&proposal_id).insert(caller.clone());

        let approval_count = self.approvals(&proposal_id).len() as u32;
        self.proposal_approved_event(&proposal_id, &caller, approval_count);
    }

    /// Marks a proposal as executed once it has reached the approval threshold
    /// and has not expired.
    ///
    /// This endpoint records the approved operational decision on-chain and
    /// emits the execution event. It does not dispatch a downstream cross-
    /// contract call.
    #[endpoint(executeProposal)]
    fn execute_proposal(&self, proposal_id: ManagedBuffer) {
        let caller = self.blockchain().get_caller();
        require!(self.signers().contains(&caller), "caller not a signer");

        let proposal = self.proposals().get(&proposal_id);
        require!(proposal.is_some(), "proposal not found");
        let proposal = proposal.unwrap();
        require!(!proposal.executed, "proposal already executed");
        require!(
            self.blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds()
                <= proposal.created_at + PROPOSAL_EXPIRY_SECONDS,
            "PROPOSAL_EXPIRED: proposal must be executed within expiry window"
        );
        require!(
            self.current_proposal_approval_count(&proposal_id) >= self.threshold().get(),
            "insufficient approvals"
        );

        self.proposals().entry(proposal_id.clone()).and_modify(|p| {
            p.executed = true;
        });

        self.approvals(&proposal_id).clear();
        self.proposal_executed_event(&proposal_id);
    }

    /// Submits a dispute requesting `FREEZE`, `CLAW_BACK`, or `WARN`.
    #[endpoint(submitDispute)]
    fn submit_dispute(
        &self,
        dispute_id: ManagedBuffer,
        rfq_id: ManagedBuffer,
        accused: ManagedAddress,
        evidence_cid: ManagedBuffer,
        requested_action: ManagedBuffer,
    ) {
        let caller = self.blockchain().get_caller();
        require!(
            self.signers().contains(&caller) || caller == self.blockchain().get_owner_address(),
            "caller not authorized to submit disputes"
        );
        require!(!dispute_id.is_empty(), "empty dispute_id");
        require!(!evidence_cid.is_empty(), "empty evidence_cid");
        require!(
            requested_action == b"FREEZE"
                || requested_action == b"CLAW_BACK"
                || requested_action == b"WARN",
            "requested_action must be FREEZE, CLAW_BACK, or WARN"
        );
        require!(
            !self.disputes().contains_key(&dispute_id),
            "dispute already exists"
        );

        let record = DisputeRecord {
            dispute_id: dispute_id.clone(),
            rfq_id,
            accused,
            evidence_cid,
            requested_action,
            vote_approve: 0u32,
            vote_reject: 0u32,
            resolved: false,
            action_taken: ManagedBuffer::new(),
            decided_at: 0u64,
            total_signers_at_creation: self.signers().len() as u32,
            created_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
        };

        let mut eligible_signer_set = self.dispute_eligible_signer_set(&dispute_id);
        for signer in self.signers().iter() {
            eligible_signer_set.insert(signer.as_managed_buffer().clone());
        }

        self.disputes().insert(dispute_id.clone(), record);
        self.dispute_submitted_event(&dispute_id);
    }

    /// Casts an approval or rejection vote on a dispute and resolves it once a
    /// two-thirds supermajority is reached.
    #[endpoint(voteOnDispute)]
    fn vote_on_dispute(&self, dispute_id: ManagedBuffer, approve: bool) {
        let caller = self.blockchain().get_caller();
        require!(self.signers().contains(&caller), "caller not a signer");
        require!(
            self.disputes().contains_key(&dispute_id),
            "dispute not found"
        );

        let dispute_check = self.disputes().get(&dispute_id).unwrap();
        require!(!dispute_check.resolved, "dispute already resolved");
        let now = self
            .blockchain()
            .get_block_timestamp_seconds()
            .as_u64_seconds();
        let dispute_expiry = dispute_check
            .created_at
            .checked_add(2_592_000u64)
            .unwrap_or_else(|| sc_panic!("dispute expiry overflow"));
        require!(
            now <= dispute_expiry,
            "DISPUTE_EXPIRED: disputes must be resolved within 30 days of creation"
        );

        let caller_buf = caller.as_managed_buffer().clone();
        require!(
            self.dispute_eligible_signer_set(&dispute_id)
                .contains(&caller_buf),
            "caller was not eligible when dispute was created"
        );

        let dispute_vote_key = (dispute_id.clone(), caller_buf.clone());
        require!(
            !self.dispute_votes().contains_key(&dispute_vote_key),
            "already voted on this dispute"
        );

        self.dispute_votes().insert(dispute_vote_key, approve);
        // Track voter in per-dispute set for efficient cleanup on resolution.
        self.dispute_voter_set(&dispute_id).insert(caller_buf);

        let decided_ts = self
            .blockchain()
            .get_block_timestamp_seconds()
            .as_u64_seconds();
        let (active_approve_votes, active_reject_votes) =
            self.current_dispute_vote_counts(&dispute_id);

        self.disputes().entry(dispute_id.clone()).and_modify(|d| {
            d.vote_approve = active_approve_votes;
            d.vote_reject = active_reject_votes;

            let total_signers = d.total_signers_at_creation;
            let required = (total_signers * 2).div_ceil(3);
            if d.vote_approve >= required {
                d.resolved = true;
                d.action_taken = d.requested_action.clone();
                d.decided_at = decided_ts;
            } else if d.vote_reject >= required {
                d.resolved = true;
                d.action_taken = ManagedBuffer::from(b"DISMISSED");
                d.decided_at = decided_ts;
            }
        });

        // Re-read the dispute because `and_modify` does not return the updated
        // value.
        let resolved_check = self.disputes().get(&dispute_id).unwrap();
        if resolved_check.resolved {
            // Clear votes for this dispute using the per-dispute voter set,
            // avoiding a full scan of the global dispute_votes mapper.
            let voter_set_mapper = self.dispute_voter_set(&dispute_id);
            for voter_buf in voter_set_mapper.iter() {
                let vk = (dispute_id.clone(), voter_buf.clone());
                self.dispute_votes().remove(&vk);
            }
            self.dispute_voter_set(&dispute_id).clear();
            self.dispute_eligible_signer_set(&dispute_id).clear();
        }

        self.dispute_voted_event(&dispute_id, &caller, approve);
    }

    #[view(getProposal)]
    fn get_proposal(&self, proposal_id: ManagedBuffer) -> OptionalValue<GovProposal<Self::Api>> {
        match self.proposals().get(&proposal_id) {
            Some(p) => OptionalValue::Some(p),
            None => OptionalValue::None,
        }
    }

    #[view(getDispute)]
    fn get_dispute(&self, dispute_id: ManagedBuffer) -> OptionalValue<DisputeRecord<Self::Api>> {
        match self.disputes().get(&dispute_id) {
            Some(d) => OptionalValue::Some(d),
            None => OptionalValue::None,
        }
    }

    #[view(isSigner)]
    fn is_signer(&self, addr: ManagedAddress) -> bool {
        self.signers().contains(&addr)
    }

    fn current_proposal_approval_count(&self, proposal_id: &ManagedBuffer) -> u32 {
        let mut count = 0u32;
        for approver in self.approvals(proposal_id).iter() {
            if self.signers().contains(&approver) {
                count += 1;
            }
        }
        count
    }

    fn current_dispute_vote_counts(&self, dispute_id: &ManagedBuffer) -> (u32, u32) {
        let mut approve_count = 0u32;
        let mut reject_count = 0u32;
        for voter_buf in self.dispute_voter_set(dispute_id).iter() {
            if !self
                .dispute_eligible_signer_set(dispute_id)
                .contains(&voter_buf)
            {
                continue;
            }
            if !self.is_current_signer_buffer(&voter_buf) {
                continue;
            }
            let vote_key = (dispute_id.clone(), voter_buf.clone());
            if let Some(approve) = self.dispute_votes().get(&vote_key) {
                if approve {
                    approve_count += 1;
                } else {
                    reject_count += 1;
                }
            }
        }
        (approve_count, reject_count)
    }

    fn is_current_signer_buffer(&self, voter_buf: &ManagedBuffer) -> bool {
        for signer in self.signers().iter() {
            if signer.as_managed_buffer() == voter_buf {
                return true;
            }
        }
        false
    }

    #[storage_mapper("threshold")]
    fn threshold(&self) -> SingleValueMapper<u32>;

    #[storage_mapper("signers")]
    fn signers(&self) -> UnorderedSetMapper<ManagedAddress>;

    #[storage_mapper("proposals")]
    fn proposals(&self) -> MapMapper<ManagedBuffer, GovProposal<Self::Api>>;

    /// Set of signer approvals keyed by proposal identifier.
    #[storage_mapper("approvals")]
    fn approvals(&self, proposal_id: &ManagedBuffer) -> UnorderedSetMapper<ManagedAddress>;

    #[storage_mapper("disputes")]
    fn disputes(&self) -> MapMapper<ManagedBuffer, DisputeRecord<Self::Api>>;

    #[storage_mapper("disputeVotes")]
    fn dispute_votes(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), bool>;

    /// Per-dispute voter tracking for efficient cleanup on resolution.
    /// Cleared when the dispute resolves.
    #[storage_mapper("disputeVoterSet")]
    fn dispute_voter_set(&self, dispute_id: &ManagedBuffer) -> UnorderedSetMapper<ManagedBuffer>;

    /// Per-dispute signer snapshot captured at submission time. Newly added
    /// signers cannot vote on older disputes with smaller frozen thresholds.
    #[storage_mapper("disputeEligibleSignerSet")]
    fn dispute_eligible_signer_set(
        &self,
        dispute_id: &ManagedBuffer,
    ) -> UnorderedSetMapper<ManagedBuffer>;

    #[event("signerAdded")]
    fn signer_added_event(&self, #[indexed] signer: &ManagedAddress);

    #[event("signerRemoved")]
    fn signer_removed_event(&self, #[indexed] signer: &ManagedAddress);

    #[event("proposalCreated")]
    fn proposal_created_event(&self, #[indexed] proposal_id: &ManagedBuffer);

    #[event("proposalApproved")]
    fn proposal_approved_event(
        &self,
        #[indexed] proposal_id: &ManagedBuffer,
        #[indexed] signer: &ManagedAddress,
        approval_count: u32,
    );

    #[event("proposalExecuted")]
    fn proposal_executed_event(&self, #[indexed] proposal_id: &ManagedBuffer);

    #[event("disputeSubmitted")]
    fn dispute_submitted_event(&self, #[indexed] dispute_id: &ManagedBuffer);

    #[event("disputeVoted")]
    fn dispute_voted_event(
        &self,
        #[indexed] dispute_id: &ManagedBuffer,
        #[indexed] voter: &ManagedAddress,
        approve: bool,
    );

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    /// Upgrades storage layout version if needed and preserves existing state.
    #[upgrade]
    fn upgrade(&self) {
        let stored = self.storage_version().get();
        let target = resolve_storage_version_upgrade(stored, 1u32, 1u32)
            .unwrap_or_else(|message| sc_panic!(message));
        if stored != target {
            self.storage_version().set(target);
        }
    }
}
