#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod drwa_identity_registry_proxy;

use drwa_common::{
    DrwaCallerDomain, DrwaHolderProfile, DrwaSyncEnvelope, DrwaSyncOperation,
    DrwaSyncOperationType, push_len_prefixed, require_valid_aml_status, require_valid_kyc_status,
};

const DEFAULT_IDENTITY_VALIDITY_ROUNDS: u64 = 10_000;
const MAX_IDENTITY_VALIDITY_ROUNDS: u64 = 100_000;
const IDENTITY_COMMITMENT_HASH_LEN: usize = 32;

/// Stores the identity data tracked for a holder address.
///
/// The `subject` field is stored in the value as well as used as the storage
/// key so off-chain consumers can read it without reconstructing the key.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct IdentityRecord<M: ManagedTypeApi> {
    pub subject: ManagedAddress<M>,
    pub legal_name: ManagedBuffer<M>,
    pub jurisdiction_code: ManagedBuffer<M>,
    pub registration_number: ManagedBuffer<M>,
    pub entity_type: ManagedBuffer<M>,
    pub kyc_status: ManagedBuffer<M>,
    pub aml_status: ManagedBuffer<M>,
    pub investor_class: ManagedBuffer<M>,
    pub expiry_round: u64,
}

/// Forward-only privacy-preserving identity anchor.
///
/// The hash is expected to commit to the off-chain legal/KYC payload using a
/// domain-separated preimage maintained by the regulated identity authority.
/// The contract stores only the commitment, not the raw legal name or
/// registration number.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct IdentityPrivacyCommitment<M: ManagedTypeApi> {
    pub subject: ManagedAddress<M>,
    pub identity_ref_hash: ManagedBuffer<M>,
    pub committed_round: u64,
}

/// Manages per-holder identity records (KYC, AML, investor class, jurisdiction)
/// and syncs holder-profile state to the native DRWA mirror on every mutation.
///
/// Governance is transferable via a propose-accept pattern with a time-limited
/// acceptance window.
#[multiversx_sc::contract]
pub trait DrwaIdentityRegistry: drwa_common::DrwaGovernanceModule {
    /// Initializes the contract with the governance address.
    #[init]
    fn init(&self, governance: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        self.governance().set(governance);
        self.default_validity_rounds()
            .set(DEFAULT_IDENTITY_VALIDITY_ROUNDS);
        self.max_validity_rounds().set(MAX_IDENTITY_VALIDITY_ROUNDS);
        self.storage_version().set(1u32);
    }

    /// Registers a new identity for `subject`.
    ///
    /// Sets both KYC and AML status to `"pending"` and sets the initial expiry
    /// round to the current block round plus `DEFAULT_IDENTITY_VALIDITY_ROUNDS`.
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if `subject` is the zero address or an identity already exists.
    #[endpoint(registerIdentity)]
    fn register_identity(
        &self,
        subject: ManagedAddress,
        legal_name: ManagedBuffer,
        jurisdiction_code: ManagedBuffer,
        registration_number: ManagedBuffer,
        entity_type: ManagedBuffer,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        require!(!subject.is_zero(), "subject must not be zero");
        require!(
            !jurisdiction_code.is_empty(),
            "jurisdiction_code is required"
        );
        require!(
            self.identity(&subject).is_empty(),
            "IDENTITY_ALREADY_REGISTERED: use updateComplianceStatus to modify existing identity"
        );

        let record = IdentityRecord {
            subject: subject.clone(),
            legal_name,
            jurisdiction_code,
            registration_number,
            entity_type,
            kyc_status: ManagedBuffer::from(b"pending"),
            aml_status: ManagedBuffer::from(b"pending"),
            investor_class: ManagedBuffer::new(),
            expiry_round: self.default_expiry_round(),
        };

        self.identity(&subject).set(record.clone());
        let envelope = self.emit_holder_profile_sync(subject.clone(), &record);
        self.drwa_identity_registered_event(
            &subject,
            &record.jurisdiction_code,
            &record.entity_type,
        );
        envelope
    }

