#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

/// Maximum number of oracle source types supported by `compute_mrv_root`.
const MAX_ORACLE_SOURCES: usize = 3;

/// Oracle source type identifier for IoT readings.
const SOURCE_IOT: u8 = 0;
/// Oracle source type identifier for satellite readings.
const SOURCE_SATELLITE: u8 = 1;
/// Oracle source type identifier for government lab readings.
const SOURCE_GOVT_LAB: u8 = 2;

/// Default time-coherence windows (seconds). Configurable at init.
const DEFAULT_IOT_WINDOW: u64 = 172_800;
const DEFAULT_SATELLITE_WINDOW: u64 = 864_000;
const DEFAULT_GOVT_LAB_WINDOW: u64 = 2_592_000;

/// Minimum oracle readings required to seal (2-of-3 quorum).
const QUORUM_MIN: u32 = 2;

/// Oracle reading stored for a single PAI, monitoring period, and source.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
pub struct OracleReading<M: ManagedTypeApi> {
    pub pai_id: ManagedBuffer<M>,
    pub period_start: u64,
    pub period_end: u64,
    pub source: u8,
    pub data_cid: ManagedBuffer<M>,
    pub source_timestamp: u64,
    pub device_did: ManagedAddress<M>,
    pub device_signature: ManagedBuffer<M>,
}

/// Sealed monitoring-period record containing the computed MRV root.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
pub struct SealedEvent<M: ManagedTypeApi> {
    pub pai_id: ManagedBuffer<M>,
    pub period_end: u64,
    pub mrv_root: ManagedBuffer<M>,
    pub reading_count: u32,
    pub semantic_discrepancy: bool,
    pub sealed_at: u64,
}

/// Oracle aggregator contract for MRV monitoring periods.
///
/// Collects IoT, Satellite, and Government Lab readings, enforces a
/// configurable quorum, detects semantic divergence between IoT and
/// Satellite sources, and seals monitoring periods into a deterministic
/// MRV root hash.
#[multiversx_sc::contract]
pub trait MrvAggregator {
    /// Initializes quorum, per-source coherence windows, and divergence
    /// threshold. Zero values fall back to built-in defaults.
    #[init]
    fn init(
        &self,
        quorum: u32,
        iot_window: u64,
        satellite_window: u64,
        govt_lab_window: u64,
        divergence_threshold_bps: u64,
    ) {
        require!(quorum >= QUORUM_MIN, "quorum must be >= 2");
        self.quorum().set(quorum);
        self.iot_window().set(if iot_window > 0 { iot_window } else { DEFAULT_IOT_WINDOW });
        self.satellite_window().set(if satellite_window > 0 { satellite_window } else { DEFAULT_SATELLITE_WINDOW });
        self.govt_lab_window().set(if govt_lab_window > 0 { govt_lab_window } else { DEFAULT_GOVT_LAB_WINDOW });
        self.divergence_threshold_bps().set(if divergence_threshold_bps > 0 { divergence_threshold_bps } else { 3000u64 });
        self.storage_version().set(1u32);
    }

