#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub use mrv_common::{GsocVerifierEntry, VerifierAccreditation};

/// Proposal type for emergency pause/unpause actions.
const PROPOSAL_TYPE_PAUSE: u8 = 1;
/// Proposal type for granting or revoking VVB accreditation.
const PROPOSAL_TYPE_VERIFIER_ACCREDITATION: u8 = 2;
/// Proposal type used for Green Badge SFT issuance.
/// The proposal target stores the farmer address and the `role` field stores
/// the badge metadata hash.
const PROPOSAL_TYPE_BADGE_ISSUANCE: u8 = 3;

/// Multi-sig governance proposal supporting pause, verifier accreditation,
/// and Green Badge issuance actions.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
pub struct GovernanceProposal<M: ManagedTypeApi> {
    pub proposal_id: ManagedBuffer<M>,
    pub proposal_type: u8,
    pub target: ManagedAddress<M>,
    pub bool_value: bool,
    pub role: ManagedBuffer<M>,
    pub eta: u64,
    pub executed: bool,
    pub executed_at_timestamp: u64,
}

/// GSOC verifier proposal record.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
pub struct GsocVerifierProposal<M: ManagedTypeApi> {
    pub verifier_did: ManagedAddress<M>,
    pub credentials_cid: ManagedBuffer<M>,
    pub jurisdiction: ManagedBuffer<M>,
    pub eta: u64,
    pub executed: bool,
}

/// Multi-sig governance contract with timelock enforcement for MRV
/// ecosystem actions: emergency pause, VVB accreditation, GSOC verifier
/// management, and Green Badge issuance.
#[multiversx_sc::contract]
pub trait MrvGovernance {
    /// Initializes the signer set, approval threshold, and timelock duration.
    #[init]
    fn init(
        &self,
        approval_threshold: u32,
        timelock_seconds: u64,
        initial_signers: MultiValueEncoded<ManagedAddress>,
    ) {
        require!(approval_threshold > 0, "invalid approval threshold");
        require!(timelock_seconds >= 3600, "TIMELOCK_TOO_LOW: must be at least 1 hour (3600 seconds)");

        let mut signer_count = 0u32;
        for signer in initial_signers {
            require!(!signer.is_zero(), "signer must not be zero");
            if !self.signers().contains(&signer) {
                self.signers().insert(signer);
                signer_count += 1;
            }
        }

        require!(signer_count >= approval_threshold, "threshold exceeds signer count");
        self.approval_threshold().set(approval_threshold);
        self.timelock_seconds().set(timelock_seconds);
        self.paused().set(false);
        self.next_gsoc_verifier_proposal_id().set(1u64);
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
        self.signers().swap_remove(&signer);
        require!(
            self.signers().len() >= self.approval_threshold().get() as usize,
            "SIGNERS_BELOW_THRESHOLD: cannot remove signer below approval threshold"
        );
        self.signer_removed_event(&signer);
    }

    /// Updates the approval quorum.
    #[only_owner]
    #[endpoint(setApprovalThreshold)]
    fn set_approval_threshold(&self, approval_threshold: u32) {
        require!(approval_threshold > 0, "invalid approval threshold");
        require!(
            (self.signers().len() as u32) >= approval_threshold,
            "threshold exceeds signer count"
        );
        self.approval_threshold().set(approval_threshold);
        self.threshold_changed_event(approval_threshold);
    }

    /// Updates the timelock duration.
    #[only_owner]
    #[endpoint(setTimelockSeconds)]
    fn set_timelock_seconds(&self, timelock_seconds: u64) {
        require!(timelock_seconds >= 3600, "TIMELOCK_TOO_SHORT: minimum 3600 seconds (1 hour)");
        self.timelock_seconds().set(timelock_seconds);
        self.timelock_changed_event(timelock_seconds);
    }

    /// Create an emergency pause/unpause proposal. Subject to timelock and multi-sig approval.
    #[endpoint(proposeEmergencyPause)]
    fn propose_emergency_pause(&self, proposal_id: ManagedBuffer, pause: bool) {
        self.require_signer();
        require!(!proposal_id.is_empty(), "empty proposal id");
        require!(!self.proposals().contains_key(&proposal_id), "proposal already exists");

        let proposal = GovernanceProposal {
            proposal_id: proposal_id.clone(),
            proposal_type: PROPOSAL_TYPE_PAUSE,
            target: ManagedAddress::zero(),
            bool_value: pause,
            role: ManagedBuffer::new(),
            eta: self.blockchain().get_block_timestamp_seconds().as_u64_seconds().saturating_add(self.timelock_seconds().get()),
            executed: false,
            executed_at_timestamp: 0u64,
        };

        self.proposals().insert(proposal_id.clone(), proposal);
        self.proposal_created_event(&proposal_id, PROPOSAL_TYPE_PAUSE);
    }

