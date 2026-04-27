#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod drwa_asset_manager_proxy;

use drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy;

use drwa_common::{
    DrwaCallerDomain, DrwaHolderMirror, DrwaSyncEnvelope, DrwaSyncOperation, DrwaSyncOperationType,
    push_len_prefixed, require_valid_aml_status, require_valid_kyc_status, require_valid_token_id,
};

const POLICY_REGISTRY_READ_GAS_BUDGET: u64 = 20_000_000;
const POLICY_REGISTRY_READ_GAS_SAFETY_BUFFER: u64 = 1;
const WIND_DOWN_STATUS_NONE: u8 = 0;
const WIND_DOWN_STATUS_INITIATED: u8 = 1;
const WIND_DOWN_STATUS_COMPLETED: u8 = 2;
const WIND_DOWN_STATUS_CANCELLED: u8 = 3;
const WIND_DOWN_EVIDENCE_CID_MAX_LEN: usize = 256;
const ASSET_LEGAL_BINDING_HASH_LEN: usize = 32;

/// Stores the regulated asset metadata associated with a token identifier.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct AssetRecord<M: ManagedTypeApi> {
    pub token_id: ManagedBuffer<M>,
    pub carrier_type: ManagedBuffer<M>,
    pub asset_class: ManagedBuffer<M>,
    pub policy_id: ManagedBuffer<M>,
    pub regulated: bool,
    pub policy_version_at_register: u64,
    /// MiCA orderly wind-down flag. Once true, the Go transfer gate restricts
    /// transfers to issuer-only (redemption). Appended at struct tail for
    /// backwards-compatible deserialization of existing records.
    pub wind_down_initiated: bool,
    /// Block round at which wind-down was initiated; zero if not initiated.
    pub wind_down_round: u64,
}

/// Hash-only legal/custody binding pack for an off-chain RWA asset file.
///
/// The referenced documents remain off-chain under the regulated asset
/// authority. On chain we keep only fixed-size commitments and the valuation
/// authority address so token-to-asset truth can be audited without storing
/// legal documents or regulated personal data directly in contract storage.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct AssetLegalCustodyPack<M: ManagedTypeApi> {
    pub legal_pack_hash: ManagedBuffer<M>,
    pub custody_attestation_hash: ManagedBuffer<M>,
    pub insurance_ref_hash: ManagedBuffer<M>,
    pub valuation_authority: ManagedAddress<M>,
    pub redemption_terms_hash: ManagedBuffer<M>,
    pub asset_state_proof_hash: ManagedBuffer<M>,
    pub bound_round: u64,
}

/// Event payload emitted when legal/custody commitments are attached.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct AssetLegalCustodyEventPayload<M: ManagedTypeApi> {
    pub legal_pack_hash: ManagedBuffer<M>,
    pub custody_attestation_hash: ManagedBuffer<M>,
    pub insurance_ref_hash: ManagedBuffer<M>,
    pub valuation_authority: ManagedAddress<M>,
    pub redemption_terms_hash: ManagedBuffer<M>,
    pub asset_state_proof_hash: ManagedBuffer<M>,
}

/// Event payload emitted when holder compliance data is updated.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct HolderComplianceEventPayload<M: ManagedTypeApi> {
    pub holder_policy_version: u64,
    pub policy_version_evaluated: u64,
    pub kyc_status: ManagedBuffer<M>,
    pub aml_status: ManagedBuffer<M>,
    pub investor_class: ManagedBuffer<M>,
    pub jurisdiction_code: ManagedBuffer<M>,
    pub expiry_round: u64,
    pub transfer_locked: bool,
    pub receive_locked: bool,
    pub auditor_authorized: bool,
}

/// Manages regulated asset registration and per-holder, per-token compliance
/// state. Syncs both asset records and holder compliance mirrors to the native
/// DRWA layer on every mutation.
///
/// Governance is transferable via a propose-accept pattern with a time-limited
/// acceptance window.
#[multiversx_sc::contract]
pub trait DrwaAssetManager: drwa_common::DrwaGovernanceModule {
    /// Initializes the contract with the governance address.
    #[init]
    fn init(&self, governance: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        self.governance().set(&governance);
        self.storage_version().set(1u32);
    }