    /// Registers an identity using only a 32-byte off-chain identity
    /// commitment instead of raw legal name / registration-number payloads.
    ///
    /// This is the forward-safe path for new DRWA deployments. It preserves
    /// the existing holder-profile sync semantics while leaving historical
    /// raw-PII records to a separate migration/disclosure process.
    #[endpoint(registerIdentityCommitment)]
    fn register_identity_commitment(
        &self,
        subject: ManagedAddress,
        identity_ref_hash: ManagedBuffer,
        jurisdiction_code: ManagedBuffer,
        entity_type: ManagedBuffer,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        require!(!subject.is_zero(), "subject must not be zero");
        self.require_valid_identity_commitment_hash(&identity_ref_hash);
        require!(
            !jurisdiction_code.is_empty(),
            "jurisdiction_code is required"
        );
        require!(
            self.identity(&subject).is_empty(),
            "IDENTITY_ALREADY_REGISTERED: use updateComplianceStatus to modify existing identity"
        );

        let record = IdentityRecord {
            subject: subject.clone(),
            legal_name: ManagedBuffer::new(),
            jurisdiction_code,
            registration_number: ManagedBuffer::new(),
            entity_type,
            kyc_status: ManagedBuffer::from(b"pending"),
            aml_status: ManagedBuffer::from(b"pending"),
            investor_class: ManagedBuffer::new(),
            expiry_round: self.default_expiry_round(),
        };
        let commitment = IdentityPrivacyCommitment {
            subject: subject.clone(),
            identity_ref_hash: identity_ref_hash.clone(),
            committed_round: self.blockchain().get_block_round(),
        };

        self.identity(&subject).set(record.clone());
        self.identity_privacy_commitment(&subject).set(commitment);
        let envelope = self.emit_holder_profile_sync(subject.clone(), &record);
        self.drwa_identity_commitment_registered_event(
            &subject,
            &identity_ref_hash,
            &record.jurisdiction_code,
            &record.entity_type,
        );
        envelope
    }

    /// Updates the compliance fields for an existing identity and syncs the
    /// holder profile to the native mirror.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if the subject is missing, `expiry_round` is in the past
    /// unless it is `0`, or `expiry_round` exceeds the configured maximum
    /// validity window.
    #[endpoint(updateComplianceStatus)]
    fn update_compliance_status(
        &self,
        subject: ManagedAddress,
        kyc_status: ManagedBuffer,
        aml_status: ManagedBuffer,
        investor_class: ManagedBuffer,
        expiry_round: u64,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        require!(!subject.is_zero(), "subject must not be zero");
        require_valid_kyc_status(&kyc_status);
        require_valid_aml_status(&aml_status);
        if !investor_class.is_empty() {
            let len = investor_class.len();
            require!(len <= 64, "investor_class is too long");
            let mut bytes = [0u8; 64];
            investor_class.load_slice(0, &mut bytes[..len]);
            for &b in &bytes[..len] {
                require!(
                    b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-',
                    "investor_class contains invalid characters"
                );
            }
        }
        require!(
            !self.identity(&subject).is_empty(),
            "identity not registered - call registerIdentity first"
        );
        let current_round = self.blockchain().get_block_round();
        require!(
            expiry_round == 0 || expiry_round > current_round,
            "expiry_round must be in the future or 0 for permanent"
        );
        require!(
            expiry_round == 0 || expiry_round <= self.max_expiry_round(current_round),
            "expiry_round exceeds maximum identity validity window"
        );

        let current = self.identity(&subject).get();
        if current.kyc_status == kyc_status
            && current.aml_status == aml_status
            && current.investor_class == investor_class
            && current.expiry_round == expiry_round
        {
            return self.emit_sync_noop_envelope(DrwaCallerDomain::IdentityRegistry);
        }

        self.identity(&subject).update(|record| {
            record.kyc_status = kyc_status;
            record.aml_status = aml_status;
            record.investor_class = investor_class;
            record.expiry_round = expiry_round;
        });
        let record = self.identity(&subject).get();
        let envelope = self.emit_holder_profile_sync(subject.clone(), &record);
        self.drwa_compliance_updated_event(&subject, &record.kyc_status, &record.aml_status);
        envelope
    }

    /// Deactivates an existing identity by setting both KYC and AML status to
    /// `"deactivated"`, incrementing the holder profile version, and syncing
    /// the change to the native mirror.
    ///
    /// This preserves the audit trail (the record is not deleted).
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if the identity does not exist or `subject` is the zero address.
    #[endpoint(deactivateIdentity)]
    fn deactivate_identity(&self, subject: ManagedAddress) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        require!(!subject.is_zero(), "subject address must not be zero");
        require!(
            !self.identity(&subject).is_empty(),
            "identity not registered"
        );

        let current = self.identity(&subject).get();
        if current.kyc_status == ManagedBuffer::from(b"deactivated")
            && current.aml_status == ManagedBuffer::from(b"deactivated")
            && current.investor_class.is_empty()
            && current.jurisdiction_code == ManagedBuffer::from(b"DEACTIVATED")
            && current.expiry_round == 0
        {
            return self.emit_sync_noop_envelope(DrwaCallerDomain::IdentityRegistry);
        }

