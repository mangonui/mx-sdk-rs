#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod drwa_asset_manager_proxy;

use drwa_common::{
    DrwaCallerDomain, DrwaHolderMirror, DrwaSyncEnvelope, DrwaSyncOperation, DrwaSyncOperationType,
    push_len_prefixed, require_valid_aml_status, require_valid_kyc_status,
    require_valid_token_id,
};

/// Stores the regulated asset metadata associated with a token identifier.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct AssetRecord<M: ManagedTypeApi> {
    pub token_id: ManagedBuffer<M>,
    pub carrier_type: ManagedBuffer<M>,
    pub asset_class: ManagedBuffer<M>,
    pub policy_id: ManagedBuffer<M>,
    pub regulated: bool,
    /// MiCA orderly wind-down flag. Once true, the Go transfer gate restricts
    /// transfers to issuer-only (redemption). Appended at struct tail for
    /// backwards-compatible deserialization of existing records.
    pub wind_down_initiated: bool,
    /// Block round at which wind-down was initiated; zero if not initiated.
    pub wind_down_round: u64,
}

/// Event payload emitted when holder compliance data is updated.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct HolderComplianceEventPayload<M: ManagedTypeApi> {
    pub holder_policy_version: u64,
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
        require!(
            !policy_id.to_boxed_bytes().as_slice().contains(&b':'),
            "policy_id must not contain ':'"
        );
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
            wind_down_initiated: false,
            wind_down_round: 0,
        });
        self.drwa_asset_registered_event(&token_id, &policy_id, true);

        let next_version = self.asset_record_version(&token_id).get() + 1;
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

    /// Writes per-holder, per-token compliance state and syncs it to the
    /// native mirror.
    ///
    /// Access is limited to the governance address or the contract owner.
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
        require_valid_kyc_status(&kyc_status);
        require_valid_aml_status(&aml_status);
        if !investor_class.is_empty() {
            let bytes = investor_class.to_boxed_bytes();
            for &b in bytes.as_slice() {
                require!(
                    b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-',
                    "investor_class contains invalid characters"
                );
            }
        }
        if !jurisdiction_code.is_empty() {
            let bytes = jurisdiction_code.to_boxed_bytes();
            for &b in bytes.as_slice() {
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

        let next_version = self.holder_policy_version(&token_id, &holder).get() + 1;

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

        let body = self.serialize_holder(&mirror);
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
        require!(
            !policy_id.to_boxed_bytes().as_slice().contains(&b':'),
            "policy_id must not contain ':'"
        );
        require!(
            !self.asset(&token_id).is_empty(),
            "asset not registered: use registerAsset first"
        );

        self.asset(&token_id).update(|record| {
            record.carrier_type = carrier_type;
            record.asset_class = asset_class;
            record.policy_id = policy_id;
        });

        let record = self.asset(&token_id).get();
        self.drwa_asset_updated_event(&token_id, &record.policy_id);

        let next_version = self.asset_record_version(&token_id).get() + 1;
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
        require!(
            !self.asset(&token_id).is_empty(),
            "ASSET_NOT_REGISTERED"
        );

        let mut record = self.asset(&token_id).get();
        require!(!record.wind_down_initiated, "WIND_DOWN_ALREADY_INITIATED");

        // MiCA Art. 47: instead of scanning all holder mirrors (unbounded),
        // the global wind-down flag delegates transfer-lock enforcement to
        // the Go transfer gate.
        record.wind_down_initiated = true;
        record.wind_down_round = self.blockchain().get_block_round();
        self.asset(&token_id).set(record);

        self.drwa_wind_down_initiated_event(&token_id);

        let next_version = self.asset_record_version(&token_id).get() + 1;
        self.asset_record_version(&token_id).set(next_version);

        // Format discriminator byte:
        //   0x00 = delimiter format (token_id:policy_id) used by registerAsset/updateAsset
        //   0x01 = JSON format used by wind-down and other structured payloads
        // The Go-side decoder reads the first byte to select the parser.
        let mut body = ManagedBuffer::new();
        body.append_bytes(&[0x01u8]); // JSON format discriminator
        body.append_bytes(b"{\"wind_down_initiated\":true,\"wind_down_round\":");
        let round = self.blockchain().get_block_round();
        // u64-to-decimal without alloc (no_std constraint).
        let mut digits = [0u8; 20];
        let mut pos = 20usize;
        let mut val = round;
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
        body.append_bytes(&digits[pos..20]);
        body.append_bytes(b",\"global_transfer_lock\":true}");

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

    /// Returns whether wind-down has been initiated for the given token.
    #[view(isWindDownInitiated)]
    fn is_wind_down_initiated(&self, token_id: ManagedBuffer) -> bool {
        if self.asset(&token_id).is_empty() {
            return false;
        }
        self.asset(&token_id).get().wind_down_initiated
    }

    /// Emits when orderly wind-down is initiated for an asset.
    #[event("drwaWindDownInitiated")]
    fn drwa_wind_down_initiated_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
    );

    /// Maps a token identifier to its regulated asset record.
    #[view(getAsset)]
    #[storage_mapper("asset")]
    fn asset(&self, token_id: &ManagedBuffer) -> SingleValueMapper<AssetRecord<Self::Api>>;

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
    fn serialize_holder(&self, holder: &DrwaHolderMirror<Self::Api>) -> ManagedBuffer {
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
        result
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
