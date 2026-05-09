#![no_std]

use multiversx_sc::api::HandleConstraints;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use drwa_common::{
    DrwaCallerDomain, DrwaSyncEnvelope, DrwaSyncOperation, DrwaSyncOperationType,
    build_sync_hook_payload, invoke_drwa_sync_hook, serialize_sync_envelope_payload,
};

#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone, PartialEq, Eq)]
pub enum DrwaAuthAction<M: ManagedTypeApi> {
    Nothing,
    UpdateCallerAddress {
        domain: ManagedBuffer<M>,
        new_address: ManagedBuffer<M>,
    },
    AddSigner {
        new_signer: ManagedAddress<M>,
    },
    RemoveSigner {
        signer: ManagedAddress<M>,
    },
    ReplaceSigner {
        old_signer: ManagedAddress<M>,
        new_signer: ManagedAddress<M>,
    },
    ChangeQuorum {
        new_quorum: usize,
    },
}

#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone, PartialEq, Eq)]
pub struct DrwaAuthActionProposalEvent {
    pub created_round: u64,
    pub expiry_round: u64,
    pub timelock_rounds: u64,
}

#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone, PartialEq, Eq)]
pub struct DrwaAuthActionApprovalEvent {
    pub approvals: usize,
    pub quorum: usize,
}

#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone, PartialEq, Eq)]
pub struct DrwaAuthActionDiscardEvent {
    pub discarded_round: u64,
}

#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone, PartialEq, Eq)]
pub struct DrwaAuthorizedCallerUpdateEvent<M: ManagedTypeApi> {
    pub new_address: ManagedBuffer<M>,
    pub version: u64,
}

#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone, PartialEq, Eq)]
pub struct DrwaSignerSetEvent {
    pub signer_count: usize,
    pub quorum: usize,
}

#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone, PartialEq, Eq)]
pub struct DrwaQuorumChangeEvent {
    pub previous_quorum: usize,
    pub new_quorum: usize,
    pub signer_count: usize,
}

impl<M: ManagedTypeApi> DrwaAuthAction<M> {
    pub fn is_pending(&self) -> bool {
        !matches!(self, Self::Nothing)
    }
}

// B-03 Option A (AUD-003) — mandatory timelock + fixed-threshold floors.
//
// The approved `DRWA-Key-Rotation-Procedures.md` requires:
//   - fixed 3-of-5 quorum for signer add / revoke
//   - 24-hour timelock after threshold is reached for signer change
//   - 48-hour timelock for recovery-admin rotation
//   - no immediate emergency override exists in this contract. Incident
//     response must still pass through the quorum + timelock path unless a
//     separately audited emergency-governor contract is deployed and accepted
//     by governance.
//
// We encode the timelock at *propose* time so a later quorum change cannot
// retro-shorten the delay for a pending action. `action_approved_at_round`
// is set when the Nth signature lands and is cleared by `unsign` when the
// approval count drops back below quorum.

/// Minimum number of signers a deployment must carry. Matches the
/// procedures-doc promise of "3-of-5 governance signers." Enforced at
/// `init` AND at the `RemoveSigner` action; any ChangeQuorum that would
/// drop below `DRWA_AUTH_MIN_QUORUM` is rejected.
const DRWA_AUTH_MIN_SIGNER_COUNT: usize = 5;

/// Minimum quorum. See `DRWA_AUTH_MIN_SIGNER_COUNT`.
const DRWA_AUTH_MIN_QUORUM: usize = 3;

/// Block-round equivalent of a 24-hour timelock at ~6 seconds per round.
/// Applied by default to every admin action. Signer add / remove /
/// replace / caller-address updates and quorum changes all land on this
/// delay unless an operator explicitly configures a longer one for a
/// specific domain.
const DRWA_AUTH_TIMELOCK_DEFAULT_ROUNDS: u64 = 14_400;

/// Block-round equivalent of a 48-hour timelock. Applied when the action
/// targets the `recovery-admin` caller domain (see the match arm in
/// `propose_update_caller_address` / resolution inside `create_action`).
const DRWA_AUTH_TIMELOCK_RECOVERY_ADMIN_ROUNDS: u64 = 28_800;
const DRWA_AUTH_STORAGE_VERSION: u32 = 2;
const DRWA_EMERGENCY_OVERRIDE_POLICY: &[u8] =
    b"not_supported: all auth-admin actions require quorum and timelock";

