#![no_std]
//! DRWA Attestation Contract — Auditor credential management.
//!
//! ## Access Model
//! Unlike identity-registry, policy-registry, and asset-manager, this contract
//! intentionally does NOT inherit `DrwaGovernanceModule`. It has its own
//! auditor propose-accept pattern because the auditor role is semantically
//! distinct from governance — an auditor provides independent third-party
//! verification, which requires separation from the governance authority that
//! controls token policies and holder compliance.
//!
//! Access model: `owner` (contract deployer) + `auditor` (proposed + accepted).
//! No `governance` address exists on this contract.

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod drwa_attestation_proxy;

use drwa_common::{
    DrwaCallerDomain, DrwaHolderAuditorAuthorization, DrwaSyncEnvelope, DrwaSyncOperation,
    DrwaSyncOperationType, build_sync_hook_payload, invoke_drwa_sync_hook, require_valid_token_id,
    serialize_sync_envelope_payload,
};
use multiversx_sc::api::HandleConstraints;

/// Maximum number of block rounds during which a proposed auditor may accept
/// the role transfer.
const PENDING_AUDITOR_ACCEPTANCE_ROUNDS: u64 = 1_000;

/// Stores the latest attestation recorded for a subject and token pair.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct AttestationRecord<M: ManagedTypeApi> {
    pub token_id: ManagedBuffer<M>,
    pub subject: ManagedAddress<M>,
    pub attestation_type: ManagedBuffer<M>,
    pub evidence_hash: ManagedBuffer<M>,
    pub approved: bool,
    pub attested_round: u64,
}

/// Event payload emitted when an attestation is recorded or revoked.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct AttestationEventPayload<M: ManagedTypeApi> {
    pub attestation_type: ManagedBuffer<M>,
    pub approved: bool,
    pub attested_round: u64,
}

/// Manages auditor attestations (approve/revoke) for holder-token pairs and
/// syncs the resulting auditor-authorization state to the native DRWA mirror.
///
/// The auditor role is transferable via a propose-accept pattern with a
/// time-limited acceptance window.
#[multiversx_sc::contract]
pub trait DrwaAttestation {
    /// Initializes the contract with the auditor address.
    #[init]
    fn init(&self, auditor: ManagedAddress) {
        require!(!auditor.is_zero(), "auditor must not be zero");
        self.auditor().set(auditor);
        self.storage_version().set(1u32);
    }

    /// Proposes a new auditor address.
    ///
    /// Only the contract owner may call this endpoint.
    /// The proposed address must call `acceptAuditor` within
    /// `PENDING_AUDITOR_ACCEPTANCE_ROUNDS` to finalize the transfer.
    #[only_owner]
    #[endpoint(setAuditor)]
    fn set_auditor(&self, auditor: ManagedAddress) {
        require!(!auditor.is_zero(), "auditor must not be zero");
        let expires_at_round = self
            .blockchain()
            .get_block_round()
            .checked_add(PENDING_AUDITOR_ACCEPTANCE_ROUNDS)
            .unwrap_or_else(|| sc_panic!("auditor acceptance round overflow"));
        self.pending_auditor().set(&auditor);
        self.pending_auditor_expires_at_round()
            .set(expires_at_round);
        self.drwa_auditor_proposed_event(&auditor);
    }

    /// Accepts a pending auditor transfer.
    ///
    /// Only the proposed auditor address may call this endpoint before the
    /// acceptance window expires. On success, the pending state is cleared and
    /// the active auditor address is updated.
    #[endpoint(acceptAuditor)]
    fn accept_auditor(&self) {
        require!(
            !self.pending_auditor().is_empty(),
            "pending auditor not set"
        );

        let caller = self.blockchain().get_caller();
        let pending = self.pending_auditor().get();
        let expires_at_round = self.pending_auditor_expires_at_round().get();
        require!(
            self.blockchain().get_block_round() <= expires_at_round,
            "pending auditor acceptance expired"
        );
        require!(caller == pending, "caller not pending auditor");

        self.auditor().set(&pending);
        self.pending_auditor().clear();
        self.pending_auditor_expires_at_round().clear();
        self.drwa_auditor_accepted_event(&pending);
    }

