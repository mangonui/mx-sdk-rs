#![no_std]

pub mod buffer_pool_proxy;
pub mod governance_proxy;
pub mod registry_lifecycle_proxy;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use mrv_common::resolve_storage_version_upgrade;

const MAX_GSOC_SERIALS_PER_PROJECT: u64 = 1024;

/// IME validation record used to gate dVCU issuance.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct ImeValidationRecord<M: ManagedTypeApi> {
    pub project_id: ManagedBuffer<M>,
    pub science_service_image_digest: ManagedBuffer<M>,
    pub parameter_pack_hash: ManagedBuffer<M>,
    pub calibration_dataset_hash: ManagedBuffer<M>,
    pub strata_protocol_hash: ManagedBuffer<M>,
    pub methodology_version: ManagedBuffer<M>,
    pub domain_codes: ManagedVec<M, ManagedBuffer<M>>,
    pub valid_until: u64,
    pub revoked: bool,
}

/// Maximum number of jurisdiction/domain codes carried by one IME record.
const MAX_IME_DOMAIN_CODES: usize = 64;
/// Maximum byte length for one jurisdiction/domain code.
const MAX_IME_DOMAIN_CODE_LEN: usize = 64;

/// Execution bundle fields checked against the active IME record during issuance.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct ExecutionBundleRef<M: ManagedTypeApi> {
    pub science_service_image_digest: ManagedBuffer<M>,
    pub parameter_pack_hash: ManagedBuffer<M>,
    pub calibration_dataset_hash: ManagedBuffer<M>,
    pub strata_protocol_hash: ManagedBuffer<M>,
    pub methodology_version: ManagedBuffer<M>,
}

/// Retirement record for the two-phase retirement workflow.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct RetirementRecord<M: ManagedTypeApi> {
    pub retirement_id: ManagedBuffer<M>,
    pub lot_id: ManagedBuffer<M>,
    pub project_id: ManagedBuffer<M>,
    pub amount_scaled: BigUint<M>,
    pub beneficiary: ManagedAddress<M>,
    pub status: ManagedBuffer<M>,
    pub initiated_at: u64,
    pub burn_tx_hash: ManagedBuffer<M>,
}

/// M-03 (AUD-008): non-indexed payload for the
/// `gsocSerialPartiallyRetired` event. Bundles the two BigUints that
/// event-log framing cannot carry as separate non-indexed fields
/// (framing allows exactly one non-indexed argument).
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct GsocPartialRetirementEventPayload<M: ManagedTypeApi> {
    pub amount_scaled: BigUint<M>,
    pub remaining_after: BigUint<M>,
}

#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct IssuanceLotReversalRecordedPayload<M: ManagedTypeApi> {
    pub reversed_amount_scaled: BigUint<M>,
    pub replacement_lot_id: ManagedBuffer<M>,
}

/// M-03 (AUD-008): append-only GSOC retirement-event log entry.
///
/// Written by `burn_and_retire_gsoc` on every partial or full
/// retirement. Together with `gsoc_serial_remaining` and the
/// pre-existing immutable `gsoc_serial_records`, this gives readers a
/// consistent, replay-safe view of every retirement against a serial
/// — replacing the previous in-place mutation of the remaining
/// balance (which left no audit trail and produced inconsistent
/// snapshots across two reads between retirements).
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct GsocRetirementEventRecord<M: ManagedTypeApi> {
    /// Per-serial monotonically-increasing sequence number, starting at 0.
    pub seq: u64,
    /// Amount retired in THIS event (not cumulative).
    pub amount_scaled: BigUint<M>,
    /// Remaining balance on the serial AFTER this event is applied.
    pub remaining_after: BigUint<M>,
    pub beneficiary_name: ManagedBuffer<M>,
    pub beneficiary_address: ManagedAddress<M>,
    /// Block round at which this retirement was recorded.
    pub retired_at_round: u64,
}

/// Carbon credit issuance and retirement contract.
///
/// dVCU issuance is gated by an active IME record and a committed execution
/// bundle. GSOC credits follow a parallel track with verifier validation,
/// DNA project reference, and ITMO serial uniqueness. Both tracks enforce a
/// configurable buffer pool contribution.
#[multiversx_sc::contract]
pub trait CarbonCreditModule: mrv_common::MrvGovernanceModule {
    #[init]
    fn init(&self, governance: ManagedAddress, buffer_pool_addr: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        require!(
            !buffer_pool_addr.is_zero(),
            "buffer_pool_addr must not be zero"
        );
        self.governance().set(governance);
        self.buffer_pool_addr().set(buffer_pool_addr);
        self.storage_version().set(1u32);
    }

    #[endpoint(setGovernanceReadAddress)]
    fn set_governance_read_address(&self, governance_read_address: ManagedAddress) {
        self.require_governance_or_owner();
        require!(
            !governance_read_address.is_zero(),
            "governance_read_address must not be zero"
        );
        self.governance_read_address().set(&governance_read_address);
        self.governance_read_address_updated_event(&governance_read_address);
    }

    #[endpoint(clearGovernanceReadAddress)]
    fn clear_governance_read_address(&self) {
        self.require_governance_or_owner();
        self.governance_read_address().clear();
        self.governance_read_address_cleared_event();
    }