const DRWA_BECH32_ADDRESS_LEN: usize = 62;
const DRWA_BECH32_PREFIX: &[u8] = b"erd1";
const DRWA_BECH32_DATA_CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const DRWA_BECH32_HRP: &[u8] = b"erd";
const DRWA_BECH32_POLYMOD_GENERATORS: [u32; 5] =
    [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];

// B-03: the recovery-admin caller-domain tag is `b"recovery_admin"`,
// matching `DrwaCallerDomain::RecoveryAdmin` serialization in
// `drwa_common`. We compare against the byte-array literal directly at
// the callsite because `ManagedBuffer` only implements PartialEq against
// fixed-size byte-array literals (`&[u8; N]`), not arbitrary `&[u8]`
// slice references.

#[multiversx_sc::contract]
pub trait DrwaAuthAdmin {
    #[init]
    fn init(
        &self,
        quorum: usize,
        proposal_ttl_rounds: u64,
        signers: MultiValueEncoded<ManagedAddress>,
    ) {
        require!(proposal_ttl_rounds > 0, "proposal TTL must be > 0");

        let mut signer_count = 0usize;
        for signer in signers {
            require!(!signer.is_zero(), "signer must not be zero");
            require!(!self.signers().contains(&signer), "duplicate signer");
            self.signers().insert(signer);
            signer_count += 1;
        }

        require!(signer_count > 0, "signers must not be empty");
        require!(quorum > 0, "quorum must be > 0");
        require!(quorum <= signer_count, "quorum exceeds signer count");

        // B-03 (AUD-003): the DRWA-Key-Rotation-Procedures doc commits the
        // deployment to at least 3-of-5. Reject any init that cannot honor
        // that contract. Existing deployments already at 3-of-5 or stronger
        // are unaffected; weaker configurations were never safe.
        require!(
            signer_count >= DRWA_AUTH_MIN_SIGNER_COUNT,
            "signer count below procedure floor (3-of-5)"
        );
        require!(
            quorum >= DRWA_AUTH_MIN_QUORUM,
            "quorum below procedure floor (3-of-5)"
        );

        self.quorum().set(quorum);
        self.proposal_ttl_rounds().set(proposal_ttl_rounds);
        self.next_action_id().set(1u64);
        self.storage_version().set(DRWA_AUTH_STORAGE_VERSION);
    }

    #[upgrade]
    fn upgrade(
        &self,
        quorum: usize,
        proposal_ttl_rounds: u64,
        signers: MultiValueEncoded<ManagedAddress>,
    ) {
        if self.next_action_id().is_empty() {
            self.init(quorum, proposal_ttl_rounds, signers);
            return;
        }

        let stored_version = if self.storage_version().is_empty() {
            1u32
        } else {
            self.storage_version().get()
        };

        require!(
            stored_version <= DRWA_AUTH_STORAGE_VERSION,
            "unsupported future storage version"
        );

        if stored_version < 2u32 {
            self.migrate_v1_to_v2();
        }

        self.storage_version().set(DRWA_AUTH_STORAGE_VERSION);
    }

    #[endpoint(proposeUpdateCallerAddress)]
    fn propose_update_caller_address(
        &self,
        domain: ManagedBuffer,
        new_address: ManagedBuffer,
    ) -> u64 {
        self.require_signer();
        require!(!domain.is_empty(), "domain must not be empty");
        self.require_valid_authorized_caller(&new_address);
        // B-03: caller-address updates for the recovery-admin domain carry a
        // 48-hour timelock per procedure §5.2 / §6.2. All other domains use
        // the default 24-hour delay.
        let timelock_rounds = if domain == b"recovery_admin" {
            DRWA_AUTH_TIMELOCK_RECOVERY_ADMIN_ROUNDS
        } else {
            DRWA_AUTH_TIMELOCK_DEFAULT_ROUNDS
        };
        self.create_action_with_timelock(
            DrwaAuthAction::UpdateCallerAddress {
                domain,
                new_address,
            },
            timelock_rounds,
        )
    }

    #[endpoint(proposeAddSigner)]
    fn propose_add_signer(&self, new_signer: ManagedAddress) -> u64 {
        self.require_signer();
        require!(!new_signer.is_zero(), "signer must not be zero");
        require!(
            !self.signers().contains(&new_signer),
            "signer already exists"
        );
        self.create_action_with_timelock(
            DrwaAuthAction::AddSigner { new_signer },
            DRWA_AUTH_TIMELOCK_DEFAULT_ROUNDS,
        )
    }