    /// Sets the policy-registry contract address consulted for on-chain token
    /// policy existence checks.
    #[endpoint(setPolicyRegistryAddress)]
    fn set_policy_registry_address(&self, policy_registry: ManagedAddress) {
        self.require_governance_or_owner();
        require!(
            !policy_registry.is_zero(),
            "policy registry address must not be zero"
        );

        self.policy_registry_address().set(&policy_registry);
        self.drwa_policy_registry_address_set_event(&policy_registry);
    }

    /// Registers a new regulated asset and syncs it to the native mirror.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// Stores `regulated = true` for the new asset.
    /// Reverts if `token_id` is invalid or the asset is already registered.
    #[endpoint(registerAsset)]
    fn register_asset(
        &self,
        token_id: ManagedBuffer,
        carrier_type: ManagedBuffer,
        asset_class: ManagedBuffer,
        policy_id: ManagedBuffer,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();

        self.require_valid_token_id(&token_id);
        self.require_policy_id_matches_token_id(&policy_id, &token_id);
        let policy_version_at_register = self.require_token_policy_registered(&token_id);
        require!(
            self.asset(&token_id).is_empty(),
            "asset already registered - use an upgrade endpoint to modify"
        );

        self.asset(&token_id).set(AssetRecord {
            token_id: token_id.clone(),
            carrier_type,
            asset_class,
            policy_id: policy_id.clone(),
            regulated: true,
            policy_version_at_register,
            wind_down_initiated: false,
            wind_down_round: 0,
        });
        self.drwa_asset_registered_event(&token_id, &policy_id, true);

        let next_version = self
            .asset_record_version(&token_id)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));
        self.asset_record_version(&token_id).set(next_version);

        // Format discriminator byte 0x00 = delimiter format (token_id:policy_id).
        // The Go-side decoder reads byte[0] to select the parser:
        //   0x00 = delimiter format, 0x01 = JSON format (used by wind-down).
        let mut body = ManagedBuffer::new();
        body.append_bytes(&[0x00u8]); // delimiter format discriminator
        body.append(&token_id);
        body.append_bytes(b":");
        body.append(&policy_id);

        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::AssetRecord,
            token_id: token_id.clone(),
            holder: ManagedAddress::default(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::AssetManager, operations)
    }

    /// Attaches a hash-only legal/custody binding pack to an existing asset.
    ///
    /// This does not store legal documents, custody contracts, insurance
    /// papers, redemption terms, or valuation files on chain. It stores fixed
    /// 32-byte commitments plus the valuation authority address so the
    /// off-chain pack can be independently audited and projection layers can
    /// fail closed when the binding is absent.
    #[endpoint(attachAssetLegalCustodyPack)]
    fn attach_asset_legal_custody_pack(
        &self,
        token_id: ManagedBuffer,
        legal_pack_hash: ManagedBuffer,
        custody_attestation_hash: ManagedBuffer,
        insurance_ref_hash: ManagedBuffer,
        valuation_authority: ManagedAddress,
        redemption_terms_hash: ManagedBuffer,
        asset_state_proof_hash: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_valid_token_id(&token_id);
        require!(!self.asset(&token_id).is_empty(), "ASSET_NOT_REGISTERED");
        self.require_valid_asset_binding_hash(&legal_pack_hash);
        self.require_valid_asset_binding_hash(&custody_attestation_hash);
        self.require_valid_asset_binding_hash(&insurance_ref_hash);
        self.require_valid_asset_binding_hash(&redemption_terms_hash);
        self.require_valid_asset_binding_hash(&asset_state_proof_hash);
        require!(
            !valuation_authority.is_zero(),
            "VALUATION_AUTHORITY_MUST_NOT_BE_ZERO"
        );

        let pack = AssetLegalCustodyPack {
            legal_pack_hash: legal_pack_hash.clone(),
            custody_attestation_hash: custody_attestation_hash.clone(),
            insurance_ref_hash: insurance_ref_hash.clone(),
            valuation_authority: valuation_authority.clone(),
            redemption_terms_hash: redemption_terms_hash.clone(),
            asset_state_proof_hash: asset_state_proof_hash.clone(),
            bound_round: self.blockchain().get_block_round(),
        };
        self.asset_legal_custody_pack(&token_id).set(pack);
        self.drwa_asset_legal_custody_pack_attached_event(
            &token_id,
            &AssetLegalCustodyEventPayload {
                legal_pack_hash,
                custody_attestation_hash,
                insurance_ref_hash,
                valuation_authority,
                redemption_terms_hash,
                asset_state_proof_hash,
            },
        );
    }

    /// Writes per-holder, per-token compliance state and syncs it to the
    /// native mirror.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// `auditor_authorized` is attestation-owned and must remain `false` on
    /// this path; the attestation contract is the only authority that may
    /// promote or revoke auditor authorization.
    /// Increments the holder policy version monotonically.
    /// Reverts if `holder` is the zero address, `token_id` is invalid,
    /// `expiry_round` is in the past unless it is `0`, or the native mirror
    /// sync fails.
    #[endpoint(syncHolderCompliance)]
    fn sync_holder_compliance(
        &self,
        token_id: ManagedBuffer,
        holder: ManagedAddress,
        kyc_status: ManagedBuffer,
        aml_status: ManagedBuffer,
        investor_class: ManagedBuffer,
        jurisdiction_code: ManagedBuffer,
        expiry_round: u64,
        transfer_locked: bool,
        receive_locked: bool,
        auditor_authorized: bool,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        require!(!holder.is_zero(), "ZERO_ADDRESS: holder must not be zero");

        self.require_valid_token_id(&token_id);
        require!(
            !self.asset(&token_id).is_empty(),
            "asset not registered: use registerAsset first"
        );
        let policy_version_evaluated = self.require_token_policy_registered(&token_id);
        require!(
            !auditor_authorized,
            "AUDITOR_AUTHORIZATION_ATTESTATION_OWNED: use attestation::recordAttestation"
        );
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
        if !jurisdiction_code.is_empty() {
            let len = jurisdiction_code.len();
            require!(len <= 64, "jurisdiction_code is too long");
            let mut bytes = [0u8; 64];
            jurisdiction_code.load_slice(0, &mut bytes[..len]);
            for &b in &bytes[..len] {
                require!(
                    b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-',
                    "jurisdiction_code contains invalid characters"
                );
            }
        }

        let current_round = self.blockchain().get_block_round();
        require!(
            expiry_round == 0 || expiry_round > current_round,
            "expiry_round must be in the future or 0 for permanent"
        );

        if !self.holder_mirror(&token_id, &holder).is_empty() {
            let current = self.holder_mirror(&token_id, &holder).get();
            if current.kyc_status == kyc_status
                && current.aml_status == aml_status
                && current.investor_class == investor_class
                && current.jurisdiction_code == jurisdiction_code
                && current.expiry_round == expiry_round
                && current.transfer_locked == transfer_locked
                && current.receive_locked == receive_locked
                && current.auditor_authorized == auditor_authorized
            {
                return self.emit_sync_noop_envelope(DrwaCallerDomain::AssetManager);
            }
        }

        let next_version = self
            .holder_policy_version(&token_id, &holder)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));

        let mirror = DrwaHolderMirror {
            holder_policy_version: next_version,
            kyc_status,
            aml_status,
            investor_class,
            jurisdiction_code,
            expiry_round,
            transfer_locked,
            receive_locked,
            auditor_authorized,
        };

        self.holder_mirror(&token_id, &holder).set(mirror.clone());
        self.holder_policy_version(&token_id, &holder)
            .set(next_version);
        self.drwa_holder_compliance_event(
            &token_id,
            &holder,
            &HolderComplianceEventPayload {
                holder_policy_version: next_version,
                policy_version_evaluated,
                kyc_status: mirror.kyc_status.clone(),
                aml_status: mirror.aml_status.clone(),
                investor_class: mirror.investor_class.clone(),
                jurisdiction_code: mirror.jurisdiction_code.clone(),
                expiry_round: mirror.expiry_round,
                transfer_locked: mirror.transfer_locked,
                receive_locked: mirror.receive_locked,
                auditor_authorized: mirror.auditor_authorized,
            },
        );

        let body = self.serialize_holder(&mirror, policy_version_evaluated);
        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::HolderMirror,
            token_id: token_id.clone(),
            holder: holder.clone(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::AssetManager, operations)
    }

    /// Updates the carrier_type, asset_class, and policy_id of an existing
    /// registered asset and syncs the updated record to the native mirror.
    /// Does not re-register or change the `regulated` flag.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if the asset is not registered.
    #[endpoint(updateAsset)]
    fn update_asset(
        &self,
        token_id: ManagedBuffer,
        carrier_type: ManagedBuffer,
        asset_class: ManagedBuffer,
        policy_id: ManagedBuffer,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        self.require_valid_token_id(&token_id);
        self.require_policy_id_matches_token_id(&policy_id, &token_id);
        require!(
            !self.asset(&token_id).is_empty(),
            "asset not registered: use registerAsset first"
        );
        self.require_token_policy_registered(&token_id);

        let current = self.asset(&token_id).get();
        if current.carrier_type == carrier_type
            && current.asset_class == asset_class
            && current.policy_id == policy_id
        {
            return self.emit_sync_noop_envelope(DrwaCallerDomain::AssetManager);
        }

        self.asset(&token_id).update(|record| {
            record.carrier_type = carrier_type;
            record.asset_class = asset_class;
            record.policy_id = policy_id;
        });

        let record = self.asset(&token_id).get();
        self.drwa_asset_updated_event(&token_id, &record.policy_id);

        let next_version = self
            .asset_record_version(&token_id)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));
        self.asset_record_version(&token_id).set(next_version);

        // Format discriminator byte 0x00 = delimiter format (token_id:policy_id).
        let mut body = ManagedBuffer::new();
        body.append_bytes(&[0x00u8]); // delimiter format discriminator
        body.append(&token_id);
        body.append_bytes(b":");
        body.append(&record.policy_id);

        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::AssetRecord,
            token_id: token_id.clone(),
            holder: ManagedAddress::default(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::AssetManager, operations)
    }

    // ── MiCA Orderly Wind-Down ──────────────────────────────────────────

    /// Initiates orderly wind-down for a regulated asset (MiCA Art. 47).
    ///
    /// Once initiated, the transfer gate in the Go layer will only allow
    /// transfers TO the issuer address (redemption). Peer-to-peer transfers
    /// are denied with `DRWA_WIND_DOWN_ACTIVE`.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if the asset is not registered or wind-down was already initiated.
    #[endpoint(initiateWindDown)]
    fn initiate_wind_down(&self, token_id: ManagedBuffer) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        self.require_valid_token_id(&token_id);
        require!(!self.asset(&token_id).is_empty(), "ASSET_NOT_REGISTERED");

        let mut record = self.asset(&token_id).get();
        let current_status = self.current_wind_down_status(&token_id);
        require!(
            current_status != WIND_DOWN_STATUS_INITIATED,
            "WIND_DOWN_ALREADY_INITIATED"
        );
        require!(
            current_status != WIND_DOWN_STATUS_COMPLETED,
            "WIND_DOWN_ALREADY_COMPLETED"
        );

        // MiCA Art. 47: instead of scanning all holder mirrors (unbounded),
        // the global wind-down flag delegates transfer-lock enforcement to
        // the Go transfer gate.
        record.wind_down_initiated = true;
        let wind_down_round = self.blockchain().get_block_round();
        record.wind_down_round = wind_down_round;
        self.asset(&token_id).set(record);
        self.wind_down_status(&token_id)
            .set(WIND_DOWN_STATUS_INITIATED);
        self.wind_down_status_round(&token_id).set(wind_down_round);
        self.wind_down_evidence_cid(&token_id).clear();

        self.drwa_wind_down_initiated_event(&token_id);

        self.emit_wind_down_sync_envelope(
            token_id,
            WIND_DOWN_STATUS_INITIATED,
            wind_down_round,
            true,
        )
    }

    /// Completes an initiated wind-down. Completion keeps the global transfer
    /// lock active and records a bounded evidence CID for the legal/operator
    /// completion package.
    #[endpoint(completeWindDown)]
    fn complete_wind_down(
        &self,
        token_id: ManagedBuffer,
        completion_evidence_cid: ManagedBuffer,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        self.require_valid_token_id(&token_id);
        self.require_valid_wind_down_evidence_cid(&completion_evidence_cid);
        require!(!self.asset(&token_id).is_empty(), "ASSET_NOT_REGISTERED");
        require!(
            self.current_wind_down_status(&token_id) == WIND_DOWN_STATUS_INITIATED,
            "WIND_DOWN_NOT_INITIATED"
        );

        let current_round = self.blockchain().get_block_round();
        self.wind_down_status(&token_id)
            .set(WIND_DOWN_STATUS_COMPLETED);
        self.wind_down_status_round(&token_id).set(current_round);
        self.wind_down_evidence_cid(&token_id)
            .set(completion_evidence_cid.clone());
        self.asset(&token_id).update(|record| {
            record.wind_down_initiated = true;
        });

        self.drwa_wind_down_completed_event(&token_id, &completion_evidence_cid);
        self.emit_wind_down_sync_envelope(token_id, WIND_DOWN_STATUS_COMPLETED, current_round, true)
    }

    /// Cancels an initiated wind-down only with an explicit legal basis CID.
    /// Cancellation clears the global transfer lock in the native mirror.
    #[endpoint(cancelWindDown)]
    fn cancel_wind_down(
        &self,
        token_id: ManagedBuffer,
        legal_basis_cid: ManagedBuffer,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        self.require_valid_token_id(&token_id);
        self.require_valid_wind_down_evidence_cid(&legal_basis_cid);
        require!(!self.asset(&token_id).is_empty(), "ASSET_NOT_REGISTERED");
        require!(
            self.current_wind_down_status(&token_id) == WIND_DOWN_STATUS_INITIATED,
            "WIND_DOWN_NOT_INITIATED"
        );

        let current_round = self.blockchain().get_block_round();
        self.wind_down_status(&token_id)
            .set(WIND_DOWN_STATUS_CANCELLED);
        self.wind_down_status_round(&token_id).set(current_round);
        self.wind_down_evidence_cid(&token_id)
            .set(legal_basis_cid.clone());
        self.asset(&token_id).update(|record| {
            record.wind_down_initiated = false;
        });

        self.drwa_wind_down_cancelled_event(&token_id, &legal_basis_cid);
        self.emit_wind_down_sync_envelope(
            token_id,
            WIND_DOWN_STATUS_CANCELLED,
            current_round,
            false,
        )
    }

    /// Returns whether wind-down has been initiated for the given token.
    #[view(isWindDownInitiated)]
    fn is_wind_down_initiated(&self, token_id: ManagedBuffer) -> bool {
        if self.asset(&token_id).is_empty() {
            return false;
        }
        self.asset(&token_id).get().wind_down_initiated
    }

    #[view(getWindDownStatusCode)]
    fn get_wind_down_status_code(&self, token_id: ManagedBuffer) -> u8 {
        self.current_wind_down_status(&token_id)
    }

    #[view(getWindDownStatusRound)]
    fn get_wind_down_status_round(&self, token_id: ManagedBuffer) -> u64 {
        self.wind_down_status_round(&token_id).get()
    }

    #[view(getWindDownEvidenceCid)]
    fn get_wind_down_evidence_cid(&self, token_id: ManagedBuffer) -> ManagedBuffer {
        self.wind_down_evidence_cid(&token_id).get()
    }

    /// Emits when orderly wind-down is initiated for an asset.
    #[event("drwaWindDownInitiated")]
    fn drwa_wind_down_initiated_event(&self, #[indexed] token_id: &ManagedBuffer);

    #[event("drwaWindDownCompleted")]
    fn drwa_wind_down_completed_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        evidence_cid: &ManagedBuffer,
    );

    #[event("drwaWindDownCancelled")]
    fn drwa_wind_down_cancelled_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        legal_basis_cid: &ManagedBuffer,
    );

    /// Maps a token identifier to its regulated asset record.
    #[view(getAsset)]
    #[storage_mapper("asset")]
    fn asset(&self, token_id: &ManagedBuffer) -> SingleValueMapper<AssetRecord<Self::Api>>;

    #[view(getWindDownStatusStorage)]
    #[storage_mapper("windDownStatus")]
    fn wind_down_status(&self, token_id: &ManagedBuffer) -> SingleValueMapper<u8>;

    #[view(getWindDownStatusRoundStorage)]
    #[storage_mapper("windDownStatusRound")]
    fn wind_down_status_round(&self, token_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[view(getWindDownEvidenceCidStorage)]
    #[storage_mapper("windDownEvidenceCid")]
    fn wind_down_evidence_cid(&self, token_id: &ManagedBuffer) -> SingleValueMapper<ManagedBuffer>;

    #[view(getAssetLegalCustodyPack)]
    #[storage_mapper("assetLegalCustodyPack")]
    fn asset_legal_custody_pack(
        &self,
        token_id: &ManagedBuffer,
    ) -> SingleValueMapper<AssetLegalCustodyPack<Self::Api>>;

    /// Returns the holder compliance mirror for a given (token_id, holder) pair.
    #[view(getHolderMirror)]
    fn get_holder_mirror(
        &self,
        token_id: ManagedBuffer,
        holder: ManagedAddress,
    ) -> DrwaHolderMirror<Self::Api> {
        require!(
            !self.holder_mirror(&token_id, &holder).is_empty(),
            "holder mirror not found"
        );
        self.holder_mirror(&token_id, &holder).get()
    }

    /// Maps (token_id, holder) to the holder's compliance mirror state.
    #[storage_mapper("holderMirror")]
    fn holder_mirror(
        &self,
        token_id: &ManagedBuffer,
        holder: &ManagedAddress,
    ) -> SingleValueMapper<DrwaHolderMirror<Self::Api>>;

    /// Monotonically increasing version counter per `(token_id, holder)` pair,
    /// used for staleness detection.
    #[storage_mapper("holderPolicyVersion")]
    fn holder_policy_version(
        &self,
        token_id: &ManagedBuffer,
        holder: &ManagedAddress,
    ) -> SingleValueMapper<u64>;

    /// Monotonically increasing version counter per token asset record,
    /// used for staleness detection on native mirror sync.
    #[storage_mapper("assetRecordVersion")]
    fn asset_record_version(&self, token_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[view(getPolicyRegistryAddress)]
    #[storage_mapper("policyRegistryAddress")]
    fn policy_registry_address(&self) -> SingleValueMapper<ManagedAddress>;

    /// Emits when an asset record is created.
    #[event("drwaAssetRegistered")]
    fn drwa_asset_registered_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] policy_id: &ManagedBuffer,
        #[indexed] regulated: bool,
    );

    /// Emits when an asset record is updated.
    #[event("drwaAssetUpdated")]
    fn drwa_asset_updated_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] policy_id: &ManagedBuffer,
    );

    #[event("drwaPolicyRegistryAddressSet")]
    fn drwa_policy_registry_address_set_event(&self, #[indexed] policy_registry: &ManagedAddress);

    #[event("drwaAssetLegalCustodyPackAttached")]
    fn drwa_asset_legal_custody_pack_attached_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        payload: &AssetLegalCustodyEventPayload<Self::Api>,
    );

    /// Emits when holder compliance data is written.
    #[event("drwaHolderCompliance")]
    fn drwa_holder_compliance_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] holder: &ManagedAddress,
        payload: &HolderComplianceEventPayload<Self::Api>,
    );

    /// Serializes holder compliance data in the binary field order consumed by
    /// the native mirror.
    fn serialize_holder(
        &self,
        holder: &DrwaHolderMirror<Self::Api>,
        policy_version_evaluated: u64,
    ) -> ManagedBuffer {
        let mut result = ManagedBuffer::new();
        result.append_bytes(&holder.holder_policy_version.to_be_bytes());
        push_len_prefixed(&mut result, &holder.kyc_status);
        push_len_prefixed(&mut result, &holder.aml_status);
        push_len_prefixed(&mut result, &holder.investor_class);
        push_len_prefixed(&mut result, &holder.jurisdiction_code);
        result.append_bytes(&holder.expiry_round.to_be_bytes());
        result.append_bytes(&[holder.transfer_locked as u8]);
        result.append_bytes(&[holder.receive_locked as u8]);
        result.append_bytes(&[holder.auditor_authorized as u8]);
        result.append_bytes(&policy_version_evaluated.to_be_bytes());
        result
    }

    /// Validates the token identifier format accepted by this contract.
    fn require_valid_token_id(&self, token_id: &ManagedBuffer) {
        require_valid_token_id(token_id);
    }

    fn current_wind_down_status(&self, token_id: &ManagedBuffer) -> u8 {
        if !self.wind_down_status(token_id).is_empty() {
            return self.wind_down_status(token_id).get();
        }
        if !self.asset(token_id).is_empty() && self.asset(token_id).get().wind_down_initiated {
            return WIND_DOWN_STATUS_INITIATED;
        }
        WIND_DOWN_STATUS_NONE
    }

    fn require_valid_wind_down_evidence_cid(&self, cid: &ManagedBuffer) {
        let len = cid.len();
        require!(len > 0, "WIND_DOWN_EVIDENCE_REQUIRED");
        require!(
            len <= WIND_DOWN_EVIDENCE_CID_MAX_LEN,
            "WIND_DOWN_EVIDENCE_TOO_LONG"
        );
        let mut bytes = [0u8; WIND_DOWN_EVIDENCE_CID_MAX_LEN];
        cid.load_slice(0, &mut bytes[..len]);
        for &b in &bytes[..len] {
            require!(
                b.is_ascii_alphanumeric()
                    || b == b'.'
                    || b == b'_'
                    || b == b'-'
                    || b == b':'
                    || b == b'/',
                "WIND_DOWN_EVIDENCE_INVALID"
            );
        }
    }

    fn require_valid_asset_binding_hash(&self, hash: &ManagedBuffer) {
        require!(
            hash.len() == ASSET_LEGAL_BINDING_HASH_LEN,
            "ASSET_BINDING_HASH_MUST_BE_32_BYTES"
        );
    }

    fn emit_wind_down_sync_envelope(
        &self,
        token_id: ManagedBuffer,
        status: u8,
        status_round: u64,
        wind_down_initiated: bool,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let next_version = self
            .asset_record_version(&token_id)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));
        self.asset_record_version(&token_id).set(next_version);

        let mut body = ManagedBuffer::new();
        body.append_bytes(&[0x01u8]); // JSON format discriminator.
        body.append_bytes(b"{\"wind_down_initiated\":");
        if wind_down_initiated {
            body.append_bytes(b"true");
        } else {
            body.append_bytes(b"false");
        }
        body.append_bytes(b",\"wind_down_round\":");
        self.append_u64_decimal(&mut body, status_round);
        body.append_bytes(b",\"wind_down_status\":\"");
        self.append_wind_down_status_label(&mut body, status);
        body.append_bytes(b"\",\"global_transfer_lock\":");
        if wind_down_initiated {
            body.append_bytes(b"true");
        } else {
            body.append_bytes(b"false");
        }
        body.append_bytes(b"}");

        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::AssetRecord,
            token_id: token_id.clone(),
            holder: ManagedAddress::default(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::AssetManager, operations)
    }

    fn append_wind_down_status_label(&self, out: &mut ManagedBuffer, status: u8) {
        match status {
            WIND_DOWN_STATUS_INITIATED => out.append_bytes(b"initiated"),
            WIND_DOWN_STATUS_COMPLETED => out.append_bytes(b"completed"),
            WIND_DOWN_STATUS_CANCELLED => out.append_bytes(b"cancelled"),
            _ => out.append_bytes(b"none"),
        }
    }

    fn append_u64_decimal(&self, out: &mut ManagedBuffer, value: u64) {
        let mut digits = [0u8; 20];
        let mut pos = 20usize;
        let mut val = value;
        if val == 0 {
            pos -= 1;
            digits[pos] = b'0';
        } else {
            while val > 0 {
                pos -= 1;
                digits[pos] = b'0' + (val % 10) as u8;
                val /= 10;
            }
        }
        out.append_bytes(&digits[pos..20]);
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

    fn require_policy_id_matches_token_id(
        &self,
        policy_id: &ManagedBuffer,
        token_id: &ManagedBuffer,
    ) {
        let len = policy_id.len();
        require!(len <= 128, "policy_id is too long");
        let mut bytes = [0u8; 128];
        policy_id.load_slice(0, &mut bytes[..len]);
        require!(
            !bytes[..len].contains(&b':'),
            "policy_id must not contain ':'"
        );
        require!(policy_id == token_id, "policy_id must equal token_id");
    }

    fn require_token_policy_registered(&self, token_id: &ManagedBuffer) -> u64 {
        require!(
            !self.policy_registry_address().is_empty(),
            "policy registry address not configured"
        );

        let policy_registry = self.policy_registry_address().get();
        let gas_for_query = self.policy_registry_read_gas_budget();
        let version = self
            .tx()
            .to(&policy_registry)
            .gas(gas_for_query)
            .typed(DrwaPolicyRegistryProxy)
            .token_policy_version(token_id)
            .returns(ReturnsResult)
            .sync_call_readonly();

        require!(
            version > 0,
            "token policy not registered: setTokenPolicy must be called first"
        );
        version
    }

    fn policy_registry_read_gas_budget(&self) -> u64 {
        let gas_left = self.blockchain().get_gas_left();
        require!(
            gas_left > POLICY_REGISTRY_READ_GAS_SAFETY_BUFFER,
            "insufficient gas for policy registry read"
        );

        let available_for_read = gas_left - POLICY_REGISTRY_READ_GAS_SAFETY_BUFFER;
        if available_for_read < POLICY_REGISTRY_READ_GAS_BUDGET {
            available_for_read
        } else {
            POLICY_REGISTRY_READ_GAS_BUDGET
        }
    }
}