    /// Submits a reading for a PAI monitoring period from an authorized source.
    ///
    /// SECURITY-BOUNDARY: IoT device signature verification is structural only
    /// (non-empty, >= 64 bytes). Cryptographic ed25519 verification is performed
    /// by the off-chain oracle validation pipeline before on-chain submission.
    /// On-chain verification is deferred because the VM does not natively support
    /// ed25519 at acceptable gas cost. The trust model requires that all registered
    /// oracles are operated by trusted parties.
    ///
    /// `device_did` may be the zero address for non-IoT sources. IoT
    /// submissions require a registered device and a non-empty
    /// `device_signature`.
    ///
    /// For IoT readings, this endpoint validates only that a signature value is
    /// present.
    /// Cryptographic verification of that signature is expected to happen
    /// off-chain before submission.
    #[endpoint(submitOracleReading)]
    fn submit_oracle_reading(
        &self,
        pai_id: ManagedBuffer,
        period_start: u64,
        period_end: u64,
        source: u8,
        data_cid: ManagedBuffer,
        source_timestamp: u64,
        device_did: ManagedAddress,
        device_signature: ManagedBuffer,
    ) {
        let caller = self.blockchain().get_caller();
        require!(
            self.authorized_oracles().contains(&caller),
            "ORACLE_NOT_AUTHORIZED: caller must be a registered authorized oracle"
        );

        require!(!pai_id.is_empty(), "empty pai_id");
        require!(period_start > 0, "invalid period_start");
        require!(period_end > period_start, "period_end must be after period_start");
        require!(
            source == SOURCE_IOT || source == SOURCE_SATELLITE || source == SOURCE_GOVT_LAB,
            "source must be 0 (IoT), 1 (Satellite), or 2 (GovtLab)"
        );
        require!(!data_cid.is_empty(), "empty data_cid");
        require!(source_timestamp > 0, "invalid source_timestamp");
        let now = self.blockchain().get_block_timestamp_seconds().as_u64_seconds();
        require!(
            source_timestamp <= now,
            "FUTURE_TIMESTAMP: source_timestamp cannot be in the future"
        );

        require!(
            device_did.is_zero() || self.registered_devices().contains(&device_did),
            "DEVICE_NOT_REGISTERED: device_did must be registered via registerDevice"
        );

        // device_signature validation: this is a structural assertion only.
        // The contract verifies that a signature value is present and meets the
        // minimum length for an ed25519 signature (64 bytes). **Cryptographic
        // verification** of the signature against the device's public key is
        // performed off-chain by the oracle validation pipeline before
        // submission. On-chain verification is deferred because the VM does
        // not natively support ed25519 signature verification at acceptable
        // gas cost.
        if source == SOURCE_IOT {
            require!(
                !device_signature.is_empty(),
                "INVALID_DEVICE_SIGNATURE: IoT readings require non-empty device_signature"
            );
            require!(
                device_signature.len() >= 64,
                "INVALID_DEVICE_SIGNATURE: signature must be at least 64 bytes (ed25519 minimum)"
            );
        }

        let seal_key = (pai_id.clone(), mrv_common::period_key(period_end));
        require!(
            !self.sealed_events().contains_key(&seal_key),
            "EVENT_ALREADY_SEALED: period already sealed for this PAI"
        );

        let window = self.get_coherence_window(source);
        require!(
            source_timestamp >= period_end.saturating_sub(window),
            "reading outside coherence window"
        );

        let reading = OracleReading {
            pai_id: pai_id.clone(),
            period_start,
            period_end,
            source,
            data_cid: data_cid.clone(),
            source_timestamp,
            device_did: device_did.clone(),
            device_signature: device_signature.clone(),
        };

        let reading_key = (pai_id.clone(), mrv_common::period_key(period_end), mrv_common::source_key(source));
        require!(
            !self.oracle_readings().contains_key(&reading_key),
            "READING_ALREADY_SUBMITTED: reading already exists for this source/period"
        );
        self.oracle_readings().insert(reading_key, reading);

        self.oracle_reading_submitted_event(&pai_id, &data_cid, &device_did);
    }