    #[endpoint(proposeRemoveSigner)]
    fn propose_remove_signer(&self, signer: ManagedAddress) -> u64 {
        self.require_signer();
        require!(!signer.is_zero(), "signer must not be zero");
        require!(self.signers().contains(&signer), "signer not found");
        self.create_action_with_timelock(
            DrwaAuthAction::RemoveSigner { signer },
            DRWA_AUTH_TIMELOCK_DEFAULT_ROUNDS,
        )
    }

    #[endpoint(proposeReplaceSigner)]
    fn propose_replace_signer(
        &self,
        old_signer: ManagedAddress,
        new_signer: ManagedAddress,
    ) -> u64 {
        self.require_signer();
        require!(!old_signer.is_zero(), "old signer must not be zero");
        require!(!new_signer.is_zero(), "new signer must not be zero");
        require!(self.signers().contains(&old_signer), "old signer not found");
        require!(
            !self.signers().contains(&new_signer),
            "new signer already exists"
        );
        self.create_action_with_timelock(
            DrwaAuthAction::ReplaceSigner {
                old_signer,
                new_signer,
            },
            DRWA_AUTH_TIMELOCK_DEFAULT_ROUNDS,
        )
    }

    #[endpoint(proposeChangeQuorum)]
    fn propose_change_quorum(&self, new_quorum: usize) -> u64 {
        self.require_signer();
        self.create_action_with_timelock(
            DrwaAuthAction::ChangeQuorum { new_quorum },
            DRWA_AUTH_TIMELOCK_DEFAULT_ROUNDS,
        )
    }

    #[endpoint(sign)]
    fn sign(&self, action_id: u64) {
        self.require_signer();
        self.require_pending_action(action_id);
        let caller = self.blockchain().get_caller();
        let current_count_before = self.current_action_signer_count(action_id);
        if !self.action_approved_at_round(action_id).is_empty()
            && current_count_before < self.quorum().get()
        {
            self.action_approved_at_round(action_id).clear();
        }
        if !self.action_signers(action_id).contains(&caller) {
            self.action_signers(action_id).insert(caller.clone());
            self.signer_pending_action_ids(&caller).insert(action_id);
        }
        // B-03: capture the round on which quorum is first reached; the
        // timelock window is measured from this instant, not from the
        // initial propose. If quorum is later lost via `unsign` the
        // stored round is cleared so a subsequent re-approval restarts
        // the timelock. This matches the procedure-doc intent of
        // "24-hour timelock AFTER threshold reached."
        //
        // Encoding: stored value = round + 1 so that a round-0 approval
        // is distinguishable from "never approved" (`SingleValueMapper<u64>`
        // reports a storage slot containing 0 as `is_empty()==true`, so
        // the offset ensures the slot always carries a non-zero value
        // once set).
        let current_count = self.current_action_signer_count(action_id);
        if self.action_approved_at_round(action_id).is_empty()
            && current_count >= self.quorum().get()
        {
            let current = self.blockchain().get_block_round();
            let approved_at = current
                .checked_add(1)
                .unwrap_or_else(|| sc_panic!("action approval round overflow"));
            self.action_approved_at_round(action_id).set(approved_at);
        }

        self.action_signed_event(
            action_id,
            caller,
            DrwaAuthActionApprovalEvent {
                approvals: current_count,
                quorum: self.quorum().get(),
            },
        );
    }

    #[endpoint(unsign)]
    fn unsign(&self, action_id: u64) {
        self.require_signer();
        self.require_pending_action(action_id);
        let caller = self.blockchain().get_caller();
        self.action_signers(action_id).swap_remove(&caller);
        self.signer_pending_action_ids(&caller)
            .swap_remove(&action_id);
        // B-03: if approvals drop back below quorum after this unsign,
        // invalidate the previously captured approval round so the
        // timelock cannot be satisfied by a brief-quorum / retract /
        // re-quorum dance that evades the full delay window.
        let current_count = self.current_action_signer_count(action_id);
        if current_count < self.quorum().get() {
            self.action_approved_at_round(action_id).clear();
        }

        self.action_unsigned_event(
            action_id,
            caller,
            DrwaAuthActionApprovalEvent {
                approvals: current_count,
                quorum: self.quorum().get(),
            },
        );
    }