    /// Revokes the current auditor address, clearing all auditor and pending
    /// auditor state. Only the contract owner may call this.
    #[only_owner]
    #[endpoint(revokeAuditor)]
    fn revoke_auditor(&self) {
        let current = self.auditor().get();
        self.drwa_auditor_revoked_event(&current);
        self.pending_auditor().clear();
        self.pending_auditor_expires_at_round().clear();
        self.auditor().clear();
    }

    /// Records or replaces an attestation for a subject on a given token.
    ///
    /// Only the current auditor may call this endpoint.
    /// Syncs the resulting auditor-authorization state to the native mirror.
    /// Reverts if the caller is not the auditor, `subject` is the zero
    /// address, or `token_id` fails validation.
    #[endpoint(recordAttestation)]
    fn record_attestation(
        &self,
        token_id: ManagedBuffer,
        subject: ManagedAddress,
        attestation_type: ManagedBuffer,
        evidence_hash: ManagedBuffer,
        approved: bool,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let caller = self.blockchain().get_caller();
        require!(caller == self.auditor().get(), "caller not auditor");
        require!(!subject.is_zero(), "ZERO_ADDRESS: subject must not be zero");
        self.require_valid_token_id(&token_id);

        let record = AttestationRecord {
            token_id: token_id.clone(),
            subject: subject.clone(),
            attestation_type,
            evidence_hash,
            approved,
            attested_round: self.blockchain().get_block_round(),
        };

        // Capture overwrite flag before replacing; event emitted after set
        // so indexers can correlate with the subsequent Recorded event.
        let is_overwrite = !self.attestation(&token_id, &subject).is_empty();
        if is_overwrite {
            let current = self.attestation(&token_id, &subject).get();
            if current.attestation_type == record.attestation_type
                && current.evidence_hash == record.evidence_hash
                && current.approved == record.approved
            {
                return self.emit_sync_noop_envelope(DrwaCallerDomain::Attestation);
            }
        }
        self.attestation(&token_id, &subject).set(record.clone());
        if is_overwrite {
            self.drwa_attestation_overwritten_event(&token_id, &subject, &caller);
        }
        self.drwa_attestation_recorded_event(
            &token_id,
            &subject,
            &caller,
            &AttestationEventPayload {
                attestation_type: record.attestation_type.clone(),
                approved: record.approved,
                attested_round: record.attested_round,
            },
        );
        self.emit_auditor_authorization_sync(token_id, subject, approved)
    }

    /// Revokes an existing attestation by setting `approved = false` and
    /// syncing the revocation to the native mirror.
    ///
    /// Only the current auditor may call this endpoint.
    /// Reverts if the caller is not the auditor or the attestation does not
    /// exist.
    #[endpoint(revokeAttestation)]
    fn revoke_attestation(
        &self,
        token_id: ManagedBuffer,
        subject: ManagedAddress,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let caller = self.blockchain().get_caller();
        require!(caller == self.auditor().get(), "caller not auditor");
        require!(!subject.is_zero(), "subject address must not be zero");
        self.require_valid_token_id(&token_id);
        require!(
            !self.attestation(&token_id, &subject).is_empty(),
            "attestation does not exist"
        );

        if !self.attestation(&token_id, &subject).get().approved {
            return self.emit_sync_noop_envelope(DrwaCallerDomain::Attestation);
        }

        let mut attestation_type = ManagedBuffer::new();
        self.attestation(&token_id, &subject).update(|record| {
            attestation_type = record.attestation_type.clone();
            record.approved = false;
        });

        self.drwa_attestation_recorded_event(
            &token_id,
            &subject,
            &caller,
            &AttestationEventPayload {
                attestation_type,
                approved: false,
                attested_round: self.blockchain().get_block_round(),
            },
        );
        self.emit_auditor_authorization_sync(token_id, subject, false)
    }

    /// Maps (token_id, subject) to the latest attestation record.
    #[view(getAttestation)]
    #[storage_mapper("attestation")]
    fn attestation(
        &self,
        token_id: &ManagedBuffer,
        subject: &ManagedAddress,
    ) -> SingleValueMapper<AttestationRecord<Self::Api>>;