    /// Attempts to seal a monitoring period once quorum is satisfied.
    #[endpoint(trySeal)]
    fn try_seal(
        &self,
        pai_id: ManagedBuffer,
        period_end: u64,
    ) {
        let caller = self.blockchain().get_caller();
        require!(
            caller == self.blockchain().get_owner_address()
                || self.authorized_oracles().contains(&caller)
                || self.authorized_verifiers().contains(&caller),
            "only owner, authorized oracle, or authorized verifier can seal"
        );
        require!(!pai_id.is_empty(), "empty pai_id");

        let now = self.blockchain().get_block_timestamp_seconds().as_u64_seconds();
        require!(
            now > period_end,
            "PERIOD_NOT_ENDED: cannot seal before monitoring period closes"
        );

        let pk = mrv_common::period_key(period_end);

        let seal_key = (pai_id.clone(), pk.clone());
        require!(
            !self.sealed_events().contains_key(&seal_key),
            "EVENT_ALREADY_SEALED: period already sealed"
        );

        let mut cids = ManagedVec::<Self::Api, ManagedBuffer>::new();
        let mut has_iot = false;
        let mut has_satellite = false;
        let mut iot_cid = ManagedBuffer::new();
        let mut satellite_cid = ManagedBuffer::new();

        for source in [SOURCE_IOT, SOURCE_SATELLITE, SOURCE_GOVT_LAB] {
            let rk = (pai_id.clone(), pk.clone(), mrv_common::source_key(source));
            if let Some(reading) = self.oracle_readings().get(&rk) {
                let window = self.get_coherence_window(source);
                if reading.source_timestamp >= period_end.saturating_sub(window) {
                    cids.push(reading.data_cid.clone());
                    if source == SOURCE_IOT {
                        has_iot = true;
                        iot_cid = reading.data_cid.clone();
                    }
                    if source == SOURCE_SATELLITE {
                        has_satellite = true;
                        satellite_cid = reading.data_cid.clone();
                    }
                }
            }
        }

        let reading_count: u32 = cids.len() as u32;
        let quorum_val: u32 = self.quorum().get();
        require!(
            reading_count >= quorum_val,
            "insufficient oracle readings for quorum"
        );

        let semantic_discrepancy = if has_iot && has_satellite {
            self.check_semantic_divergence(&iot_cid, &satellite_cid)
        } else {
            false
        };

        if semantic_discrepancy {
            let ack_key = (pai_id.clone(), pk.clone());
            require!(
                self.discrepancy_acknowledged().contains_key(&ack_key),
                "DISCREPANCY_NOT_ACKNOWLEDGED: IoT-Satellite divergence detected — call acknowledgeDiscrepancy before sealing"
            );
        }

        let mrv_root = self.compute_mrv_root(&cids, semantic_discrepancy);

        let sealed = SealedEvent {
            pai_id: pai_id.clone(),
            period_end,
            mrv_root: mrv_root.clone(),
            reading_count,
            semantic_discrepancy,
            sealed_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
        };

        self.sealed_events().insert(seal_key, sealed);

        for source in [SOURCE_IOT, SOURCE_SATELLITE, SOURCE_GOVT_LAB] {
            let rk = (pai_id.clone(), pk.clone(), mrv_common::source_key(source));
            self.oracle_readings().remove(&rk);
        }

        self.discrepancy_acknowledged().remove(&(pai_id.clone(), pk.clone()));

        self.event_sealed_event(&pai_id, &mrv_root);
    }

    /// Seals a period after the longest coherence window has elapsed, even if
    /// quorum was not reached.
    ///
    /// When either the IoT or Satellite reading is missing, the sealed record
    /// is marked with `semantic_discrepancy = true`.
    #[only_owner]
    #[endpoint(forceSealAfterTimeout)]
    fn force_seal_after_timeout(
        &self,
        pai_id: ManagedBuffer,
        period_end: u64,
    ) {
        require!(!pai_id.is_empty(), "empty pai_id");

        let now = self.blockchain().get_block_timestamp_seconds().as_u64_seconds();
        require!(
            now > period_end,
            "period has not ended yet"
        );
        let timeout_window = self.govt_lab_window().get();
        require!(
            now >= period_end + timeout_window,
            "timeout window has not elapsed — wait for coherence window to expire"
        );

        let pk = mrv_common::period_key(period_end);
        let seal_key = (pai_id.clone(), pk.clone());
        require!(
            !self.sealed_events().contains_key(&seal_key),
            "EVENT_ALREADY_SEALED"
        );

        let mut cids = ManagedVec::<Self::Api, ManagedBuffer>::new();
        let mut iot_cid: Option<ManagedBuffer> = None;
        let mut satellite_cid: Option<ManagedBuffer> = None;
        for source in [SOURCE_IOT, SOURCE_SATELLITE, SOURCE_GOVT_LAB] {
            let rk = (pai_id.clone(), pk.clone(), mrv_common::source_key(source));
            if let Some(reading) = self.oracle_readings().get(&rk) {
                cids.push(reading.data_cid.clone());
                if source == SOURCE_IOT {
                    iot_cid = Some(reading.data_cid.clone());
                } else if source == SOURCE_SATELLITE {
                    satellite_cid = Some(reading.data_cid.clone());
                }
            }
        }
        require!(!cids.is_empty(), "no readings available to force-seal");

        let has_discrepancy = match (&iot_cid, &satellite_cid) {
            (Some(iot), Some(sat)) => self.check_semantic_divergence(iot, sat),
            _ => true,
        };

        let mrv_root = self.compute_mrv_root(&cids, has_discrepancy);

        let sealed = SealedEvent {
            pai_id: pai_id.clone(),
            period_end,
            mrv_root: mrv_root.clone(),
            reading_count: cids.len() as u32,
            semantic_discrepancy: has_discrepancy,
            sealed_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
        };

        self.sealed_events().insert(seal_key, sealed);

        for source in [SOURCE_IOT, SOURCE_SATELLITE, SOURCE_GOVT_LAB] {
            let rk = (pai_id.clone(), pk.clone(), mrv_common::source_key(source));
            self.oracle_readings().remove(&rk);
        }

        self.discrepancy_acknowledged().remove(&(pai_id.clone(), pk.clone()));

        self.force_sealed_event(&pai_id, &mrv_root);
    }