    #[endpoint(discardAction)]
    fn discard_action(&self, action_id: u64) {
        self.require_signer();
        self.require_pending_action(action_id);
        let current_round = self.blockchain().get_block_round();
        let expiry_round = self.action_expiry_round(action_id).get();
        require!(
            current_round > expiry_round || self.action_signers(action_id).is_empty(),
            "cannot discard active action"
        );
        let caller = self.blockchain().get_caller();
        self.clear_action(action_id);
        self.action_discarded_event(
            action_id,
            caller,
            DrwaAuthActionDiscardEvent {
                discarded_round: current_round,
            },
        );
    }

    #[endpoint(performAction)]
    fn perform_action(&self, action_id: u64) -> OptionalValue<DrwaSyncEnvelope<Self::Api>> {
        self.require_signer();
        require!(
            !self.performed_action_ids().contains(&action_id),
            "action already performed"
        );
        self.require_pending_action(action_id);
        let current_round = self.blockchain().get_block_round();
        require!(
            current_round <= self.action_expiry_round(action_id).get(),
            "action expired"
        );
        require!(
            self.current_action_signer_count(action_id) >= self.quorum().get(),
            "insufficient approvals"
        );

        // B-03 (AUD-003): enforce the mandatory post-quorum timelock. The
        // storage key is only populated AFTER quorum is first reached
        // (see `sign`), so a missing entry here means the action has
        // reached `len() >= quorum` only via the post-quorum increment
        // path — defensive require! catches an impossible code path.
        //
        // C-213: this contract deliberately has no immediate emergency
        // override. Every action, including signer rotation and
        // recovery-admin rotation, must pass quorum and the stored timelock.
        // A future emergency-governor must be a separate audited contract,
        // not an implicit zero-timelock branch hidden in auth-admin.
        require!(
            !self.action_approved_at_round(action_id).is_empty(),
            "approval round not recorded; sign again after reaching quorum"
        );
        // Stored as `round + 1` (see `sign`). Subtract the offset here to
        // recover the true round at which quorum was reached.
        let approved_round = self
            .action_approved_at_round(action_id)
            .get()
            .checked_sub(1)
            .unwrap_or_else(|| sc_panic!("action approval round underflow"));
        let timelock_rounds = self.action_timelock_rounds(action_id).get();
        let executable_round = approved_round
            .checked_add(timelock_rounds)
            .unwrap_or_else(|| sc_panic!("action timelock round overflow"));
        require!(
            current_round >= executable_round,
            "timelock not elapsed: must wait 24h after quorum (48h for recovery-admin)"
        );

        let action = self.actions(action_id).get();
        let action_kind = self.action_kind(&action);
        let mut result = OptionalValue::None;

        match action {
            DrwaAuthAction::Nothing => sc_panic!("action does not exist"),
            DrwaAuthAction::UpdateCallerAddress {
                domain,
                new_address,
            } => {
                require!(!domain.is_empty(), "domain must not be empty");
                self.require_valid_authorized_caller(&new_address);

                let next_version = self
                    .authorized_caller_version(&domain)
                    .get()
                    .checked_add(1)
                    .unwrap_or_else(|| sc_panic!("version overflow"));
                self.authorized_caller(&domain).set(&new_address);
                self.authorized_caller_version(&domain).set(next_version);
                self.authorized_caller_updated_event(
                    domain.clone(),
                    DrwaAuthorizedCallerUpdateEvent {
                        new_address: new_address.clone(),
                        version: next_version,
                    },
                );

                let mut operations = ManagedVec::new();
                operations.push(DrwaSyncOperation {
                    operation_type: DrwaSyncOperationType::AuthorizedCallerUpdate,
                    token_id: domain,
                    holder: ManagedAddress::zero(),
                    version: next_version,
                    body: new_address,
                });

                result = OptionalValue::Some(
                    self.emit_sync_envelope(DrwaCallerDomain::AuthAdmin, operations),
                );
            }
            DrwaAuthAction::AddSigner { new_signer } => {
                require!(!new_signer.is_zero(), "signer must not be zero");
                require!(
                    !self.signers().contains(&new_signer),
                    "signer already exists"
                );
                self.signers().insert(new_signer.clone());
                self.signer_added_event(
                    new_signer,
                    DrwaSignerSetEvent {
                        signer_count: self.signers().len(),
                        quorum: self.quorum().get(),
                    },
                );
            }
            DrwaAuthAction::RemoveSigner { signer } => {
                require!(self.signers().contains(&signer), "signer not found");
                require!(
                    self.signers().len() > self.quorum().get(),
                    "cannot remove signer below quorum"
                );
                // B-03: procedure floor — deployment must keep at least
                // `DRWA_AUTH_MIN_SIGNER_COUNT` signers regardless of what
                // the configured quorum would otherwise permit.
                require!(
                    self.signers().len() - 1 >= DRWA_AUTH_MIN_SIGNER_COUNT,
                    "cannot drop signer count below procedure floor (3-of-5)"
                );
                self.signers().swap_remove(&signer);
                self.remove_stale_action_signer(&signer);
                self.signer_removed_event(
                    signer,
                    DrwaSignerSetEvent {
                        signer_count: self.signers().len(),
                        quorum: self.quorum().get(),
                    },
                );
            }
            DrwaAuthAction::ReplaceSigner {
                old_signer,
                new_signer,
            } => {
                require!(self.signers().contains(&old_signer), "old signer not found");
                require!(!new_signer.is_zero(), "new signer must not be zero");
                require!(
                    !self.signers().contains(&new_signer),
                    "new signer already exists"
                );
                self.signers().swap_remove(&old_signer);
                self.signers().insert(new_signer.clone());
                self.remove_stale_action_signer(&old_signer);
                self.signer_replaced_event(
                    old_signer,
                    new_signer,
                    DrwaSignerSetEvent {
                        signer_count: self.signers().len(),
                        quorum: self.quorum().get(),
                    },
                );
            }
            DrwaAuthAction::ChangeQuorum { new_quorum } => {
                require!(new_quorum > 0, "quorum must be > 0");
                require!(
                    new_quorum <= self.signers().len(),
                    "quorum exceeds signer count"
                );
                // B-03: procedure floor — quorum must remain >= 3.
                require!(
                    new_quorum >= DRWA_AUTH_MIN_QUORUM,
                    "quorum below procedure floor (3-of-5)"
                );
                let previous_quorum = self.quorum().get();
                self.quorum().set(new_quorum);
                self.quorum_changed_event(DrwaQuorumChangeEvent {
                    previous_quorum,
                    new_quorum,
                    signer_count: self.signers().len(),
                });
            }
        }

        self.performed_action_ids().insert(action_id);
        self.clear_action(action_id);
        self.action_performed_event(action_id, self.blockchain().get_caller(), action_kind);
        result
    }