        self.identity(&subject).update(|record| {
            record.kyc_status = ManagedBuffer::from(b"deactivated");
            record.aml_status = ManagedBuffer::from(b"deactivated");
            record.investor_class = ManagedBuffer::new();
            record.jurisdiction_code = ManagedBuffer::from(b"DEACTIVATED");
            record.expiry_round = 0;
        });
        let record = self.identity(&subject).get();
        let envelope = self.emit_holder_profile_sync(subject.clone(), &record);
        self.drwa_identity_deactivated_event(&subject);
        envelope
    }

    /// GDPR right-to-erasure endpoint.
    ///
    /// Zeros all PII fields (legal_name, registration_number, entity_type,
    /// investor_class) while preserving the address reference and setting
    /// jurisdiction_code to "ERASED". Both KYC and AML status are set to
    /// "deactivated" to prevent the erased identity from passing compliance
    /// checks. The holder profile is synced to the native mirror so the
    /// enforcement gate sees the deactivated status immediately.
    ///
    /// Unlike `deactivateIdentity`, this endpoint is specifically for GDPR
    /// Article 17 compliance — it removes personal data while maintaining
    /// the minimum audit trail required by the regulation.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if the identity does not exist or `subject` is the zero address.
    #[endpoint(eraseIdentity)]
    fn erase_identity(&self, subject: ManagedAddress) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        require!(!subject.is_zero(), "subject address must not be zero");
        require!(!self.identity(&subject).is_empty(), "IDENTITY_NOT_FOUND");

        let erased = IdentityRecord {
            subject: subject.clone(),
            legal_name: ManagedBuffer::new(),
            jurisdiction_code: ManagedBuffer::from(b"ERASED"),
            registration_number: ManagedBuffer::new(),
            entity_type: ManagedBuffer::new(),
            kyc_status: ManagedBuffer::from(b"deactivated"),
            aml_status: ManagedBuffer::from(b"deactivated"),
            investor_class: ManagedBuffer::new(),
            expiry_round: 0,
        };

        if self.identity(&subject).get() == erased {
            return self.emit_sync_noop_envelope(DrwaCallerDomain::IdentityRegistry);
        }

        self.identity(&subject).set(erased);
        self.identity_privacy_commitment(&subject).clear();
        let envelope = self.emit_holder_mirror_delete_sync(subject.clone());
        self.drwa_identity_erased_event(&subject);
        envelope
    }

    /// Maps a holder address to its identity record.
    #[view(getIdentity)]
    #[storage_mapper("identity")]
    fn identity(&self, subject: &ManagedAddress) -> SingleValueMapper<IdentityRecord<Self::Api>>;

    /// Maps a holder address to its privacy-preserving identity commitment.
    #[view(getIdentityPrivacyCommitment)]
    #[storage_mapper("identityPrivacyCommitment")]
    fn identity_privacy_commitment(
        &self,
        subject: &ManagedAddress,
    ) -> SingleValueMapper<IdentityPrivacyCommitment<Self::Api>>;

    /// Monotonically increasing version counter per holder, used for
    /// staleness detection.
    #[storage_mapper("holderProfileVersion")]
    fn holder_profile_version(&self, subject: &ManagedAddress) -> SingleValueMapper<u64>;

    /// Storage-backed default identity validity window (in rounds).
    /// Initialized from `DEFAULT_IDENTITY_VALIDITY_ROUNDS` during `init`.
    #[storage_mapper("default_validity_rounds")]
    fn default_validity_rounds(&self) -> SingleValueMapper<u64>;

    /// Storage-backed maximum identity validity window (in rounds).
    /// Initialized from `MAX_IDENTITY_VALIDITY_ROUNDS` during `init`.
    #[storage_mapper("max_validity_rounds")]
    fn max_validity_rounds(&self) -> SingleValueMapper<u64>;

    /// Updates the identity validity configuration.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if `default_rounds` is zero, `max_rounds` is less than
    /// `default_rounds`, or `max_rounds` exceeds the hard cap of 1,000,000.
    #[endpoint(setValidityConfig)]
    fn set_validity_config(&self, default_rounds: u64, max_rounds: u64) {
        self.require_governance_or_owner();
        require!(default_rounds > 0, "default_rounds must be positive");
        require!(
            max_rounds >= default_rounds,
            "max_rounds must be >= default_rounds"
        );
        require!(max_rounds <= 1_000_000, "max_rounds cap exceeded");
        self.default_validity_rounds().set(default_rounds);
        self.max_validity_rounds().set(max_rounds);
    }

    fn default_expiry_round(&self) -> u64 {
        self.blockchain()
            .get_block_round()
            .checked_add(self.default_validity_rounds().get())
            .unwrap_or_else(|| sc_panic!("identity expiry round overflow"))
    }

    fn max_expiry_round(&self, current_round: u64) -> u64 {
        current_round
            .checked_add(self.max_validity_rounds().get())
            .unwrap_or_else(|| sc_panic!("identity max validity round overflow"))
    }

    /// Builds, stores, and emits the holder-profile sync payload sent to the
    /// native mirror.  Delegates to `emit_sync_envelope` from drwa-common.
    fn emit_holder_profile_sync(
        &self,
        subject: ManagedAddress,
        record: &IdentityRecord<Self::Api>,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let next_version = self
            .holder_profile_version(&subject)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));
        let profile = DrwaHolderProfile {
            holder_profile_version: next_version,
            kyc_status: record.kyc_status.clone(),
            aml_status: record.aml_status.clone(),
            investor_class: record.investor_class.clone(),
            jurisdiction_code: record.jurisdiction_code.clone(),
            expiry_round: record.expiry_round,
        };

        self.holder_profile_version(&subject).set(next_version);

        let body = self.serialize_holder_profile(&profile);
        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::HolderProfile,
            token_id: ManagedBuffer::new(),
            holder: subject.clone(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::IdentityRegistry, operations)
    }

    fn emit_holder_mirror_delete_sync(
        &self,
        subject: ManagedAddress,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let next_version = self
            .holder_profile_version(&subject)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));

        self.holder_profile_version(&subject).set(next_version);

        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::HolderMirrorDelete,
            token_id: ManagedBuffer::new(),
            holder: subject,
            version: next_version,
            body: ManagedBuffer::new(),
        });

        self.emit_sync_envelope(DrwaCallerDomain::IdentityRegistry, operations)
    }

    /// Serializes the holder profile in the binary field order consumed by the
    /// native mirror.
    fn serialize_holder_profile(&self, profile: &DrwaHolderProfile<Self::Api>) -> ManagedBuffer {
        let mut result = ManagedBuffer::new();
        result.append_bytes(&profile.holder_profile_version.to_be_bytes());
        push_len_prefixed(&mut result, &profile.kyc_status);
        push_len_prefixed(&mut result, &profile.aml_status);
        push_len_prefixed(&mut result, &profile.investor_class);
        push_len_prefixed(&mut result, &profile.jurisdiction_code);
        result.append_bytes(&profile.expiry_round.to_be_bytes());
        result
    }

    // ── Domain events for indexer/notifier consumption ────────────────

    #[event("drwaIdentityRegistered")]
    fn drwa_identity_registered_event(
        &self,
        #[indexed] subject: &ManagedAddress,
        #[indexed] jurisdiction_code: &ManagedBuffer,
        #[indexed] entity_type: &ManagedBuffer,
    );

    #[event("drwaIdentityCommitmentRegistered")]
    fn drwa_identity_commitment_registered_event(
        &self,
        #[indexed] subject: &ManagedAddress,
        identity_ref_hash: &ManagedBuffer,
        #[indexed] jurisdiction_code: &ManagedBuffer,
        #[indexed] entity_type: &ManagedBuffer,
    );

    #[event("drwaComplianceUpdated")]
    fn drwa_compliance_updated_event(
        &self,
        #[indexed] subject: &ManagedAddress,
        #[indexed] kyc_status: &ManagedBuffer,
        #[indexed] aml_status: &ManagedBuffer,
    );

    #[event("drwaIdentityDeactivated")]
    fn drwa_identity_deactivated_event(&self, #[indexed] subject: &ManagedAddress);

    /// Emitted when an identity is erased for GDPR compliance.
    /// The event carries only the subject address (no PII).
    #[event("drwaIdentityErased")]
    fn drwa_identity_erased_event(&self, #[indexed] subject: &ManagedAddress);

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

    fn require_valid_identity_commitment_hash(&self, identity_ref_hash: &ManagedBuffer) {
        require!(
            identity_ref_hash.len() == IDENTITY_COMMITMENT_HASH_LEN,
            "IDENTITY_COMMITMENT_HASH_MUST_BE_32_BYTES"
        );
    }
}