    /// The active auditor address authorized to record and revoke attestations.
    #[view(getAuditor)]
    #[storage_mapper("auditor")]
    fn auditor(&self) -> SingleValueMapper<ManagedAddress>;

    /// The proposed auditor address awaiting acceptance.
    #[storage_mapper("pendingAuditor")]
    fn pending_auditor(&self) -> SingleValueMapper<ManagedAddress>;

    /// Block round after which the pending auditor proposal expires.
    #[storage_mapper("pendingAuditorExpiresAtRound")]
    fn pending_auditor_expires_at_round(&self) -> SingleValueMapper<u64>;

    /// Monotonically increasing version counter per `(token_id, subject)` pair,
    /// used for staleness detection.
    #[storage_mapper("holderAuditorAuthorizationVersion")]
    fn holder_auditor_authorization_version(
        &self,
        token_id: &ManagedBuffer,
        subject: &ManagedAddress,
    ) -> SingleValueMapper<u64>;

    /// Emits when a new auditor is proposed.
    #[event("drwaAuditorProposed")]
    fn drwa_auditor_proposed_event(&self, #[indexed] auditor: &ManagedAddress);

    /// Emits when a pending auditor accepts the role.
    #[event("drwaAuditorAccepted")]
    fn drwa_auditor_accepted_event(&self, #[indexed] auditor: &ManagedAddress);

    /// Emits when the auditor address is revoked by the owner.
    #[event("drwaAuditorRevoked")]
    fn drwa_auditor_revoked_event(&self, #[indexed] previous_auditor: &ManagedAddress);

    /// Emits when an existing attestation is overwritten by a new one.
    /// Indexers can diff this against the subsequent `drwaAttestationRecorded`
    /// event to detect what changed.
    #[event("drwaAttestationOverwritten")]
    fn drwa_attestation_overwritten_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] subject: &ManagedAddress,
        #[indexed] auditor: &ManagedAddress,
    );

    /// Emits when an attestation is recorded or revoked.
    #[event("drwaAttestationRecorded")]
    fn drwa_attestation_recorded_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] subject: &ManagedAddress,
        #[indexed] auditor: &ManagedAddress,
        payload: &AttestationEventPayload<Self::Api>,
    );

    /// Builds, stores, and emits the holder auditor-authorization sync payload
    /// sent to the native mirror.
    fn emit_auditor_authorization_sync(
        &self,
        token_id: ManagedBuffer,
        subject: ManagedAddress,
        approved: bool,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let next_version = self
            .holder_auditor_authorization_version(&token_id, &subject)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));
        let authorization = DrwaHolderAuditorAuthorization {
            holder_auditor_authorization_version: next_version,
            auditor_authorized: approved,
        };

        self.holder_auditor_authorization_version(&token_id, &subject)
            .set(next_version);

        let mut body = ManagedBuffer::new();
        body.append_bytes(
            &authorization
                .holder_auditor_authorization_version
                .to_be_bytes(),
        );
        body.append_bytes(&[authorization.auditor_authorized as u8]);

        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::HolderAuditorAuthorization,
            token_id: token_id.clone(),
            holder: subject.clone(),
            version: next_version,
            body,
        });

        let caller_domain = DrwaCallerDomain::Attestation;
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

    fn emit_sync_noop_envelope(
        &self,
        caller_domain: DrwaCallerDomain,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let operations = ManagedVec::new();
        let payload_hash = self
            .crypto()
            .keccak256(serialize_sync_envelope_payload(&caller_domain, &operations))
            .as_managed_buffer()
            .clone();

        DrwaSyncEnvelope {
            schema_version: drwa_common::DRWA_SYNC_ENVELOPE_SCHEMA_VERSION,
            caller_domain,
            payload_hash,
            operations,
            pre_recovery_state_hash: ManagedBuffer::new(),
            recovery_scope: ManagedVec::new(),
        }
    }

    /// Validates the token identifier format accepted by this contract.
    fn require_valid_token_id(&self, token_id: &ManagedBuffer) {
        require_valid_token_id(token_id);
    }

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