    #[view(getQuorum)]
    #[storage_mapper("quorum")]
    fn quorum(&self) -> SingleValueMapper<usize>;

    #[view(getProposalTtlRounds)]
    #[storage_mapper("proposalTtlRounds")]
    fn proposal_ttl_rounds(&self) -> SingleValueMapper<u64>;

    #[view(getNextActionId)]
    #[storage_mapper("nextActionId")]
    fn next_action_id(&self) -> SingleValueMapper<u64>;

    #[view(getAction)]
    #[storage_mapper("actions")]
    fn actions(&self, action_id: u64) -> SingleValueMapper<DrwaAuthAction<Self::Api>>;

    #[view(getActionProposer)]
    #[storage_mapper("actionProposer")]
    fn action_proposer(&self, action_id: u64) -> SingleValueMapper<ManagedAddress>;

    #[view(getActionCreatedRound)]
    #[storage_mapper("actionCreatedRound")]
    fn action_created_round(&self, action_id: u64) -> SingleValueMapper<u64>;

    #[view(getActionExpiryRound)]
    #[storage_mapper("actionExpiryRound")]
    fn action_expiry_round(&self, action_id: u64) -> SingleValueMapper<u64>;

    /// B-03 (AUD-003): round at which the action's approval count first
    /// reached `quorum`. Populated by `sign` when the quorum threshold
    /// is crossed upward; cleared by `unsign` when it drops back below.
    /// `performAction` rejects when this is empty (quorum never reached)
    /// or when `current_round < approved_round + timelock_rounds`.
    ///
    /// Encoding: the stored value is `approved_round + 1`. The `+ 1`
    /// offset lets `SingleValueMapper::is_empty()` act as the "not yet
    /// approved" sentinel even when quorum is reached at block-round 0
    /// (which is legitimate in scenario tests; no effect in real chain
    /// execution where block-round 0 is the genesis slot). Consumers of
    /// this view MUST subtract 1 to recover the real approval round.
    #[view(getActionApprovedAtRound)]
    #[storage_mapper("actionApprovedAtRound")]
    fn action_approved_at_round(&self, action_id: u64) -> SingleValueMapper<u64>;

    /// B-03 (AUD-003): timelock window in block rounds that must elapse
    /// between `action_approved_at_round` and a successful
    /// `performAction`. Set at propose time and immutable thereafter
    /// so a later `ChangeQuorum` cannot retro-shorten pending actions.
    #[view(getActionTimelockRounds)]
    #[storage_mapper("actionTimelockRounds")]
    fn action_timelock_rounds(&self, action_id: u64) -> SingleValueMapper<u64>;