    /// Configures the MRV registry mirror that receives terminal lot
    /// lifecycle transitions from this carbon-credit contract.
    #[endpoint(setRegistryLifecycleAddress)]
    fn set_registry_lifecycle_address(&self, registry_address: ManagedAddress) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(
            !registry_address.is_zero(),
            "registry_address must not be zero"
        );
        self.registry_lifecycle_address().set(&registry_address);
        self.registry_lifecycle_address_updated_event(&registry_address);
    }

    #[endpoint(clearRegistryLifecycleAddress)]
    fn clear_registry_lifecycle_address(&self) {
        self.require_governance_or_owner();
        self.require_not_paused();
        self.registry_lifecycle_address().clear();
        self.registry_lifecycle_address_cleared_event();
    }

    /// Configures the canonical dVCU token identifier controlled by this
    /// lifecycle contract.
    #[endpoint(setDvcuTokenId)]
    fn set_dvcu_token_id(&self, token_id: TokenIdentifier) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(token_id.is_valid_esdt_identifier(), "invalid token_id");
        self.require_dvcu_token_replacement_allowed(&token_id);
        self.dvcu_token_id().set(&token_id);
        self.dvcu_token_id_updated_event(&token_id);
    }

    /// Configures the canonical dGSC token identifier controlled by this
    /// lifecycle contract.
    #[endpoint(setDgscTokenId)]
    fn set_dgsc_token_id(&self, token_id: TokenIdentifier) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(token_id.is_valid_esdt_identifier(), "invalid token_id");
        self.require_dgsc_token_replacement_allowed(&token_id);
        self.dgsc_token_id().set(&token_id);
        self.dgsc_token_id_updated_event(&token_id);
    }

    /// Issues dVCU credits after validating the IME record, bundle binding,
    /// and jurisdiction membership for the requested period.
    #[endpoint(issueCredits)]
    fn issue_credits(
        &self,
        project_id: ManagedBuffer,
        lot_id: ManagedBuffer,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
        jurisdiction_code: ManagedBuffer,
        gross_removals_scaled: BigUint,
        buffer_pct_bps: u64,
        bundle_ref: ExecutionBundleRef<Self::Api>,
        committed_bundle_hash: ManagedBuffer,
        recipient: ManagedAddress,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!project_id.is_empty(), "empty project_id");
        require!(!lot_id.is_empty(), "empty lot_id");
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(
            gross_removals_scaled > 0u64,
            "gross_removals must be positive"
        );
        require!(
            !recipient.is_zero(),
            "ZERO_ADDRESS: recipient must not be zero"
        );
        require!(
            buffer_pct_bps > 0 && buffer_pct_bps <= 2500,
            "buffer_pct_bps must be 1-2500"
        );

        require!(
            committed_bundle_hash.len() == 32,
            "committed_bundle_hash must be 32 bytes"
        );
        require!(
            !self.issued_issuance_lot_amounts().contains_key(&lot_id),
            "issuance lot already tokenized"
        );
        let bundle_key = (pai_id.clone(), mrv_common::period_key(monitoring_period_n));
        let registered = self.committed_bundles().get(&bundle_key);
        require!(
            registered.is_some(),
            "BUNDLE_NOT_REGISTERED: call registerCommittedBundle(pai_id, period, hash) first"
        );
        let registered_hash = registered.unwrap_or_else(|| sc_panic!("BUNDLE_NOT_REGISTERED"));
        require!(
            registered_hash == committed_bundle_hash,
            "BUNDLE_HASH_MISMATCH: committed_bundle_hash does not match registered hash for this PAI/period"
        );
        require!(
            !self.bound_bundle_hashes().contains_key(&bundle_key),
            "credits already issued for this PAI/period"
        );
        self.bound_bundle_hashes()
            .insert(bundle_key, committed_bundle_hash);

        let ime = self.active_ime_record(&project_id);
        require!(!ime.is_empty(), "IME_NOT_REGISTERED");
        let ime_version = self.active_ime_record_version(&project_id).get();
        require!(ime_version > 0u64, "IME_VERSION_NOT_REGISTERED");
        let ime = ime.get();
        require!(!ime.revoked, "IME_REVOKED");
        require!(
            ime.valid_until
                > self
                    .blockchain()
                    .get_block_timestamp_seconds()
                    .as_u64_seconds(),
            "IME_EXPIRED"
        );

        require!(
            bundle_ref.science_service_image_digest == ime.science_service_image_digest,
            "IME_IMAGE_MISMATCH"
        );
        require!(
            bundle_ref.parameter_pack_hash == ime.parameter_pack_hash,
            "IME_PARAMETER_MISMATCH"
        );
        require!(
            bundle_ref.calibration_dataset_hash == ime.calibration_dataset_hash,
            "IME_CALIBRATION_MISMATCH"
        );
        require!(
            bundle_ref.strata_protocol_hash == ime.strata_protocol_hash,
            "IME_STRATA_PROTOCOL_MISMATCH"
        );
        require!(
            bundle_ref.methodology_version == ime.methodology_version,
            "IME_METHODOLOGY_MISMATCH"
        );
        let mut jurisdiction_valid = false;
        for i in 0..ime.domain_codes.len() {
            if *ime.domain_codes.get(i) == jurisdiction_code {
                jurisdiction_valid = true;
                break;
            }
        }
        require!(jurisdiction_valid, "IME_JURISDICTION_NOT_IN_DOMAIN");

        // M-05 (AUD-011): buffer is a non-permanence reserve against future
        // reversal, so rounding MUST be conservative (up). The previous floor
        // division `(gross * bps) / 10_000` accumulated dust across many
        // issuances and slowly underfunded the buffer pool.
        // `ceil(x / d)` is computed as `(x + d - 1) / d` for positive integers.
        let buffer_numerator = &gross_removals_scaled * buffer_pct_bps;
        let buffer_contribution = (buffer_numerator + 9_999u64) / 10_000u64;
        require!(
            buffer_contribution > 0u64,
            "BUFFER_ROUNDS_TO_ZERO: increase gross_removals_scaled to produce non-zero buffer"
        );
        let net_issuable = &gross_removals_scaled - &buffer_contribution;
        require!(
            net_issuable > 0u64,
            "NET_ISSUABLE_ZERO: gross_removals too small after buffer deduction"
        );

        let issuance_key = (
            project_id.clone(),
            pai_id.clone(),
            mrv_common::period_key(monitoring_period_n),
        );
        require!(
            !self.issuances().contains_key(&issuance_key),
            "credits already issued for this PAI/period"
        );
        self.issuances()
            .insert(issuance_key.clone(), net_issuable.clone());
        self.issuance_lots_by_issue_key()
            .insert(issuance_key.clone(), lot_id.clone());
        self.issued_issuance_lot_projects()
            .insert(lot_id.clone(), project_id.clone());
        self.issued_issuance_lot_amounts()
            .insert(lot_id.clone(), net_issuable.clone());
        self.issued_issuance_lot_recipients()
            .insert(lot_id.clone(), recipient.clone());
        self.issuance_ime_versions()
            .insert(issuance_key, ime_version);
        let dvcu_token_id = self.require_dvcu_token_id();
        self.send()
            .esdt_local_mint(&dvcu_token_id, 0, &net_issuable);
        self.send()
            .direct_esdt(&recipient, &dvcu_token_id, 0, &net_issuable);
        self.total_dvcu_minted()
            .update(|total| *total += &net_issuable);

        let buffer_deposit_key = (
            project_id.clone(),
            pai_id.clone(),
            mrv_common::period_key(monitoring_period_n),
        );
        self.pending_buffer_deposits()
            .insert(buffer_deposit_key, buffer_contribution.clone());

        self.credits_issued_event(&project_id, &lot_id, &pai_id, &net_issuable, &recipient);
        self.buffer_deposit_pending_event(&project_id, &pai_id, &buffer_contribution);
    }

    /// Registers the committed execution bundle hash for a PAI and monitoring period.
    #[endpoint(registerCommittedBundle)]
    fn register_committed_bundle(
        &self,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
        bundle_hash: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(bundle_hash.len() == 32, "bundle_hash must be 32 bytes");
        let key = (pai_id, mrv_common::period_key(monitoring_period_n));
        require!(
            !self.committed_bundles().contains_key(&key),
            "bundle already registered for this PAI/period"
        );
        self.committed_bundles().insert(key, bundle_hash);
    }

    /// Registers the active IME validation record for a project.
    #[endpoint(registerImeRecord)]
    fn register_ime_record(
        &self,
        project_id: ManagedBuffer,
        science_service_image_digest: ManagedBuffer,
        parameter_pack_hash: ManagedBuffer,
        calibration_dataset_hash: ManagedBuffer,
        strata_protocol_hash: ManagedBuffer,
        methodology_version: ManagedBuffer,
        valid_until: u64,
        domain_codes: MultiValueEncoded<ManagedBuffer>,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!project_id.is_empty(), "empty project_id");
        require!(
            valid_until
                > self
                    .blockchain()
                    .get_block_timestamp_seconds()
                    .as_u64_seconds(),
            "valid_until must be in the future"
        );
        require!(
            domain_codes.len() <= MAX_IME_DOMAIN_CODES,
            "too many IME domain codes"
        );

        let domain_codes_vec = domain_codes.to_vec();
        for i in 0..domain_codes_vec.len() {
            let code = domain_codes_vec.get(i);
            require!(!code.is_empty(), "empty IME domain code");
            require!(
                code.len() <= MAX_IME_DOMAIN_CODE_LEN,
                "IME domain code too long"
            );
        }

        let record = ImeValidationRecord {
            project_id: project_id.clone(),
            science_service_image_digest,
            parameter_pack_hash,
            calibration_dataset_hash,
            strata_protocol_hash,
            methodology_version,
            domain_codes: domain_codes_vec,
            valid_until,
            revoked: false,
        };

        let next_version = self.ime_record_version_count(&project_id).get() + 1u64;
        self.ime_record_versions()
            .insert((project_id.clone(), next_version), record.clone());
        self.ime_record_version_count(&project_id).set(next_version);
        self.active_ime_record_version(&project_id)
            .set(next_version);
        self.active_ime_record(&project_id).set(record);
        self.ime_registered_event(&project_id);
    }

    /// Atomically materializes a pending buffer contribution in buffer-pool
    /// and clears the local pending state only after that call succeeds.
    #[endpoint(confirmBufferDeposit)]
    fn confirm_buffer_deposit(
        &self,
        project_id: ManagedBuffer,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        let key = (
            project_id.clone(),
            pai_id,
            mrv_common::period_key(monitoring_period_n),
        );
        require!(
            self.pending_buffer_deposits().contains_key(&key),
            "no pending buffer deposit for this project/PAI/period"
        );
        let pending_amount = self.pending_buffer_deposits().get(&key).unwrap();

        self.deposit_pending_buffer_contribution(&project_id, &pending_amount, monitoring_period_n);

        self.pending_buffer_deposits().remove(&key);
        self.buffer_deposit_confirmed_event(&project_id, &pending_amount);
    }

    /// Revokes the active IME record for a project. Future issuance attempts
    /// for this project will fail until a new record is registered.
    #[endpoint(revokeImeRecord)]
    fn revoke_ime_record(&self, project_id: ManagedBuffer) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(
            !self.active_ime_record(&project_id).is_empty(),
            "IME not registered"
        );
        self.active_ime_record(&project_id)
            .update(|r| r.revoked = true);
        self.ime_revoked_event(&project_id);
    }

    /// Starts a retirement record that can later be burned or reverted.
    #[endpoint(initiateRetirement)]
    fn initiate_retirement(
        &self,
        retirement_id: ManagedBuffer,
        lot_id: ManagedBuffer,
        project_id: ManagedBuffer,
        amount_scaled: BigUint,
        beneficiary: ManagedAddress,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!retirement_id.is_empty(), "empty retirement_id");
        require!(!lot_id.is_empty(), "empty lot_id");
        require!(amount_scaled > 0u64, "amount must be positive");
        require!(
            !beneficiary.is_zero(),
            "ZERO_ADDRESS: beneficiary must not be zero"
        );
        require!(
            !self.retirements().contains_key(&retirement_id),
            "retirement already initiated"
        );
        require!(
            self.issued_issuance_lot_amounts().contains_key(&lot_id),
            "issuance lot not tokenized"
        );
        require!(
            self.issued_issuance_lot_projects().get(&lot_id).unwrap() == project_id,
            "retirement lot project mismatch"
        );
        let lot_issued_amount = self.issued_issuance_lot_amounts().get(&lot_id).unwrap();
        let lot_retired_amount = self.retired_issuance_lot_amount(&lot_id).get();
        require!(
            &lot_retired_amount + &amount_scaled <= lot_issued_amount,
            "retirement exceeds issued lot amount"
        );

        let record = RetirementRecord {
            retirement_id: retirement_id.clone(),
            lot_id: lot_id.clone(),
            project_id: project_id.clone(),
            amount_scaled: amount_scaled.clone(),
            beneficiary: beneficiary.clone(),
            status: ManagedBuffer::from(b"initiated"),
            initiated_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
            burn_tx_hash: ManagedBuffer::new(),
        };

        self.retirements().insert(retirement_id.clone(), record);
        self.retirement_initiated_event(&retirement_id, &lot_id, &project_id, &amount_scaled);
    }

    /// Confirms a retirement burn by recording the burn transaction hash and
    /// transitioning the retirement to `burned` status.
    #[payable("*")]
    #[endpoint(confirmRetirementBurn)]
    fn confirm_retirement_burn(&self, retirement_id: ManagedBuffer, burn_tx_hash: ManagedBuffer) {
        self.require_not_paused();
        require!(
            self.retirements().contains_key(&retirement_id),
            "retirement not found"
        );
        require!(!burn_tx_hash.is_empty(), "empty burn_tx_hash");

        let record = self
            .retirements()
            .get(&retirement_id)
            .unwrap_or_else(|| sc_panic!("RETIREMENT_NOT_FOUND"));
        require!(
            record.status == b"initiated",
            "retirement not in initiated state"
        );
        let caller = self.blockchain().get_caller();
        require!(
            caller == record.beneficiary,
            "ONLY_BENEFICIARY: retirement burn must be paid by beneficiary"
        );
        let payment = self.call_value().single_esdt();
        let dvcu_token_id = self.require_dvcu_token_id();
        require!(
            payment.token_identifier == dvcu_token_id,
            "must pay with dVCU token"
        );
        require!(
            payment.token_nonce == 0,
            "FUNGIBLE_ONLY: token nonce must be 0"
        );
        require!(
            payment.amount == record.amount_scaled,
            "wrong retirement burn amount"
        );

        self.send()
            .esdt_local_burn(&dvcu_token_id, 0, &record.amount_scaled);
        self.retirements()
            .entry(retirement_id.clone())
            .and_modify(|r| {
                r.status = ManagedBuffer::from(b"burned");
                r.burn_tx_hash = burn_tx_hash.clone();
            });
        self.total_dvcu_burned()
            .update(|total| *total += &record.amount_scaled);
        self.retired_issuance_lot_amount(&record.lot_id)
            .update(|total| *total += &record.amount_scaled);
        self.sync_registry_retired_lot(&record.lot_id);

        self.retirement_burned_event(&retirement_id, &burn_tx_hash);
    }

    /// Records a non-retirement issuance-lot reversal in the carbon-credit
    /// lifecycle authority and mirrors the terminal state to MRV registry when
    /// configured. `lot_id` is the carbon-credit reversal identifier.
    #[payable("*")]
    #[endpoint(recordIssuanceLotReversal)]
    fn record_issuance_lot_reversal(
        &self,
        lot_id: ManagedBuffer,
        reversed_amount_scaled: BigUint,
        replacement_lot_id: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!lot_id.is_empty(), "empty lot_id");
        require!(
            reversed_amount_scaled > 0u64,
            "reversed amount must be positive"
        );
        require!(
            !self.reversed_issuance_lots().contains_key(&lot_id),
            "issuance lot reversal already recorded"
        );
        require!(
            self.issued_issuance_lot_amounts().contains_key(&lot_id),
            "issuance lot not tokenized"
        );
        let issued_amount = self.issued_issuance_lot_amounts().get(&lot_id).unwrap();
        let retired_amount = self.retired_issuance_lot_amount(&lot_id).get();
        require!(
            &retired_amount + &reversed_amount_scaled <= issued_amount,
            "reversal exceeds unretired issued lot amount"
        );

        let dvcu_token_id = self.require_dvcu_token_id();
        let payment = self.call_value().single_esdt();
        require!(
            payment.token_identifier == dvcu_token_id,
            "must pay with dVCU token"
        );
        require!(
            payment.token_nonce == 0,
            "FUNGIBLE_ONLY: token nonce must be 0"
        );
        require!(
            payment.amount == reversed_amount_scaled,
            "wrong reversal burn amount"
        );
        self.send()
            .esdt_local_burn(&dvcu_token_id, 0, &reversed_amount_scaled);
        self.total_dvcu_burned()
            .update(|total| *total += &reversed_amount_scaled);
        self.reversed_issuance_lots()
            .insert(lot_id.clone(), reversed_amount_scaled.clone());
        self.sync_registry_reversed_lot(&lot_id, &reversed_amount_scaled, &replacement_lot_id);
        self.issuance_lot_reversal_recorded_event(
            &lot_id,
            &IssuanceLotReversalRecordedPayload {
                reversed_amount_scaled,
                replacement_lot_id,
            },
        );
    }

    /// Reverts an initiated retirement back to `reverted` status. Only
    /// retirements in `initiated` state can be reverted.
    #[endpoint(revertRetirement)]
    fn revert_retirement(&self, retirement_id: ManagedBuffer) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(
            self.retirements().contains_key(&retirement_id),
            "retirement not found"
        );

        let record = self
            .retirements()
            .get(&retirement_id)
            .unwrap_or_else(|| sc_panic!("RETIREMENT_NOT_FOUND"));
        require!(
            record.status == b"initiated",
            "can only revert initiated retirements"
        );

        self.retirements()
            .entry(retirement_id.clone())
            .and_modify(|r| {
                r.status = ManagedBuffer::from(b"reverted");
            });

        self.retirement_reverted_event(&retirement_id);
    }

    /// Issues GSOC credits after validating the registered bundle, verifier,
    /// DNA reference, and ITMO serial for the period.
    #[endpoint(issueGsocCredits)]
    fn issue_gsoc_credits(
        &self,
        project_id: ManagedBuffer,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
        gsoc_bundle_hash: ManagedBuffer,
        verifier_did: ManagedAddress,
        dna_project_ref: ManagedBuffer,
        itmo_serial: ManagedBuffer,
        gross_removals_scaled: BigUint,
        buffer_pct_bps: u64,
        recipient: ManagedAddress,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!project_id.is_empty(), "empty project_id");
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(
            gross_removals_scaled > 0u64,
            "gross_removals must be positive"
        );
        require!(
            !recipient.is_zero(),
            "ZERO_ADDRESS: recipient must not be zero"
        );
        require!(
            buffer_pct_bps > 0 && buffer_pct_bps <= 2500,
            "buffer_pct_bps must be 1-2500"
        );

        require!(
            gsoc_bundle_hash.len() == 32,
            "gsoc_bundle_hash must be 32 bytes"
        );
        let bundle_key = (pai_id.clone(), mrv_common::period_key(monitoring_period_n));
        let registered = self.gsoc_bundles().get(&bundle_key);
        require!(
            registered.is_some(),
            "GSOC_BUNDLE_NOT_REGISTERED: call registerGsocBundle first"
        );
        let registered_hash = registered.unwrap_or_else(|| sc_panic!("GSOC_BUNDLE_NOT_REGISTERED"));
        require!(
            registered_hash == gsoc_bundle_hash,
            "GSOC_BUNDLE_HASH_MISMATCH"
        );

        require!(!verifier_did.is_zero(), "empty verifier_did");
        require!(
            self.is_gsoc_verifier_approved_via_governance_or_local(verifier_did.clone()),
            "GSOC_VERIFIER_NOT_APPROVED"
        );

        require!(!dna_project_ref.is_empty(), "DNA_PROJECT_REF_REQUIRED");

        require!(!itmo_serial.is_empty(), "ITMO_SERIAL_REQUIRED");

        let issuance_key = (
            project_id.clone(),
            pai_id.clone(),
            mrv_common::period_key(monitoring_period_n),
        );
        require!(
            !self.gsoc_issuances().contains_key(&issuance_key),
            "GSOC credits already issued for this PAI/period"
        );

        // M-05 (AUD-011): conservative (ceiling) rounding on the GSOC buffer
        // contribution. Matches the dVCU issuance path above.
        let buffer_numerator = &gross_removals_scaled * buffer_pct_bps;
        let buffer_contribution = (buffer_numerator + 9_999u64) / 10_000u64;
        require!(buffer_contribution > 0u64, "BUFFER_ROUNDS_TO_ZERO");
        let net_issuable = &gross_removals_scaled - &buffer_contribution;
        require!(net_issuable > 0u64, "net_issuable must be positive");

        require!(
            !self.gsoc_serial_records().contains_key(&itmo_serial),
            "GSOC_SERIAL_ALREADY_ISSUED: itmo_serial already has an issuance record"
        );
        require!(
            self.project_gsoc_serial_count(&project_id).get() < MAX_GSOC_SERIALS_PER_PROJECT,
            "GSOC_PROJECT_SERIAL_LIMIT_EXCEEDED: project serial inventory exceeds bounded canonical hash limit"
        );

        self.gsoc_issuances()
            .insert(issuance_key, net_issuable.clone());
        let dgsc_token_id = self.require_dgsc_token_id();
        self.send()
            .esdt_local_mint(&dgsc_token_id, 0, &net_issuable);
        self.send()
            .direct_esdt(&recipient, &dgsc_token_id, 0, &net_issuable);
        self.total_dgsc_minted()
            .update(|total| *total += &net_issuable);
        self.project_gsoc_total_issued(&project_id)
            .update(|total| *total += &net_issuable);
        self.project_gsoc_serial_count(&project_id).update(|count| {
            *count = count
                .checked_add(1u64)
                .unwrap_or_else(|| sc_panic!("project_gsoc_serial_count overflow"))
        });
        self.project_gsoc_serials(&project_id)
            .insert(itmo_serial.clone());
        self.gsoc_serial_records().insert(
            itmo_serial.clone(),
            (
                project_id.clone(),
                monitoring_period_n,
                net_issuable.clone(),
            ),
        );
        self.gsoc_serial_recipients()
            .insert(itmo_serial.clone(), recipient.clone());

        self.gsoc_credits_issued_event(
            &project_id,
            &pai_id,
            &itmo_serial,
            &net_issuable,
            &recipient,
        );
    }

    /// Registers the committed GSOC bundle hash for a PAI and monitoring period.
    #[endpoint(registerGsocBundle)]
    fn register_gsoc_bundle(
        &self,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
        bundle_hash: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(bundle_hash.len() == 32, "bundle_hash must be 32 bytes");
        let key = (pai_id, mrv_common::period_key(monitoring_period_n));
        require!(
            !self.gsoc_bundles().contains_key(&key),
            "GSOC bundle already registered for this PAI/period"
        );
        self.gsoc_bundles().insert(key, bundle_hash);
    }

    /// Retires GSOC credits for a serial and emits the corresponding retirement event.
    ///
    /// M-03 (AUD-008): this method no longer mutates the third field
    /// of `gsoc_serial_records` in place. Instead:
    ///  - `gsoc_serial_records(serial)` is now read-only after
    ///    issuance. The `BigUint` field carries the IMMUTABLE initial
    ///    amount minted for the serial and is authoritative for the
    ///    replay-safe lineage view.
    ///  - `gsoc_serial_remaining(serial)` carries the running total.
    ///    It is initialized implicitly from the initial amount (by
    ///    fallback in the read path) and decrements on every
    ///    retirement.
    ///  - `gsoc_retirement_events(serial, seq_key)` is the append-only
    ///    log of every retirement touching the serial. Each entry
    ///    captures the amount retired, the beneficiary, the block
    ///    round, and the `remaining_after` balance, so a reader can
    ///    reconstruct the full retirement history without trusting a
    ///    mutable snapshot.
    ///  - `gsoc_retirement_seq_count(serial)` stores the next
    ///    sequence number to be written.
    #[payable("*")]
    #[endpoint(burnAndRetireGsoc)]
    fn burn_and_retire_gsoc(
        &self,
        itmo_serial: ManagedBuffer,
        amount_scaled: BigUint,
        beneficiary_name: ManagedBuffer,
        beneficiary_address: ManagedAddress,
    ) {
        self.require_not_paused();
        require!(!itmo_serial.is_empty(), "empty itmo_serial");
        require!(amount_scaled > 0u64, "amount must be positive");
        require!(
            !beneficiary_address.is_zero(),
            "ZERO_ADDRESS: beneficiary_address must not be zero"
        );
        require!(!beneficiary_name.is_empty(), "empty beneficiary_name");
        let caller = self.blockchain().get_caller();
        require!(
            caller == beneficiary_address,
            "ONLY_BENEFICIARY: GSOC retirement must be paid by beneficiary"
        );

        require!(
            self.gsoc_serial_records().contains_key(&itmo_serial),
            "GSOC serial not issued"
        );

        require!(
            !self.gsoc_retired_serials().contains(&itmo_serial),
            "GSOC_SERIAL_FULLY_RETIRED: no remaining balance on this serial"
        );

        // Read the IMMUTABLE initial amount from records. Do NOT mutate.
        let (_project_id, _period_n, initial_amount) = self
            .gsoc_serial_records()
            .get(&itmo_serial)
            .unwrap_or_else(|| sc_panic!("GSOC_SERIAL_NOT_FOUND"));

        // Read the current remaining balance. If the running-total
        // slot has never been written for this serial (i.e., this is
        // the first retirement), fall back to the immutable initial
        // amount. This is the sole correct bridge between the new
        // remaining-tracking schema and any pre-M-03 issuance record.
        let remaining = if self.gsoc_serial_remaining(&itmo_serial).is_empty() {
            initial_amount
        } else {
            self.gsoc_serial_remaining(&itmo_serial).get()
        };
        require!(
            remaining > 0u64,
            "GSOC_SERIAL_FULLY_RETIRED: remaining balance is zero"
        );
        require!(
            amount_scaled <= remaining,
            "GSOC_AMOUNT_EXCEEDS_REMAINING: cannot retire more than remaining quantity"
        );

        let new_remaining = &remaining - &amount_scaled;
        let dgsc_token_id = self.require_dgsc_token_id();
        let payment = self.call_value().single_esdt();
        require!(
            payment.token_identifier == dgsc_token_id,
            "must pay with dGSC token"
        );
        require!(
            payment.token_nonce == 0,
            "FUNGIBLE_ONLY: token nonce must be 0"
        );
        require!(payment.amount == amount_scaled, "wrong GSOC burn amount");
        self.send()
            .esdt_local_burn(&dgsc_token_id, 0, &amount_scaled);
        self.total_dgsc_burned()
            .update(|total| *total += &amount_scaled);
        self.project_gsoc_total_retired(&_project_id)
            .update(|total| *total += &amount_scaled);
        self.gsoc_serial_remaining(&itmo_serial).set(&new_remaining);

        // Append the event to the per-serial log. `seq` is stable
        // once written; `gsoc_retirement_seq_count` tracks the next
        // sequence number for future appends on the same serial.
        let seq = self.gsoc_retirement_seq_count(&itmo_serial).get();
        let retired_at_round = self.blockchain().get_block_round();
        let event_record = GsocRetirementEventRecord {
            seq,
            amount_scaled: amount_scaled.clone(),
            remaining_after: new_remaining.clone(),
            beneficiary_name: beneficiary_name.clone(),
            beneficiary_address: beneficiary_address.clone(),
            retired_at_round,
        };
        self.gsoc_retirement_events(&itmo_serial, seq)
            .set(event_record);
        let next_seq = seq
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("GSOC_RETIREMENT_SEQUENCE_OVERFLOW"));
        self.gsoc_retirement_seq_count(&itmo_serial).set(next_seq);

        if new_remaining == 0u64 {
            self.gsoc_retired_serials().insert(itmo_serial.clone());
        }

        self.gsoc_credit_retired_event(
            &itmo_serial,
            &amount_scaled,
            &beneficiary_name,
            &beneficiary_address,
        );
        // M-03: fires on EVERY partial or full retirement so indexers
        // can track the running remaining balance and sequence order
        // without re-deriving from the transaction log.
        let partial_payload = GsocPartialRetirementEventPayload {
            amount_scaled: amount_scaled.clone(),
            remaining_after: new_remaining.clone(),
        };
        self.gsoc_serial_partially_retired_event(&itmo_serial, seq, &partial_payload);
    }

    /// Adds a verifier to the approved GSOC verifier set.
    ///
    /// This contract maintains a local GSOC verifier set separate from the
    /// governance contract's GSOC verifier registry.  The governance contract
    /// is the authoritative source; this set remains only for legacy/bootstrap
    /// visibility and is not used by `issueGsocCredits`.
    ///
    /// Issuance checks the configured governance-read contract directly, so
    /// this legacy set cannot authorize `issueGsocCredits`.
    #[endpoint(addApprovedGsocVerifier)]
    fn add_approved_gsoc_verifier(&self, verifier: ManagedAddress) {
        self.require_governance_or_owner();
        self.require_not_paused();
        self.require_local_gsoc_verifier_registry_mode();
        require!(!verifier.is_zero(), "verifier must not be zero");
        self.approved_gsoc_verifiers().insert(verifier);
    }

    /// Removes a verifier from the approved GSOC verifier set.
    #[endpoint(removeApprovedGsocVerifier)]
    fn remove_approved_gsoc_verifier(&self, verifier: ManagedAddress) {
        self.require_governance_or_owner();
        self.require_not_paused();
        self.require_local_gsoc_verifier_registry_mode();
        self.approved_gsoc_verifiers().swap_remove(&verifier);
    }

    #[view(isGsocVerifierApproved)]
    fn is_gsoc_verifier_approved(&self, verifier: ManagedAddress) -> bool {
        self.is_gsoc_verifier_approved_via_governance_or_local(verifier)
    }

    /// Returns the canonical GSOC serial inventory hash for a project.
    ///
    /// The preimage matches the worker cadence exactly:
    /// `sha256(JSON.stringify([{serial, quantityTco2e, status}, ...]))`
    /// with entries sorted by serial ascending and status restricted to
    /// `"registered"` or `"retired"`.
    #[view(getCanonicalGsocSerialInventoryHash)]
    fn get_canonical_gsoc_serial_inventory_hash(&self, project_id: ManagedBuffer) -> ManagedBuffer {
        self.compute_canonical_gsoc_serial_inventory_hash(&project_id)
    }

    #[view(verifyCanonicalGsocSerialInventoryHash)]
    fn verify_canonical_gsoc_serial_inventory_hash(
        &self,
        project_id: ManagedBuffer,
        expected_hash: ManagedBuffer,
    ) -> bool {
        self.compute_canonical_gsoc_serial_inventory_hash(&project_id) == expected_hash
    }

    #[view(getImeRecord)]
    fn get_ime_record(
        &self,
        project_id: ManagedBuffer,
    ) -> OptionalValue<ImeValidationRecord<Self::Api>> {
        if self.active_ime_record(&project_id).is_empty() {
            OptionalValue::None
        } else {
            OptionalValue::Some(self.active_ime_record(&project_id).get())
        }
    }

    #[view(getActiveImeRecordVersion)]
    fn get_active_ime_record_version(&self, project_id: ManagedBuffer) -> u64 {
        self.active_ime_record_version(&project_id).get()
    }

    #[view(getImeRecordVersionCount)]
    fn get_ime_record_version_count(&self, project_id: ManagedBuffer) -> u64 {
        self.ime_record_version_count(&project_id).get()
    }

    #[view(getImeRecordVersion)]
    fn get_ime_record_version(
        &self,
        project_id: ManagedBuffer,
        version: u64,
    ) -> OptionalValue<ImeValidationRecord<Self::Api>> {
        match self.ime_record_versions().get(&(project_id, version)) {
            Some(record) => OptionalValue::Some(record),
            None => OptionalValue::None,
        }
    }

    #[view(getIssuanceImeRecordVersion)]
    fn get_issuance_ime_record_version(
        &self,
        project_id: ManagedBuffer,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
    ) -> u64 {
        let key = (
            project_id,
            pai_id,
            mrv_common::period_key(monitoring_period_n),
        );
        self.issuance_ime_versions().get(&key).unwrap_or_default()
    }

    #[view(getRetirement)]
    fn get_retirement(
        &self,
        retirement_id: ManagedBuffer,
    ) -> OptionalValue<RetirementRecord<Self::Api>> {
        match self.retirements().get(&retirement_id) {
            Some(r) => OptionalValue::Some(r),
            None => OptionalValue::None,
        }
    }

    #[view(getRecordedIssuanceLotReversal)]
    fn get_recorded_issuance_lot_reversal(&self, lot_id: ManagedBuffer) -> OptionalValue<BigUint> {
        match self.reversed_issuance_lots().get(&lot_id) {
            Some(amount) => OptionalValue::Some(amount),
            None => OptionalValue::None,
        }
    }

    #[view(getIssuedIssuanceLotRecipient)]
    fn get_issued_issuance_lot_recipient(
        &self,
        lot_id: ManagedBuffer,
    ) -> OptionalValue<ManagedAddress> {
        match self.issued_issuance_lot_recipients().get(&lot_id) {
            Some(recipient) => OptionalValue::Some(recipient),
            None => OptionalValue::None,
        }
    }

    #[view(getGsocSerialRecipient)]
    fn get_gsoc_serial_recipient(
        &self,
        itmo_serial: ManagedBuffer,
    ) -> OptionalValue<ManagedAddress> {
        match self.gsoc_serial_recipients().get(&itmo_serial) {
            Some(recipient) => OptionalValue::Some(recipient),
            None => OptionalValue::None,
        }
    }

    /// Buffer-pool contract address. Pending contributions are tracked locally
    /// and materialized by `confirmBufferDeposit` through a same-shard
    /// buffer-pool deposit call.
    #[storage_mapper("bufferPoolAddr")]
    fn buffer_pool_addr(&self) -> SingleValueMapper<ManagedAddress>;

    /// Canonical dVCU lifecycle token identifier.
    #[view(getDvcuTokenId)]
    #[storage_mapper("dvcuTokenId")]
    fn dvcu_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    /// Canonical dGSC lifecycle token identifier.
    #[view(getDgscTokenId)]
    #[storage_mapper("dgscTokenId")]
    fn dgsc_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    /// Monotonic dVCU minted supply counter.
    #[view(getTotalDvcuMinted)]
    #[storage_mapper("totalDvcuMinted")]
    fn total_dvcu_minted(&self) -> SingleValueMapper<BigUint>;

    /// Monotonic dVCU burned supply counter.
    #[view(getTotalDvcuBurned)]
    #[storage_mapper("totalDvcuBurned")]
    fn total_dvcu_burned(&self) -> SingleValueMapper<BigUint>;

    /// Monotonic dGSC minted supply counter.
    #[view(getTotalDgscMinted)]
    #[storage_mapper("totalDgscMinted")]
    fn total_dgsc_minted(&self) -> SingleValueMapper<BigUint>;

    /// Monotonic dGSC burned supply counter.
    #[view(getTotalDgscBurned)]
    #[storage_mapper("totalDgscBurned")]
    fn total_dgsc_burned(&self) -> SingleValueMapper<BigUint>;

    /// Project-scoped GSOC issued counter used by reserve-proof reconciliation.
    #[view(getGsocProjectTotalIssued)]
    #[storage_mapper("projectGsocTotalIssued")]
    fn project_gsoc_total_issued(&self, project_id: &ManagedBuffer) -> SingleValueMapper<BigUint>;

    /// Project-scoped GSOC retired counter used by reserve-proof reconciliation.
    #[view(getGsocProjectTotalRetired)]
    #[storage_mapper("projectGsocTotalRetired")]
    fn project_gsoc_total_retired(&self, project_id: &ManagedBuffer) -> SingleValueMapper<BigUint>;

    /// Project-scoped GSOC serial count used by reserve-proof reconciliation.
    #[view(getGsocProjectSerialCount)]
    #[storage_mapper("projectGsocSerialCount")]
    fn project_gsoc_serial_count(&self, project_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    /// Project-scoped GSOC serial index used for canonical reserve-proof hashing.
    #[storage_mapper("projectGsocSerials")]
    fn project_gsoc_serials(&self, project_id: &ManagedBuffer)
    -> UnorderedSetMapper<ManagedBuffer>;

    #[storage_mapper("activeImeRecord")]
    fn active_ime_record(
        &self,
        project_id: &ManagedBuffer,
    ) -> SingleValueMapper<ImeValidationRecord<Self::Api>>;

    #[storage_mapper("activeImeRecordVersion")]
    fn active_ime_record_version(&self, project_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[storage_mapper("imeRecordVersionCount")]
    fn ime_record_version_count(&self, project_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[storage_mapper("imeRecordVersions")]
    fn ime_record_versions(
        &self,
    ) -> MapMapper<(ManagedBuffer, u64), ImeValidationRecord<Self::Api>>;

    #[storage_mapper("issuances")]
    fn issuances(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer, ManagedBuffer), BigUint>;

    #[storage_mapper("issuanceLotsByIssueKey")]
    fn issuance_lots_by_issue_key(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer, ManagedBuffer), ManagedBuffer>;

    #[storage_mapper("issuedIssuanceLotProjects")]
    fn issued_issuance_lot_projects(&self) -> MapMapper<ManagedBuffer, ManagedBuffer>;

    #[storage_mapper("issuedIssuanceLotAmounts")]
    fn issued_issuance_lot_amounts(&self) -> MapMapper<ManagedBuffer, BigUint>;

    #[storage_mapper("issuedIssuanceLotRecipients")]
    fn issued_issuance_lot_recipients(&self) -> MapMapper<ManagedBuffer, ManagedAddress>;

    #[view(getRetiredIssuanceLotAmount)]
    #[storage_mapper("retiredIssuanceLotAmount")]
    fn retired_issuance_lot_amount(&self, lot_id: &ManagedBuffer) -> SingleValueMapper<BigUint>;

    #[storage_mapper("issuanceImeVersions")]
    fn issuance_ime_versions(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer, ManagedBuffer), u64>;

    #[storage_mapper("retirements")]
    fn retirements(&self) -> MapMapper<ManagedBuffer, RetirementRecord<Self::Api>>;

    #[view(getRegistryLifecycleAddress)]
    #[storage_mapper("registryLifecycleAddress")]
    fn registry_lifecycle_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[storage_mapper("reversedIssuanceLots")]
    fn reversed_issuance_lots(&self) -> MapMapper<ManagedBuffer, BigUint>;

    /// Committed bundle hashes keyed by `(pai_id, period_key)`.
    #[storage_mapper("committedBundles")]
    fn committed_bundles(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), ManagedBuffer>;

    /// Bound bundle hashes keyed by `(pai_id, period_key)` after issuance.
    #[storage_mapper("boundBundleHashes")]
    fn bound_bundle_hashes(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), ManagedBuffer>;

    /// Pending buffer contributions keyed by `(project_id, pai_id, period_key)`.
    #[storage_mapper("pendingBufferDeposits")]
    fn pending_buffer_deposits(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer, ManagedBuffer), BigUint>;

    fn sync_registry_retired_lot(&self, lot_id: &ManagedBuffer) {
        if self.registry_lifecycle_address().is_empty() {
            return;
        }

        use registry_lifecycle_proxy::RegistryLifecycleProxy;

        let registry_address = self.registry_lifecycle_address().get();
        let gas_for_call = self.blockchain().get_gas_left() / 4;
        self.tx()
            .to(&registry_address)
            .gas(gas_for_call)
            .typed(RegistryLifecycleProxy)
            .retire_issuance_lot(lot_id.clone())
            .sync_call();
    }

    fn sync_registry_reversed_lot(
        &self,
        lot_id: &ManagedBuffer,
        reversed_amount_scaled: &BigUint,
        replacement_lot_id: &ManagedBuffer,
    ) {
        if self.registry_lifecycle_address().is_empty() {
            return;
        }

        use registry_lifecycle_proxy::RegistryLifecycleProxy;

        let registry_address = self.registry_lifecycle_address().get();
        let gas_for_call = self.blockchain().get_gas_left() / 4;
        self.tx()
            .to(&registry_address)
            .gas(gas_for_call)
            .typed(RegistryLifecycleProxy)
            .reverse_issuance_lot(
                lot_id.clone(),
                reversed_amount_scaled.clone(),
                replacement_lot_id.clone(),
            )
            .sync_call();
    }

    fn get_project_live_buffer_reserve(&self, project_id: &ManagedBuffer) -> BigUint {
        use buffer_pool_proxy::BufferPoolProxy;

        require!(
            !self.buffer_pool_addr().is_empty(),
            "buffer_pool_addr not configured"
        );

        let gas_for_query = self.blockchain().get_gas_left() / 16;
        let buffer_record: OptionalValue<mrv_buffer_pool::BufferRecord<Self::Api>> = self
            .tx()
            .to(self.buffer_pool_addr().get())
            .gas(gas_for_query)
            .typed(BufferPoolProxy)
            .get_buffer_record(project_id.clone())
            .returns(ReturnsResult)
            .sync_call_readonly();

        match buffer_record {
            OptionalValue::Some(record) => {
                let total_inflows = &record.total_deposited + &record.total_replenished;
                if total_inflows >= record.total_cancelled {
                    total_inflows - record.total_cancelled
                } else {
                    BigUint::zero()
                }
            }
            OptionalValue::None => BigUint::zero(),
        }
    }

    fn get_buffer_pool_total_balance(&self) -> BigUint {
        use buffer_pool_proxy::BufferPoolProxy;

        require!(
            !self.buffer_pool_addr().is_empty(),
            "buffer_pool_addr not configured"
        );

        let gas_for_query = self.blockchain().get_gas_left() / 16;
        self.tx()
            .to(self.buffer_pool_addr().get())
            .gas(gas_for_query)
            .typed(BufferPoolProxy)
            .get_total_pool_balance()
            .returns(ReturnsResult)
            .sync_call_readonly()
    }

    fn deposit_pending_buffer_contribution(
        &self,
        project_id: &ManagedBuffer,
        pending_amount: &BigUint,
        monitoring_period_n: u64,
    ) {
        use buffer_pool_proxy::BufferPoolProxy;

        require!(
            !self.buffer_pool_addr().is_empty(),
            "buffer_pool_addr not configured"
        );

        let buffer_pool_address = self.buffer_pool_addr().get();
        let gas_for_call = self.blockchain().get_gas_left() / 4;
        self.tx()
            .to(&buffer_pool_address)
            .gas(gas_for_call)
            .typed(BufferPoolProxy)
            .deposit_buffer_credits(
                project_id.clone(),
                pending_amount.clone(),
                monitoring_period_n,
            )
            .sync_call();
    }

    #[event("creditsIssued")]
    fn credits_issued_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        #[indexed] lot_id: &ManagedBuffer,
        #[indexed] pai_id: &ManagedBuffer,
        net_issuable: &BigUint,
        #[indexed] recipient: &ManagedAddress,
    );

    #[event("bufferDepositPending")]
    fn buffer_deposit_pending_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        #[indexed] pai_id: &ManagedBuffer,
        buffer_contribution: &BigUint,
    );

    #[event("bufferDepositConfirmed")]
    fn buffer_deposit_confirmed_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        buffer_contribution: &BigUint,
    );

    #[event("imeRegistered")]
    fn ime_registered_event(&self, #[indexed] project_id: &ManagedBuffer);

    #[event("imeRevoked")]
    fn ime_revoked_event(&self, #[indexed] project_id: &ManagedBuffer);

    #[event("retirementInitiated")]
    fn retirement_initiated_event(
        &self,
        #[indexed] retirement_id: &ManagedBuffer,
        #[indexed] lot_id: &ManagedBuffer,
        #[indexed] project_id: &ManagedBuffer,
        amount: &BigUint,
    );

    #[event("retirementBurned")]
    fn retirement_burned_event(
        &self,
        #[indexed] retirement_id: &ManagedBuffer,
        burn_tx_hash: &ManagedBuffer,
    );

    #[event("retirementReverted")]
    fn retirement_reverted_event(&self, #[indexed] retirement_id: &ManagedBuffer);

    #[event("issuanceLotReversalRecorded")]
    fn issuance_lot_reversal_recorded_event(
        &self,
        #[indexed] lot_id: &ManagedBuffer,
        payload: &IssuanceLotReversalRecordedPayload<Self::Api>,
    );

    #[event("gsocCreditsIssued")]
    fn gsoc_credits_issued_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        #[indexed] pai_id: &ManagedBuffer,
        #[indexed] itmo_serial: &ManagedBuffer,
        net_issuable: &BigUint,
        #[indexed] recipient: &ManagedAddress,
    );

    #[event("gsocCreditRetired")]
    fn gsoc_credit_retired_event(
        &self,
        #[indexed] itmo_serial: &ManagedBuffer,
        amount: &BigUint,
        #[indexed] beneficiary_name: &ManagedBuffer,
        #[indexed] beneficiary_address: &ManagedAddress,
    );

    /// M-03 (AUD-008): fires on every partial or full GSOC retirement.
    /// Carries `seq` (per-serial sequence number) as an indexed topic
    /// and a `GsocPartialRetirementEventPayload` with the pair
    /// `{amount_scaled, remaining_after}`. Indexers reconstruct the
    /// per-serial retirement lineage by replaying these events in
    /// `seq` order.
    ///
    /// Event-log framing allows only ONE non-indexed data argument,
    /// so the two `BigUint` values are bundled into a single payload
    /// struct below.
    #[event("gsocSerialPartiallyRetired")]
    fn gsoc_serial_partially_retired_event(
        &self,
        #[indexed] itmo_serial: &ManagedBuffer,
        #[indexed] seq: u64,
        payload: &GsocPartialRetirementEventPayload<Self::Api>,
    );

    #[event("governanceReadAddressUpdated")]
    fn governance_read_address_updated_event(
        &self,
        #[indexed] governance_read_address: &ManagedAddress,
    );

    #[event("governanceReadAddressCleared")]
    fn governance_read_address_cleared_event(&self);

    #[event("registryLifecycleAddressUpdated")]
    fn registry_lifecycle_address_updated_event(
        &self,
        #[indexed] registry_address: &ManagedAddress,
    );

    #[event("registryLifecycleAddressCleared")]
    fn registry_lifecycle_address_cleared_event(&self);

    #[event("dvcuTokenIdUpdated")]
    fn dvcu_token_id_updated_event(&self, #[indexed] token_id: &TokenIdentifier);

    #[event("dgscTokenIdUpdated")]
    fn dgsc_token_id_updated_event(&self, #[indexed] token_id: &TokenIdentifier);

    /// GSOC bundle hashes keyed by (pai_id, period_key).
    #[storage_mapper("gsocBundles")]
    fn gsoc_bundles(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), ManagedBuffer>;

    /// GSOC issuances keyed by (project_id, pai_id, period_key).
    #[storage_mapper("gsocIssuances")]
    fn gsoc_issuances(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer, ManagedBuffer), BigUint>;

    /// GSOC serial records: `itmo_serial → (project_id, period, initial_amount)`.
    ///
    /// M-03 (AUD-008): the `BigUint` field is the INITIAL minted
    /// amount and is IMMUTABLE after issuance. It is NOT a running
    /// remaining balance. Consumers that want "how much is left on
    /// this serial" must read `gsoc_serial_remaining` below (falling
    /// back to this initial amount only if the remaining slot has
    /// never been written).
    #[storage_mapper("gsocSerialRecords")]
    fn gsoc_serial_records(&self) -> MapMapper<ManagedBuffer, (ManagedBuffer, u64, BigUint)>;

    #[storage_mapper("gsocSerialRecipients")]
    fn gsoc_serial_recipients(&self) -> MapMapper<ManagedBuffer, ManagedAddress>;

    /// M-03 (AUD-008): running remaining balance per serial. Only
    /// written by `burn_and_retire_gsoc`. Absent ⇒ no retirements yet
    /// (remaining equals the initial amount on the serial record).
    #[view(getGsocSerialRemaining)]
    #[storage_mapper("gsocSerialRemaining")]
    fn gsoc_serial_remaining(&self, itmo_serial: &ManagedBuffer) -> SingleValueMapper<BigUint>;

    /// M-03 (AUD-008): append-only log of every retirement event that
    /// has touched the given serial. Keyed by `(serial, seq)`;
    /// `seq` is 0-based and strictly increasing per serial.
    #[view(getGsocRetirementEvent)]
    #[storage_mapper("gsocRetirementEvents")]
    fn gsoc_retirement_events(
        &self,
        itmo_serial: &ManagedBuffer,
        seq: u64,
    ) -> SingleValueMapper<GsocRetirementEventRecord<Self::Api>>;

    /// M-03 (AUD-008): next sequence number to be written to
    /// `gsoc_retirement_events` for the given serial. Equal to the
    /// total number of retirements logged for that serial so far.
    #[view(getGsocRetirementSeqCount)]
    #[storage_mapper("gsocRetirementSeqCount")]
    fn gsoc_retirement_seq_count(&self, itmo_serial: &ManagedBuffer) -> SingleValueMapper<u64>;

    /// GSOC serials that have been fully retired.
    #[storage_mapper("gsocRetiredSerials")]
    fn gsoc_retired_serials(&self) -> UnorderedSetMapper<ManagedBuffer>;

    /// Approved GSOC verifiers.
    #[storage_mapper("approvedGsocVerifiers")]
    fn approved_gsoc_verifiers(&self) -> UnorderedSetMapper<ManagedAddress>;

    #[view(getGovernanceReadAddress)]
    #[storage_mapper("governanceReadAddress")]
    fn governance_read_address(&self) -> SingleValueMapper<ManagedAddress>;

    fn require_dvcu_token_id(&self) -> TokenIdentifier {
        require!(
            !self.dvcu_token_id().is_empty(),
            "DVCU_TOKEN_NOT_CONFIGURED"
        );
        self.dvcu_token_id().get()
    }

    fn require_dgsc_token_id(&self) -> TokenIdentifier {
        require!(
            !self.dgsc_token_id().is_empty(),
            "DGSC_TOKEN_NOT_CONFIGURED"
        );
        self.dgsc_token_id().get()
    }

    fn require_dvcu_token_replacement_allowed(&self, new_token_id: &TokenIdentifier) {
        if self.dvcu_token_id().is_empty() {
            return;
        }

        let current_token_id = self.dvcu_token_id().get();
        if current_token_id == *new_token_id {
            return;
        }

        require!(
            self.total_dvcu_minted().get() == 0u64 && self.total_dvcu_burned().get() == 0u64,
            "DVCU_TOKEN_ID_LOCKED"
        );
    }

    fn require_dgsc_token_replacement_allowed(&self, new_token_id: &TokenIdentifier) {
        if self.dgsc_token_id().is_empty() {
            return;
        }

        let current_token_id = self.dgsc_token_id().get();
        if current_token_id == *new_token_id {
            return;
        }

        require!(
            self.total_dgsc_minted().get() == 0u64 && self.total_dgsc_burned().get() == 0u64,
            "DGSC_TOKEN_ID_LOCKED"
        );
    }

    fn is_gsoc_verifier_approved_via_governance_or_local(&self, verifier: ManagedAddress) -> bool {
        use governance_proxy::GovernanceProxy;

        require!(
            !self.governance_read_address().is_empty(),
            "GSOC_VERIFIER_GOVERNANCE_READ_REQUIRED"
        );

        let gas_for_query = self.blockchain().get_gas_left() / 16;
        self.tx()
            .to(self.governance_read_address().get())
            .gas(gas_for_query)
            .typed(GovernanceProxy)
            .is_gsoc_verifier_approved(verifier)
            .returns(ReturnsResult)
            .sync_call_readonly()
    }

    fn require_local_gsoc_verifier_registry_mode(&self) {
        require!(
            self.governance_read_address().is_empty(),
            "GSOC_VERIFIER_REGISTRY_CANONICALIZED_TO_GOVERNANCE: local GSOC verifier mutations are disabled while governanceReadAddress is configured"
        );
    }

    fn require_not_paused(&self) {
        use governance_proxy::GovernanceProxy;

        if self.governance_read_address().is_empty() {
            let authority = if !self.governance().is_empty() {
                self.governance().get()
            } else {
                self.blockchain().get_owner_address()
            };
            require!(
                !self.blockchain().is_smart_contract(&authority),
                "MRV_GOVERNANCE_READ_NOT_CONFIGURED"
            );
            return;
        }

        let gas_for_query = self.blockchain().get_gas_left() / 16;
        let paused: bool = self
            .tx()
            .to(self.governance_read_address().get())
            .gas(gas_for_query)
            .typed(GovernanceProxy)
            .get_paused()
            .returns(ReturnsResult)
            .sync_call_readonly();
        require!(!paused, "MRV_GOVERNANCE_PAUSED");
    }

    fn compute_canonical_gsoc_serial_inventory_hash(
        &self,
        project_id: &ManagedBuffer,
    ) -> ManagedBuffer {
        let mut sorted_serials: ManagedVec<Self::Api, ManagedBuffer> = ManagedVec::new();
        for serial in self.project_gsoc_serials(project_id).iter() {
            let (record_project_id, _period_n, initial_amount) = self
                .gsoc_serial_records()
                .get(&serial)
                .unwrap_or_else(|| sc_panic!("GSOC_SERIAL_INDEX_CORRUPTED"));
            require!(
                record_project_id == *project_id,
                "GSOC_SERIAL_INDEX_PROJECT_MISMATCH"
            );
            let _ = initial_amount;
            sorted_serials.push(serial);
        }

        self.sort_managed_buffers(&mut sorted_serials);

        let mut canonical = ManagedBuffer::new();
        canonical.append_bytes(b"[");
        for idx in 0..sorted_serials.len() {
            if idx > 0 {
                canonical.append_bytes(b",");
            }
            let serial = sorted_serials.get(idx);
            let (_record_project_id, _period_n, amount) = self
                .gsoc_serial_records()
                .get(&serial)
                .unwrap_or_else(|| sc_panic!("GSOC_SERIAL_INDEX_CORRUPTED"));
            let is_retired = self.gsoc_retired_serials().contains(&serial);

            canonical.append_bytes(b"{\"serial\":\"");
            self.append_json_string_content(&mut canonical, &serial);
            canonical.append_bytes(b"\",\"quantityTco2e\":");
            self.append_biguint_ascii_decimal(&mut canonical, &amount);
            canonical.append_bytes(b",\"status\":\"");
            if is_retired {
                canonical.append_bytes(b"retired");
            } else {
                canonical.append_bytes(b"registered");
            }
            canonical.append_bytes(b"\"}");
        }
        canonical.append_bytes(b"]");

        self.crypto().sha256(&canonical).as_managed_buffer().clone()
    }

    fn sort_managed_buffers(&self, values: &mut ManagedVec<ManagedBuffer>) {
        let len = values.len();
        for i in 1..len {
            let mut j = i;
            while j > 0 {
                if self.managed_buffer_lex_gt(&values.get(j - 1), &values.get(j)) {
                    let left = values.get(j - 1).clone();
                    let right = values.get(j).clone();
                    values
                        .set(j - 1, right)
                        .unwrap_or_else(|_| sc_panic!("GSOC_SERIAL_SORT_FAILED"));
                    values
                        .set(j, left)
                        .unwrap_or_else(|_| sc_panic!("GSOC_SERIAL_SORT_FAILED"));
                    j -= 1;
                } else {
                    break;
                }
            }
        }
    }

    fn append_biguint_ascii_decimal(&self, out: &mut ManagedBuffer, value: &BigUint) {
        if *value == 0u64 {
            out.append_bytes(b"0");
            return;
        }

        let mut remaining = value.clone();
        let mut digits: ManagedVec<Self::Api, u8> = ManagedVec::new();
        while remaining > 0u64 {
            let digit = (&remaining % 10u64)
                .to_u64()
                .unwrap_or_else(|| sc_panic!("GSOC_SERIAL_QUANTITY_DIGIT_OVERFLOWS_U64"));
            digits.push(b'0' + digit as u8);
            remaining /= 10u64;
        }

        for digit in digits.iter().rev() {
            out.append_bytes(&[digit]);
        }
    }

    fn append_u64_ascii_decimal(&self, out: &mut ManagedBuffer, mut value: u64) {
        let mut digits = [0u8; 20];
        let mut pos = digits.len();

        if value == 0 {
            pos -= 1;
            digits[pos] = b'0';
        } else {
            while value > 0 {
                pos -= 1;
                digits[pos] = b'0' + (value % 10) as u8;
                value /= 10;
            }
        }

        out.append_bytes(&digits[pos..]);
    }

    fn append_json_string_content(&self, out: &mut ManagedBuffer, value: &ManagedBuffer) {
        const HEX: &[u8; 16] = b"0123456789abcdef";

        value.for_each_batch::<32, _>(|bytes| {
            for &byte in bytes {
                match byte {
                    b'"' => out.append_bytes(b"\\\""),
                    b'\\' => out.append_bytes(b"\\\\"),
                    0x00..=0x1f => {
                        out.append_bytes(b"\\u00");
                        out.append_bytes(&[
                            HEX[((byte >> 4) & 0x0f) as usize],
                            HEX[(byte & 0x0f) as usize],
                        ]);
                    }
                    _ => out.append_bytes(&[byte]),
                }
            }
        });
    }

    fn managed_buffer_lex_gt(&self, left: &ManagedBuffer, right: &ManagedBuffer) -> bool {
        let left_len = left.len();
        let right_len = right.len();
        let shared_len = core::cmp::min(left_len, right_len);
        let mut left_byte = [0u8; 1];
        let mut right_byte = [0u8; 1];

        for index in 0..shared_len {
            left.load_slice(index, &mut left_byte);
            right.load_slice(index, &mut right_byte);
            if left_byte[0] > right_byte[0] {
                return true;
            }
            if left_byte[0] < right_byte[0] {
                return false;
            }
        }

        left_len > right_len
    }

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