    /// VVB or governance acknowledges a semantic discrepancy between IoT and
    /// Satellite readings, allowing sealing to proceed despite divergence.
    /// Verifier authorization is managed separately from oracle authorization.
    #[endpoint(acknowledgeDiscrepancy)]
    fn acknowledge_discrepancy(
        &self,
        pai_id: ManagedBuffer,
        period_end: u64,
        acknowledgement_cid: ManagedBuffer,
    ) {
        let caller = self.blockchain().get_caller();
        require!(
            caller == self.blockchain().get_owner_address()
                || self.authorized_verifiers().contains(&caller),
            "only owner or authorized verifier (VVB) can acknowledge discrepancy"
        );
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(!acknowledgement_cid.is_empty(), "empty acknowledgement_cid");

        let pk = mrv_common::period_key(period_end);
        let iot_key = (pai_id.clone(), pk.clone(), mrv_common::source_key(SOURCE_IOT));
        let sat_key = (pai_id.clone(), pk.clone(), mrv_common::source_key(SOURCE_SATELLITE));
        require!(
            self.oracle_readings().contains_key(&iot_key) && self.oracle_readings().contains_key(&sat_key),
            "CANNOT_ACKNOWLEDGE: both IoT and Satellite readings must exist before acknowledging discrepancy"
        );

        let key = (pai_id.clone(), pk);
        self.discrepancy_acknowledged().insert(key, acknowledgement_cid);
        self.discrepancy_acknowledged_event(&pai_id);
    }

    /// Updates the minimum oracle reading quorum. Owner-only.
    #[only_owner]
    #[endpoint(setQuorum)]
    fn set_quorum(&self, quorum: u32) {
        require!(quorum >= 2, "quorum must be >= 2 (QUORUM_MIN)");
        self.quorum().set(quorum);
    }

    /// Updates per-source coherence windows. Owner-only.
    #[only_owner]
    #[endpoint(setCoherenceWindows)]
    fn set_coherence_windows(&self, iot_window: u64, satellite_window: u64, govt_lab_window: u64) {
        self.iot_window().set(iot_window);
        self.satellite_window().set(satellite_window);
        self.govt_lab_window().set(govt_lab_window);
    }

    /// Adds an address to the authorized oracle set. Owner-only.
    #[only_owner]
    #[endpoint(registerOracle)]
    fn register_oracle(&self, oracle: ManagedAddress) {
        require!(!oracle.is_zero(), "oracle must not be zero");
        self.authorized_oracles().insert(oracle);
    }

    /// Removes an address from the authorized oracle set. Owner-only.
    #[only_owner]
    #[endpoint(deregisterOracle)]
    fn deregister_oracle(&self, oracle: ManagedAddress) {
        self.authorized_oracles().swap_remove(&oracle);
    }