    #[view(getAllSigners)]
    #[storage_mapper("signers")]
    fn signers(&self) -> UnorderedSetMapper<ManagedAddress>;

    #[view(getActionSigners)]
    #[storage_mapper("actionSigners")]
    fn action_signers(&self, action_id: u64) -> UnorderedSetMapper<ManagedAddress>;

    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    #[view(getSignerPendingActionIds)]
    #[storage_mapper("signerPendingActionIds")]
    fn signer_pending_action_ids(&self, signer: &ManagedAddress) -> UnorderedSetMapper<u64>;

    #[view(getPerformedActionIds)]
    #[storage_mapper("performedActionIds")]
    fn performed_action_ids(&self) -> UnorderedSetMapper<u64>;

    #[view(getAuthorizedCaller)]
    #[storage_mapper("authorizedCaller")]
    fn authorized_caller(&self, domain: &ManagedBuffer) -> SingleValueMapper<ManagedBuffer>;

    #[view(getAuthorizedCallerVersion)]
    #[storage_mapper("authorizedCallerVersion")]
    fn authorized_caller_version(&self, domain: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[view(isEmergencyOverrideSupported)]
    fn is_emergency_override_supported(&self) -> bool {
        false
    }

    #[view(getEmergencyOverridePolicy)]
    fn get_emergency_override_policy(&self) -> ManagedBuffer {
        ManagedBuffer::from(DRWA_EMERGENCY_OVERRIDE_POLICY)
    }

    fn require_signer(&self) {
        let caller = self.blockchain().get_caller();
        require!(self.signers().contains(&caller), "caller not a signer");
    }

    fn require_pending_action(&self, action_id: u64) {
        require!(
            !self.actions(action_id).is_empty() && self.actions(action_id).get().is_pending(),
            "action does not exist"
        );
    }

    /// Legacy shim: proposals that don't specify a timelock use the
    /// default 24-hour delay. All current in-tree callers go through
    /// `create_action_with_timelock` directly; this helper remains for
    /// backward-compat with any external code that calls it.
    fn create_action(&self, action: DrwaAuthAction<Self::Api>) -> u64 {
        self.create_action_with_timelock(action, DRWA_AUTH_TIMELOCK_DEFAULT_ROUNDS)
    }

    fn create_action_with_timelock(
        &self,
        action: DrwaAuthAction<Self::Api>,
        timelock_rounds: u64,
    ) -> u64 {
        // C-213: timelock must be positive. There is no zero-timelock
        // emergency branch in this contract.
        require!(
            timelock_rounds > 0,
            "action timelock must be > 0 (emergency override not supported)"
        );

        let action_id = self.next_action_id().get();
        let next_action_id = action_id
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("action id overflow"));
        self.next_action_id().set(next_action_id);
        let action_kind = self.action_kind(&action);
        self.actions(action_id).set(action);
        let proposer = self.blockchain().get_caller();
        self.action_proposer(action_id).set(&proposer);
        let current_round = self.blockchain().get_block_round();
        self.action_created_round(action_id).set(current_round);
        let expiry_round = current_round
            .checked_add(self.proposal_ttl_rounds().get())
            .unwrap_or_else(|| sc_panic!("action expiry round overflow"));
        self.action_expiry_round(action_id).set(expiry_round);
        self.action_timelock_rounds(action_id).set(timelock_rounds);
        self.action_signers(action_id).insert(proposer.clone());
        self.signer_pending_action_ids(&proposer).insert(action_id);
        // Propose-time quorum check: the proposer counts as the first
        // approval. If `quorum == 1` the action becomes immediately
        // eligible (from a quorum standpoint) and the timelock starts
        // here; otherwise the approval round is set when the Nth
        // signer calls `sign`. Same offset-by-one encoding as `sign`.
        if self.current_action_signer_count(action_id) >= self.quorum().get() {
            let approved_at = current_round
                .checked_add(1)
                .unwrap_or_else(|| sc_panic!("action approval round overflow"));
            self.action_approved_at_round(action_id).set(approved_at);
        }
        self.action_proposed_event(
            action_id,
            proposer,
            action_kind,
            DrwaAuthActionProposalEvent {
                created_round: current_round,
                expiry_round,
                timelock_rounds,
            },
        );
        action_id
    }