    /// Propose granting or revoking VVB accreditation for a verifier address.
    #[endpoint(proposeVerifierAccreditation)]
    fn propose_verifier_accreditation(
        &self,
        proposal_id: ManagedBuffer,
        verifier: ManagedAddress,
        approved: bool,
        role: ManagedBuffer,
    ) {
        self.require_signer();
        require!(!proposal_id.is_empty(), "empty proposal id");
        require!(!verifier.is_zero(), "verifier must not be zero");
        require!(!role.is_empty(), "empty verifier role");
        require!(!self.proposals().contains_key(&proposal_id), "proposal already exists");

        let proposal = GovernanceProposal {
            proposal_id: proposal_id.clone(),
            proposal_type: PROPOSAL_TYPE_VERIFIER_ACCREDITATION,
            target: verifier,
            bool_value: approved,
            role,
            eta: self.blockchain().get_block_timestamp_seconds().as_u64_seconds().saturating_add(self.timelock_seconds().get()),
            executed: false,
            executed_at_timestamp: 0u64,
        };

        self.proposals().insert(proposal_id.clone(), proposal);
        self.proposal_created_event(&proposal_id, PROPOSAL_TYPE_VERIFIER_ACCREDITATION);
    }

    /// Proposes Green Badge SFT issuance for a farmer address.
    #[endpoint(proposeBadgeIssuance)]
    fn propose_badge_issuance(
        &self,
        proposal_id: ManagedBuffer,
        farmer: ManagedAddress,
        badge_metadata_hash: ManagedBuffer,
    ) {
        self.require_signer();
        require!(!proposal_id.is_empty(), "empty proposal id");
        require!(!farmer.is_zero(), "farmer address must not be zero");
        require!(!badge_metadata_hash.is_empty(), "empty badge metadata hash");
        require!(!self.proposals().contains_key(&proposal_id), "proposal already exists");

        let proposal = GovernanceProposal {
            proposal_id: proposal_id.clone(),
            proposal_type: PROPOSAL_TYPE_BADGE_ISSUANCE,
            target: farmer,
            bool_value: true,
            role: badge_metadata_hash,
            eta: self.blockchain().get_block_timestamp_seconds().as_u64_seconds().saturating_add(self.timelock_seconds().get()),
            executed: false,
            executed_at_timestamp: 0u64,
        };

        self.proposals().insert(proposal_id.clone(), proposal);
        self.proposal_created_event(&proposal_id, PROPOSAL_TYPE_BADGE_ISSUANCE);
    }

    /// Record caller's approval for a pending proposal. Duplicate approvals are rejected.
    #[endpoint(approveProposal)]
    fn approve_proposal(&self, proposal_id: ManagedBuffer) {
        self.require_signer();
        require!(self.proposals().contains_key(&proposal_id), "missing proposal");
        let caller = self.blockchain().get_caller();
        require!(!self.approvals(&proposal_id).contains(&caller), "ALREADY_APPROVED");
        require!(
            !self.proposals()
                .get(&proposal_id)
                .unwrap_or_else(|| sc_panic!("missing proposal"))
                .executed,
            "proposal already executed"
        );

        self.approvals(&proposal_id)
            .insert(self.blockchain().get_caller());
        self.proposal_approved_event(
            &proposal_id,
            &self.blockchain().get_caller(),
            self.approvals(&proposal_id).len() as u32,
        );
    }