    #[view(isOracleAuthorized)]
    fn is_oracle_authorized(&self, oracle: ManagedAddress) -> bool {
        self.authorized_oracles().contains(&oracle)
    }

    /// Adds an address to the authorized verifier set. Owner-only.
    #[only_owner]
    #[endpoint(registerVerifier)]
    fn register_verifier(&self, verifier: ManagedAddress) {
        require!(!verifier.is_zero(), "verifier must not be zero");
        self.authorized_verifiers().insert(verifier);
    }

    /// Removes an address from the authorized verifier set. Owner-only.
    #[only_owner]
    #[endpoint(deregisterVerifier)]
    fn deregister_verifier(&self, verifier: ManagedAddress) {
        self.authorized_verifiers().swap_remove(&verifier);
    }

    #[view(isVerifierAuthorized)]
    fn is_verifier_authorized(&self, verifier: ManagedAddress) -> bool {
        self.authorized_verifiers().contains(&verifier)
    }

    /// Computes the MRV root from the collected oracle CIDs and discrepancy flag.
    ///
    /// CIDs are sorted lexicographically as byte strings before hashing.
    /// At most 3 CIDs are expected (IoT, Satellite, GovtLab); sorting uses
    /// index-based insertion sort to avoid cloning ManagedBuffer values.
    fn compute_mrv_root(
        &self,
        cids: &ManagedVec<ManagedBuffer>,
        semantic_discrepancy: bool,
    ) -> ManagedBuffer {
        let count = cids.len();
        require!(
            count <= MAX_ORACLE_SOURCES,
            "unexpected oracle source count"
        );
        let mut sorted_indices: [usize; 3] = [0, 1, 2];
        let actual_count = if count > 3 { 3 } else { count };

        for i in 1..actual_count {
            let mut j = i;
            while j > 0 {
                let a = cids.get(sorted_indices[j - 1]).to_boxed_bytes();
                let b = cids.get(sorted_indices[j]).to_boxed_bytes();
                if a.as_slice() > b.as_slice() {
                    sorted_indices.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        let mut preimage = ManagedBuffer::new();
        for &idx in sorted_indices.iter().take(actual_count) {
            preimage.append(&cids.get(idx));
        }
        preimage.append_bytes(&[if semantic_discrepancy { 0x01u8 } else { 0x00u8 }]);

        self.crypto().sha256(&preimage).as_managed_buffer().clone()
    }

    /// Returns `true` if IoT and Satellite readings diverge beyond threshold.
    ///
    /// If both CIDs encode a numeric NDVI value as a decimal ASCII string
    /// (e.g. `"7500"` for 0.75 in bps), the function parses them and checks
    /// whether the absolute difference exceeds `divergence_threshold_bps`.
    ///
    /// When CIDs are not numeric (content-addressed hashes), CID equality
    /// is used as the divergence check. VVB must manually acknowledge the
    /// discrepancy via `acknowledgeDiscrepancy` before sealing can proceed.
    fn check_semantic_divergence(
        &self,
        iot_cid: &ManagedBuffer,
        satellite_cid: &ManagedBuffer,
    ) -> bool {
        if iot_cid == satellite_cid {
            return false;
        }
        let iot_bytes = iot_cid.to_boxed_bytes();
        let sat_bytes = satellite_cid.to_boxed_bytes();
        if let (Some(iot_val), Some(sat_val)) = (
            self.parse_ascii_u64(iot_bytes.as_slice()),
            self.parse_ascii_u64(sat_bytes.as_slice()),
        ) {
            let diff = iot_val.abs_diff(sat_val);
            let threshold = self.divergence_threshold_bps().get();
            diff > threshold
        } else {
            true
        }
    }

    /// Parses a byte slice as an ASCII decimal u64. Returns None if any
    /// byte is not an ASCII digit or the slice is empty.
    fn parse_ascii_u64(&self, bytes: &[u8]) -> Option<u64> {
        if bytes.is_empty() {
            return None;
        }
        let mut result: u64 = 0;
        for &b in bytes {
            if !b.is_ascii_digit() {
                return None;
            }
            result = result.checked_mul(10)?.checked_add((b - b'0') as u64)?;
        }
        Some(result)
    }

    /// Returns the configured time-coherence window (in seconds) for a source type.
    fn get_coherence_window(&self, source: u8) -> u64 {
        match source {
            SOURCE_IOT => self.iot_window().get(),
            SOURCE_SATELLITE => self.satellite_window().get(),
            SOURCE_GOVT_LAB => self.govt_lab_window().get(),
            _ => 0,
        }
    }

    /// Registers a device identity for IoT reading validation. Owner-only.
    #[only_owner]
    #[endpoint(registerDevice)]
    fn register_device(&self, device_did: ManagedAddress) {
        require!(!device_did.is_zero(), "device_did must not be zero");
        self.registered_devices().insert(device_did);
    }

    /// Removes a device identity. Owner-only.
    #[only_owner]
    #[endpoint(deregisterDevice)]
    fn deregister_device(&self, device_did: ManagedAddress) {
        self.registered_devices().swap_remove(&device_did);
    }

    #[view(isDeviceRegistered)]
    fn is_device_registered(&self, device_did: ManagedAddress) -> bool {
        self.registered_devices().contains(&device_did)
    }

    /// Proposes replacing an existing oracle with a substitute, scoped to
    /// a time window (`scope_end_epoch`).
    #[only_owner]
    #[endpoint(proposeOracleUpdate)]
    fn propose_oracle_update(
        &self,
        current_oracle: ManagedAddress,
        proposed_oracle: ManagedAddress,
        scope_end_epoch: u64,
    ) {
        require!(!current_oracle.is_zero(), "current_oracle must not be zero");
        require!(!proposed_oracle.is_zero(), "proposed_oracle must not be zero");
        require!(
            self.authorized_oracles().contains(&current_oracle),
            "current_oracle not in authorized set"
        );
        require!(
            scope_end_epoch > self.blockchain().get_block_epoch(),
            "scope_end_epoch must be in the future"
        );

        let proposed_at = self.blockchain().get_block_timestamp_seconds().as_u64_seconds();
        self.pending_oracle_proposals().insert(
            current_oracle.clone(),
            (proposed_oracle.clone(), scope_end_epoch, proposed_at),
        );

        self.oracle_update_proposed_event(&current_oracle, &proposed_oracle, scope_end_epoch);
    }

    /// Accepts a pending oracle rotation. Only the proposed oracle may call this.
    #[endpoint(acceptOracleUpdate)]
    fn accept_oracle_update(&self, current_oracle: ManagedAddress) {
        let caller = self.blockchain().get_caller();
        require!(
            self.pending_oracle_proposals().contains_key(&current_oracle),
            "no pending proposal for this oracle"
        );

        let (proposed_oracle, scope_end_epoch, _proposed_at) =
            self.pending_oracle_proposals().get(&current_oracle).unwrap();

        require!(
            caller == proposed_oracle,
            "only the proposed oracle can accept"
        );
        require!(
            self.blockchain().get_block_epoch() <= scope_end_epoch,
            "proposal scope has expired"
        );

        self.authorized_oracles().swap_remove(&current_oracle);
        self.authorized_oracles().insert(proposed_oracle.clone());
        self.pending_oracle_proposals().remove(&current_oracle);

        self.oracle_update_accepted_event(&current_oracle, &proposed_oracle);
    }

    /// Removes an expired oracle proposal. Any caller may trigger this
    /// once the proposal's scope_end_epoch has passed.
    #[endpoint(cancelExpiredProposal)]
    fn cancel_expired_proposal(&self, current_oracle: ManagedAddress) {
        require!(
            self.pending_oracle_proposals().contains_key(&current_oracle),
            "no pending proposal for this oracle"
        );
        let (_proposed, scope_end_epoch, _proposed_at) =
            self.pending_oracle_proposals().get(&current_oracle).unwrap();
        require!(
            self.blockchain().get_block_epoch() > scope_end_epoch,
            "proposal has not expired yet"
        );
        self.pending_oracle_proposals().remove(&current_oracle);
    }

    #[view(getSealedEvent)]
    fn get_sealed_event(
        &self,
        pai_id: ManagedBuffer,
        period_end: u64,
    ) -> OptionalValue<SealedEvent<Self::Api>> {
        let key = (pai_id, mrv_common::period_key(period_end));
        match self.sealed_events().get(&key) {
            Some(e) => OptionalValue::Some(e),
            None => OptionalValue::None,
        }
    }

    #[view(isSealed)]
    fn is_sealed(&self, pai_id: ManagedBuffer, period_end: u64) -> bool {
        let key = (pai_id, mrv_common::period_key(period_end));
        self.sealed_events().contains_key(&key)
    }

    #[storage_mapper("oracleReadings")]
    fn oracle_readings(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer, ManagedBuffer), OracleReading<Self::Api>>;

    #[storage_mapper("sealedEvents")]
    fn sealed_events(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer), SealedEvent<Self::Api>>;

    #[storage_mapper("quorum")]
    fn quorum(&self) -> SingleValueMapper<u32>;

    #[storage_mapper("iotWindow")]
    fn iot_window(&self) -> SingleValueMapper<u64>;

    #[storage_mapper("satelliteWindow")]
    fn satellite_window(&self) -> SingleValueMapper<u64>;

    #[storage_mapper("govtLabWindow")]
    fn govt_lab_window(&self) -> SingleValueMapper<u64>;

    /// Configured divergence threshold in basis points for numeric NDVI
    /// comparison between IoT and Satellite readings.
    #[storage_mapper("divergenceThresholdBps")]
    fn divergence_threshold_bps(&self) -> SingleValueMapper<u64>;

    /// Discrepancy acknowledgements keyed by `(pai_id, period_key)`.
    #[storage_mapper("discrepancyAcknowledged")]
    fn discrepancy_acknowledged(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), ManagedBuffer>;

    /// Authorized oracle addresses.
    #[storage_mapper("authorizedOracles")]
    fn authorized_oracles(&self) -> UnorderedSetMapper<ManagedAddress>;

    /// Authorized verifier addresses used for discrepancy acknowledgements.
    #[storage_mapper("authorizedVerifiers")]
    fn authorized_verifiers(&self) -> UnorderedSetMapper<ManagedAddress>;

    /// Registered device identities.
    #[storage_mapper("registeredDevices")]
    fn registered_devices(&self) -> UnorderedSetMapper<ManagedAddress>;

    /// Pending oracle rotation proposals keyed by current oracle address.
    #[storage_mapper("pendingOracleProposals")]
    fn pending_oracle_proposals(&self) -> MapMapper<ManagedAddress, (ManagedAddress, u64, u64)>;

    #[event("oracleReadingSubmitted")]
    fn oracle_reading_submitted_event(
        &self,
        #[indexed] pai_id: &ManagedBuffer,
        #[indexed] data_cid: &ManagedBuffer,
        #[indexed] device_did: &ManagedAddress,
    );

    #[event("eventSealed")]
    fn event_sealed_event(
        &self,
        #[indexed] pai_id: &ManagedBuffer,
        mrv_root: &ManagedBuffer,
    );

    #[event("discrepancyAcknowledged")]
    fn discrepancy_acknowledged_event(
        &self,
        #[indexed] pai_id: &ManagedBuffer,
    );

    #[event("forceSealed")]
    fn force_sealed_event(
        &self,
        #[indexed] pai_id: &ManagedBuffer,
        mrv_root: &ManagedBuffer,
    );

    #[event("oracleUpdateProposed")]
    fn oracle_update_proposed_event(
        &self,
        #[indexed] current_oracle: &ManagedAddress,
        #[indexed] proposed_oracle: &ManagedAddress,
        scope_end_epoch: u64,
    );

    #[event("oracleUpdateAccepted")]
    fn oracle_update_accepted_event(
        &self,
        #[indexed] replaced_oracle: &ManagedAddress,
        #[indexed] new_oracle: &ManagedAddress,
    );

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