    fn action_kind(&self, action: &DrwaAuthAction<Self::Api>) -> ManagedBuffer {
        match action {
            DrwaAuthAction::Nothing => ManagedBuffer::from(b"nothing"),
            DrwaAuthAction::UpdateCallerAddress { .. } => ManagedBuffer::from(b"update_caller"),
            DrwaAuthAction::AddSigner { .. } => ManagedBuffer::from(b"add_signer"),
            DrwaAuthAction::RemoveSigner { .. } => ManagedBuffer::from(b"remove_signer"),
            DrwaAuthAction::ReplaceSigner { .. } => ManagedBuffer::from(b"replace_signer"),
            DrwaAuthAction::ChangeQuorum { .. } => ManagedBuffer::from(b"change_quorum"),
        }
    }

    fn clear_action(&self, action_id: u64) {
        for signer in self.action_signers(action_id).iter() {
            self.signer_pending_action_ids(&signer)
                .swap_remove(&action_id);
        }
        self.actions(action_id).clear();
        self.action_proposer(action_id).clear();
        self.action_created_round(action_id).clear();
        self.action_expiry_round(action_id).clear();
        self.action_signers(action_id).clear();
        // B-03: also clear the new timelock tracking slots to avoid
        // orphan state after `discardAction` or `performAction`.
        self.action_approved_at_round(action_id).clear();
        self.action_timelock_rounds(action_id).clear();
    }

    fn current_action_signer_count(&self, action_id: u64) -> usize {
        let mut count = 0usize;
        for signer in self.action_signers(action_id).iter() {
            if self.signers().contains(&signer) {
                count += 1;
            }
        }
        count
    }

    fn remove_stale_action_signer(&self, signer: &ManagedAddress) {
        let mut pending_action_ids: ManagedVec<Self::Api, u64> = ManagedVec::new();
        for action_id in self.signer_pending_action_ids(signer).iter() {
            pending_action_ids.push(action_id);
        }

        for action_id in pending_action_ids.iter() {
            self.signer_pending_action_ids(signer)
                .swap_remove(&action_id);
            if self.actions(action_id).is_empty() {
                continue;
            }
            if self.action_signers(action_id).contains(signer) {
                self.action_signers(action_id).swap_remove(signer);
                if self.current_action_signer_count(action_id) < self.quorum().get() {
                    self.action_approved_at_round(action_id).clear();
                }
            }
        }
    }

    fn migrate_v1_to_v2(&self) {
        let current_round = self.blockchain().get_block_round();
        let next_action_id = self.next_action_id().get();
        for action_id in 1..next_action_id {
            if self.actions(action_id).is_empty() {
                continue;
            }

            let mut current_signer_count = 0usize;
            for signer in self.action_signers(action_id).iter() {
                self.signer_pending_action_ids(&signer).insert(action_id);
                if self.signers().contains(&signer) {
                    current_signer_count += 1;
                }
            }

            if self.action_timelock_rounds(action_id).is_empty() {
                self.action_timelock_rounds(action_id)
                    .set(DRWA_AUTH_TIMELOCK_DEFAULT_ROUNDS);
            }

            if self.action_approved_at_round(action_id).is_empty()
                && current_signer_count >= self.quorum().get()
            {
                let approved_at = current_round
                    .checked_add(1)
                    .unwrap_or_else(|| sc_panic!("action approval round overflow"));
                self.action_approved_at_round(action_id).set(approved_at);
            }
        }
    }

    fn require_valid_authorized_caller(&self, new_address: &ManagedBuffer) {
        require!(!new_address.is_empty(), "new address must not be empty");

        let len = new_address.len();
        require!(
            len <= 90,
            "new address must be a 64-char hex string or erd1 bech32 address"
        );

        let mut bytes = [0u8; 90];
        new_address.load_slice(0, &mut bytes[..len]);
        let address = &bytes[..len];

        let is_hex = len == 64 && address.iter().all(|b| b.is_ascii_hexdigit());
        let is_bech32 = self.is_valid_multiversx_bech32_address(address);

        require!(
            is_hex || is_bech32,
            "new address must be a 64-char hex string or erd1 bech32 address"
        );
    }