    /// Executes a fully approved proposal after the timelock expires.
    ///
    /// Proposals remain executable for 30 days after `eta`.
    #[endpoint(executeProposal)]
    fn execute_proposal(&self, proposal_id: ManagedBuffer) {
        self.require_signer();

        let mut proposal = self
            .proposals()
            .get(&proposal_id)
            .unwrap_or_else(|| sc_panic!("missing proposal"));
        require!(!proposal.executed, "proposal already executed");
        require!(
            (self.approvals(&proposal_id).len() as u32) >= self.approval_threshold().get(),
            "insufficient approvals"
        );
        require!(
            self.blockchain().get_block_timestamp_seconds().as_u64_seconds() >= proposal.eta,
            "timelock not elapsed"
        );
        require!(
            self.blockchain().get_block_timestamp_seconds().as_u64_seconds() <= proposal.eta.saturating_add(2_592_000u64),
            "PROPOSAL_EXPIRED: must be executed within 30 days of timelock expiry"
        );

        if proposal.proposal_type == PROPOSAL_TYPE_PAUSE {
            self.paused().set(proposal.bool_value);
            self.pause_changed_event(proposal.bool_value);
        } else if proposal.proposal_type == PROPOSAL_TYPE_VERIFIER_ACCREDITATION {
            let accreditation = VerifierAccreditation {
                verifier: proposal.target.clone(),
                approved: proposal.bool_value,
                role: proposal.role.clone(),
                updated_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
            };
            self.verifier_accreditations()
                .insert(proposal.target.clone(), accreditation);
            self.verifier_accreditation_changed_event(
                &proposal.target,
                proposal.bool_value,
                &proposal.role,
            );
        } else if proposal.proposal_type == PROPOSAL_TYPE_BADGE_ISSUANCE {
            require!(
                !self.badge_issuances().contains_key(&proposal.target),
                "BADGE_ALREADY_ISSUED: farmer already has a badge — revoke first"
            );
            self.badge_issuances()
                .insert(proposal.target.clone(), proposal.role.clone());
            self.badge_issued_event(
                &proposal.target,
                &proposal.role,
            );
        } else {
            sc_panic!("unsupported proposal type");
        }

        proposal.executed = true;
        proposal.executed_at_timestamp = self.blockchain().get_block_timestamp_seconds().as_u64_seconds();
        self.proposals().insert(proposal_id.clone(), proposal);
        self.approvals(&proposal_id).clear();
        self.proposal_executed_event(&proposal_id);
    }

    #[view(getProposal)]
    fn get_proposal(
        &self,
        proposal_id: ManagedBuffer,
    ) -> OptionalValue<GovernanceProposal<Self::Api>> {
        match self.proposals().get(&proposal_id) {
            Some(proposal) => OptionalValue::Some(proposal),
            None => OptionalValue::None,
        }
    }

    #[view(isSigner)]
    fn is_signer(&self, signer: ManagedAddress) -> bool {
        self.signers().contains(&signer)
    }

    #[view(getPaused)]
    #[storage_mapper("paused")]
    fn paused(&self) -> SingleValueMapper<bool>;

    #[view(getApprovalThreshold)]
    #[storage_mapper("approvalThreshold")]
    fn approval_threshold(&self) -> SingleValueMapper<u32>;

    #[view(getTimelockSeconds)]
    #[storage_mapper("timelockSeconds")]
    fn timelock_seconds(&self) -> SingleValueMapper<u64>;

    #[view(getVerifierAccreditation)]
    fn get_verifier_accreditation(
        &self,
        verifier: ManagedAddress,
    ) -> OptionalValue<VerifierAccreditation<Self::Api>> {
        match self.verifier_accreditations().get(&verifier) {
            Some(accreditation) => OptionalValue::Some(accreditation),
            None => OptionalValue::None,
        }
    }

    /// Returns true if the given verifier DID holds an active (approved=true)
    /// accreditation. Used by mrv/registry to gate submitVerificationStatement().
    #[view(isAccreditedVvb)]
    fn is_accredited_vvb(&self, verifier: ManagedAddress) -> bool {
        match self.verifier_accreditations().get(&verifier) {
            Some(accreditation) => accreditation.approved,
            None => false,
        }
    }

    #[storage_mapper("signers")]
    fn signers(&self) -> UnorderedSetMapper<ManagedAddress>;

    #[storage_mapper("proposals")]
    fn proposals(&self) -> MapMapper<ManagedBuffer, GovernanceProposal<Self::Api>>;

    #[storage_mapper("approvals")]
    fn approvals(&self, proposal_id: &ManagedBuffer) -> UnorderedSetMapper<ManagedAddress>;

    #[storage_mapper("verifierAccreditations")]
    fn verifier_accreditations(&self) -> MapMapper<ManagedAddress, VerifierAccreditation<Self::Api>>;