    fn is_valid_multiversx_bech32_address(&self, address: &[u8]) -> bool {
        if address.len() != DRWA_BECH32_ADDRESS_LEN || !address.starts_with(DRWA_BECH32_PREFIX) {
            return false;
        }

        let mut checksum = 1u32;
        for byte in DRWA_BECH32_HRP {
            checksum = self.bech32_polymod_step(checksum, u32::from(byte >> 5));
        }
        checksum = self.bech32_polymod_step(checksum, 0);
        for byte in DRWA_BECH32_HRP {
            checksum = self.bech32_polymod_step(checksum, u32::from(byte & 0x1f));
        }

        for byte in &address[DRWA_BECH32_PREFIX.len()..] {
            let Some(value) = self.bech32_charset_value(*byte) else {
                return false;
            };
            checksum = self.bech32_polymod_step(checksum, value);
        }

        checksum == 1
    }

    fn bech32_charset_value(&self, byte: u8) -> Option<u32> {
        for (idx, charset_byte) in DRWA_BECH32_DATA_CHARSET.iter().enumerate() {
            if *charset_byte == byte {
                return Some(idx as u32);
            }
        }
        None
    }

    fn bech32_polymod_step(&self, checksum: u32, value: u32) -> u32 {
        let top = checksum >> 25;
        let mut next = ((checksum & 0x01ff_ffff) << 5) ^ value;
        for (idx, generator) in DRWA_BECH32_POLYMOD_GENERATORS.iter().enumerate() {
            if ((top >> idx) & 1) == 1 {
                next ^= generator;
            }
        }
        next
    }

    fn emit_sync_envelope(
        &self,
        caller_domain: DrwaCallerDomain,
        operations: ManagedVec<DrwaSyncOperation<Self::Api>>,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let payload_hash = self
            .crypto()
            .keccak256(serialize_sync_envelope_payload(&caller_domain, &operations))
            .as_managed_buffer()
            .clone();

        let hook_payload = build_sync_hook_payload(&caller_domain, &operations, &payload_hash);
        require!(
            invoke_drwa_sync_hook(hook_payload.get_handle().get_raw_handle()) == 0,
            "native mirror sync failed"
        );

        DrwaSyncEnvelope {
            schema_version: drwa_common::DRWA_SYNC_ENVELOPE_SCHEMA_VERSION,
            caller_domain,
            payload_hash,
            operations,
            pre_recovery_state_hash: ManagedBuffer::new(),
            recovery_scope: ManagedVec::new(),
        }
    }

    #[event("drwaAuthActionProposed")]
    fn action_proposed_event(
        &self,
        #[indexed] action_id: u64,
        #[indexed] proposer: ManagedAddress,
        #[indexed] action_kind: ManagedBuffer,
        proposal: DrwaAuthActionProposalEvent,
    );

    #[event("drwaAuthActionSigned")]
    fn action_signed_event(
        &self,
        #[indexed] action_id: u64,
        #[indexed] signer: ManagedAddress,
        approval: DrwaAuthActionApprovalEvent,
    );

    #[event("drwaAuthActionUnsigned")]
    fn action_unsigned_event(
        &self,
        #[indexed] action_id: u64,
        #[indexed] signer: ManagedAddress,
        approval: DrwaAuthActionApprovalEvent,
    );

    #[event("drwaAuthActionDiscarded")]
    fn action_discarded_event(
        &self,
        #[indexed] action_id: u64,
        #[indexed] discarder: ManagedAddress,
        discard: DrwaAuthActionDiscardEvent,
    );

    #[event("drwaAuthActionPerformed")]
    fn action_performed_event(
        &self,
        #[indexed] action_id: u64,
        #[indexed] performer: ManagedAddress,
        #[indexed] action_kind: ManagedBuffer,
    );

    #[event("drwaAuthorizedCallerUpdated")]
    fn authorized_caller_updated_event(
        &self,
        #[indexed] domain: ManagedBuffer,
        update: DrwaAuthorizedCallerUpdateEvent<Self::Api>,
    );

    #[event("drwaSignerAdded")]
    fn signer_added_event(
        &self,
        #[indexed] new_signer: ManagedAddress,
        signer_set: DrwaSignerSetEvent,
    );

    #[event("drwaSignerRemoved")]
    fn signer_removed_event(
        &self,
        #[indexed] removed_signer: ManagedAddress,
        signer_set: DrwaSignerSetEvent,
    );

    #[event("drwaSignerReplaced")]
    fn signer_replaced_event(
        &self,
        #[indexed] old_signer: ManagedAddress,
        #[indexed] new_signer: ManagedAddress,
        signer_set: DrwaSignerSetEvent,
    );

    #[event("drwaQuorumChanged")]
    fn quorum_changed_event(&self, quorum_change: DrwaQuorumChangeEvent);
}