    /// Stores the governance-approved badge metadata hash for each farmer.
    ///
    /// Each farmer may have at most one active badge issuance record.
    #[storage_mapper("badgeIssuances")]
    fn badge_issuances(&self) -> MapMapper<ManagedAddress, ManagedBuffer>;

    /// Returns the governance-approved badge metadata hash for a farmer, if present.
    #[view(getBadgeIssuance)]
    fn get_badge_issuance(&self, farmer: ManagedAddress) -> OptionalValue<ManagedBuffer> {
        match self.badge_issuances().get(&farmer) {
            Some(hash) => OptionalValue::Some(hash),
            None => OptionalValue::None,
        }
    }

    #[event("signerAdded")]
    fn signer_added_event(&self, #[indexed] signer: &ManagedAddress);

    #[event("signerRemoved")]
    fn signer_removed_event(&self, #[indexed] signer: &ManagedAddress);

    #[event("thresholdChanged")]
    fn threshold_changed_event(&self, approval_threshold: u32);

    #[event("timelockChanged")]
    fn timelock_changed_event(&self, timelock_seconds: u64);

    #[event("proposalCreated")]
    fn proposal_created_event(
        &self,
        #[indexed] proposal_id: &ManagedBuffer,
        proposal_type: u8,
    );

    #[event("proposalApproved")]
    fn proposal_approved_event(
        &self,
        #[indexed] proposal_id: &ManagedBuffer,
        #[indexed] signer: &ManagedAddress,
        approval_count: u32,
    );

    #[event("proposalExecuted")]
    fn proposal_executed_event(&self, #[indexed] proposal_id: &ManagedBuffer);

    #[event("pauseChanged")]
    fn pause_changed_event(&self, paused: bool);

    /// Emitted when a Green Badge issuance is executed.
    #[event("badgeIssued")]
    fn badge_issued_event(
        &self,
        #[indexed] farmer: &ManagedAddress,
        badge_metadata_hash: &ManagedBuffer,
    );

    #[event("verifierAccreditationChanged")]
    fn verifier_accreditation_changed_event(
        &self,
        #[indexed] verifier: &ManagedAddress,
        #[indexed] approved: bool,
        role: &ManagedBuffer,
    );

    /// Reverts if the caller is not in the signer set.
    fn require_signer(&self) {
        let caller = self.blockchain().get_caller();
        require!(self.signers().contains(&caller), "caller not signer");
    }

    /// Proposes adding a GSOC verifier.
    ///
    /// GSOC verifiers are distinct from VVB accreditations and remain scoped
    /// to the GSOC methodology and jurisdiction data stored in this contract.
    #[endpoint(proposeGsocVerifier)]
    fn propose_gsoc_verifier(
        &self,
        verifier_did: ManagedAddress,
        credentials_cid: ManagedBuffer,
        jurisdiction: ManagedBuffer,
    ) {
        self.require_signer();
        require!(!verifier_did.is_zero(), "empty verifier_did");
        require!(!credentials_cid.is_empty(), "empty credentials_cid");
        require!(!jurisdiction.is_empty(), "empty jurisdiction");

        let proposal_id = self.next_gsoc_verifier_proposal_id().get();
        self.next_gsoc_verifier_proposal_id().set(proposal_id + 1);

        let eta = self.blockchain().get_block_timestamp_seconds().as_u64_seconds().saturating_add(self.timelock_seconds().get());

        self.gsoc_verifier_proposals().insert(proposal_id, GsocVerifierProposal {
            verifier_did: verifier_did.clone(),
            credentials_cid: credentials_cid.clone(),
            jurisdiction: jurisdiction.clone(),
            eta,
            executed: false,
        });

        self.gsoc_verifier_proposed_event(&verifier_did, &jurisdiction);
    }

    /// Approves a GSOC verifier proposal.
    #[endpoint(approveGsocVerifierProposal)]
    fn approve_gsoc_verifier_proposal(&self, proposal_id: u64) {
        self.require_signer();
        require!(
            self.gsoc_verifier_proposals().contains_key(&proposal_id),
            "proposal not found"
        );
        let proposal = self.gsoc_verifier_proposals().get(&proposal_id)
            .unwrap_or_else(|| sc_panic!("missing proposal"));
        require!(!proposal.executed, "proposal already executed");

        let caller = self.blockchain().get_caller();
        require!(
            !self.gsoc_verifier_approvals(proposal_id).contains(&caller),
            "ALREADY_APPROVED"
        );
        self.gsoc_verifier_approvals(proposal_id).insert(caller);
    }

    /// Executes a proposed GSOC verifier addition after the timelock and
    /// approval threshold are satisfied.
    #[endpoint(executeGsocVerifierProposal)]
    fn execute_gsoc_verifier_proposal(&self, proposal_id: u64) {
        self.require_signer();
        require!(
            self.gsoc_verifier_proposals().contains_key(&proposal_id),
            "proposal not found"
        );

        let proposal = self.gsoc_verifier_proposals().get(&proposal_id)
            .unwrap_or_else(|| sc_panic!("missing proposal"));
        require!(!proposal.executed, "proposal already executed");
        let current_ts = self.blockchain().get_block_timestamp_seconds().as_u64_seconds();
        require!(
            current_ts >= proposal.eta,
            "timelock not expired"
        );
        // GSOC verifier proposals expire after 30 days, matching the
        // main governance proposal expiry window.
        require!(
            current_ts <= proposal.eta.saturating_add(2_592_000u64),
            "GSOC_PROPOSAL_EXPIRED: must be executed within 30 days of timelock expiry"
        );

        let approval_count = self.gsoc_verifier_approvals(proposal_id).len();
        let threshold = self.approval_threshold().get();
        require!(
            approval_count >= threshold as usize,
            "INSUFFICIENT_APPROVALS: need at least threshold approvals for GSOC verifier proposals"
        );

        let verifier_did = proposal.verifier_did.clone();

        self.gsoc_verifier_registry().insert(verifier_did.clone(), GsocVerifierEntry {
            credentials_cid: proposal.credentials_cid,
            jurisdiction: proposal.jurisdiction,
            registered_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
            approved: true,
        });

        self.gsoc_verifier_approvals(proposal_id).clear();
        self.gsoc_verifier_proposals().remove(&proposal_id);
        self.gsoc_verifier_added_event(&verifier_did);
    }

    /// Removes a GSOC verifier immediately (no timelock). Owner-only.
    #[only_owner]
    #[endpoint(removeGsocVerifier)]
    fn remove_gsoc_verifier(&self, verifier_did: ManagedAddress) {
        require!(
            self.gsoc_verifier_registry().contains_key(&verifier_did),
            "verifier not found"
        );

        self.gsoc_verifier_registry().entry(verifier_did.clone()).and_modify(|r| {
            r.approved = false;
        });

        self.gsoc_verifier_removed_event(&verifier_did);
    }

    #[view(isGsocVerifierApproved)]
    fn is_gsoc_verifier_approved(&self, verifier_did: ManagedAddress) -> bool {
        match self.gsoc_verifier_registry().get(&verifier_did) {
            Some(entry) => entry.approved,
            None => false,
        }
    }

    #[storage_mapper("gsocVerifierRegistry")]
    fn gsoc_verifier_registry(&self) -> MapMapper<ManagedAddress, GsocVerifierEntry<Self::Api>>;

    #[storage_mapper("gsocVerifierProposals")]
    fn gsoc_verifier_proposals(&self) -> MapMapper<u64, GsocVerifierProposal<Self::Api>>;

    /// Stores per-proposal approvals for GSOC verifier proposals.
    #[storage_mapper("gsocVerifierApprovals")]
    fn gsoc_verifier_approvals(&self, proposal_id: u64) -> UnorderedSetMapper<ManagedAddress>;

    #[storage_mapper("nextGsocVerifierProposalId")]
    fn next_gsoc_verifier_proposal_id(&self) -> SingleValueMapper<u64>;

    #[event("gsocVerifierProposed")]
    fn gsoc_verifier_proposed_event(
        &self,
        #[indexed] verifier_did: &ManagedAddress,
        jurisdiction: &ManagedBuffer,
    );

    #[event("gsocVerifierAdded")]
    fn gsoc_verifier_added_event(&self, #[indexed] verifier_did: &ManagedAddress);

    #[event("gsocVerifierRemoved")]
    fn gsoc_verifier_removed_event(&self, #[indexed] verifier_did: &ManagedAddress);

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    /// Upgrades storage layout version if needed and preserves existing state.
    #[upgrade]
    fn upgrade(&self) {
        let current = self.storage_version().get();
        if current < 1u32 {
            self.storage_version().set(1u32);
        }
    }
}
