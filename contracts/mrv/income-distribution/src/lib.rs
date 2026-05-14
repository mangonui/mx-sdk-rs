#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use mrv_common::resolve_storage_version_upgrade;

pub mod governance_proxy;
pub mod income_distribution_proxy;

/// Minimum epochs between funding and expiry. On the Dharitri chain each
/// epoch is approximately 6 seconds (configurable per network), so 5 000
/// epochs ≈ 8.3 hours — a conservative lower bound to prevent trivially
/// short claim windows.
const MINIMUM_CLAIM_WINDOW_EPOCHS: u64 = 5_000;

/// Maximum allowed length (in bytes) for a distribution identifier to
/// prevent storage-key bloat.
const MAX_DISTRIBUTION_ID_LEN: usize = 128;

/// Merkle-gated COME distribution record with funding, claim tracking, and expiry.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct DistributionRecord<M: ManagedTypeApi> {
    pub distribution_id: ManagedBuffer<M>,
    pub issuer: ManagedAddress<M>,
    pub merkle_root: ManagedBuffer<M>,
    /// Off-chain reference block for the holder snapshot. Not enforced
    /// on-chain — consumers use this to correlate the Merkle tree with the
    /// block at which balances were snapshotted.
    pub snapshot_block: u64,
    pub manifest_cid: ManagedBuffer<M>,
    pub total_amount_scaled: BigUint<M>,
    pub total_claimed_scaled: BigUint<M>,
    pub expiry_epoch: u64,
    pub funded_at: u64,
    pub reclaimed: bool,
}

/// Merkle-based income distribution contract.
///
/// Issuers fund distributions with COME, and holders claim against a
/// recorded Merkle root until the configured expiry. Merkle proof depth
/// is capped at 64 levels to bound on-chain execution cost. Claim leaves
/// use `MRV_YIELD_CLAIM_LEAF_V2` and bind the distribution total so a
/// Merkle tree built for a different funded amount cannot be replayed.
///
/// `#[payable("*")]` is used instead of a fixed token identifier because
/// the accepted COME token ID is set dynamically at `init` time and may
/// differ across deployments.
#[multiversx_sc::contract]
pub trait IncomeDistribution: mrv_common::MrvGovernanceModule {
    #[init]
    fn init(&self, governance: ManagedAddress, come_token_id: TokenIdentifier) {
        require!(!governance.is_zero(), "governance must not be zero");
        require!(
            come_token_id.is_valid_esdt_identifier(),
            "invalid COME token ID"
        );
        self.governance().set(governance);
        self.come_token_id().set(come_token_id);
        self.storage_version().set(1u32);
    }

    #[endpoint(setGovernanceReadAddress)]
    fn set_governance_read_address(&self, addr: ManagedAddress) {
        self.require_governance_or_owner();
        require!(!addr.is_zero(), "governance_read_address must not be zero");
        self.governance_read_address().set(&addr);
        self.governance_read_address_updated_event(&addr);
    }

    #[endpoint(clearGovernanceReadAddress)]
    fn clear_governance_read_address(&self) {
        self.require_governance_or_owner();
        self.governance_read_address().clear();
        self.governance_read_address_cleared_event();
    }

    /// Funds a distribution with COME and records its Merkle root and expiry.
    #[payable("*")]
    #[endpoint(fundDistribution)]
    fn fund_distribution(
        &self,
        distribution_id: ManagedBuffer,
        merkle_root: ManagedBuffer,
        snapshot_block: u64,
        manifest_cid: ManagedBuffer,
        expiry_epoch: u64,
    ) {
        self.require_not_paused();
        self.require_governance_or_owner();
        require!(!distribution_id.is_empty(), "empty distribution_id");
        require!(
            distribution_id.len() <= MAX_DISTRIBUTION_ID_LEN,
            "distribution_id exceeds maximum length"
        );
        require!(merkle_root.len() == 32, "merkle_root must be 32 bytes");
        let zero_root = ManagedBuffer::from(&[0u8; 32]);
        require!(
            merkle_root != zero_root,
            "merkle_root must not be all zeros"
        );
        require!(!manifest_cid.is_empty(), "empty manifest_cid");

        let current_epoch = self.blockchain().get_block_epoch();
        let minimum_expiry_epoch = current_epoch
            .checked_add(MINIMUM_CLAIM_WINDOW_EPOCHS)
            .unwrap_or_else(|| sc_panic!("expiry window overflow"));
        require!(
            expiry_epoch >= minimum_expiry_epoch,
            "expiry_epoch must be at least MINIMUM_CLAIM_WINDOW_EPOCHS from now"
        );

        require!(
            !self.distributions().contains_key(&distribution_id),
            "distribution already exists"
        );

        let payment = self.call_value().single_esdt();
        require!(
            payment.token_identifier == self.come_token_id().get(),
            "must pay with COME token"
        );
        require!(
            payment.token_nonce == 0,
            "FUNGIBLE_ONLY: token nonce must be 0"
        );
        require!(payment.amount > 0u64, "must fund with positive amount");

        let record = DistributionRecord {
            distribution_id: distribution_id.clone(),
            issuer: self.blockchain().get_caller(),
            merkle_root,
            snapshot_block,
            manifest_cid,
            total_amount_scaled: payment.amount.clone(),
            total_claimed_scaled: BigUint::zero(),
            expiry_epoch,
            funded_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
            reclaimed: false,
        };

        self.distributions().insert(distribution_id.clone(), record);
        self.distribution_escrow(&distribution_id)
            .set(payment.amount.clone());
        self.distribution_funded_event(&distribution_id, &payment.amount);
    }

    /// Claims a funded amount for the caller by verifying a keccak256 Merkle proof.
    ///
    /// Proof depth is capped at 64 levels to bound execution cost. The leaf
    /// binds `distribution_id` to prevent cross-distribution replay.
    #[endpoint(claimYield)]
    fn claim_yield(
        &self,
        distribution_id: ManagedBuffer,
        amount_scaled: BigUint,
        merkle_proof: ManagedVec<ManagedBuffer>,
    ) {
        self.require_not_paused();
        require!(merkle_proof.len() <= 64, "MERKLE_PROOF_TOO_DEEP");
        require!(
            !self.distribution_paused(&distribution_id).get(),
            "DISTRIBUTION_PAUSED: claims are temporarily suspended"
        );
        let holder = self.blockchain().get_caller();
        let dist = self.distributions().get(&distribution_id);
        require!(dist.is_some(), "distribution not found");
        let dist = dist.unwrap();

        let current_epoch = self.blockchain().get_block_epoch();
        require!(current_epoch <= dist.expiry_epoch, "DISTRIBUTION_EXPIRED");
        require!(!dist.reclaimed, "distribution already reclaimed");

        let claim_key = (distribution_id.clone(), holder.as_managed_buffer().clone());
        require!(!self.claimed().contains_key(&claim_key), "ALREADY_CLAIMED");

        let mut leaf_preimage = ManagedBuffer::from(b"MRV_YIELD_CLAIM_LEAF_V2");
        self.push_len_prefixed(&mut leaf_preimage, &distribution_id);
        self.push_len_prefixed(&mut leaf_preimage, holder.as_managed_buffer());
        self.push_len_prefixed(&mut leaf_preimage, &amount_scaled.to_bytes_be_buffer());
        self.push_len_prefixed(
            &mut leaf_preimage,
            &dist.total_amount_scaled.to_bytes_be_buffer(),
        );
        let leaf = self.crypto().keccak256(&leaf_preimage);

        let mut current_hash = leaf.as_managed_buffer().clone();
        for i in 0..merkle_proof.len() {
            let sibling = merkle_proof.get(i);
            let mut combined = ManagedBuffer::new();
            if self.managed_buffer_lex_le(&current_hash, &sibling) {
                combined.append(&current_hash);
                combined.append(&sibling);
            } else {
                combined.append(&sibling);
                combined.append(&current_hash);
            }
            current_hash = self
                .crypto()
                .keccak256(&combined)
                .as_managed_buffer()
                .clone();
        }
        require!(current_hash == dist.merkle_root, "INVALID_MERKLE_PROOF");

        require!(
            &dist.total_claimed_scaled + &amount_scaled <= dist.total_amount_scaled,
            "CLAIMS_EXCEED_FUNDED: cumulative claims would exceed distribution total"
        );

        let distribution_escrow = self.distribution_escrow(&distribution_id).get();
        require!(
            distribution_escrow >= amount_scaled,
            "INSUFFICIENT_DISTRIBUTION_ESCROW: distribution does not hold enough COME for this claim"
        );

        let come_token = self.come_token_id().get();
        let sc_balance = self
            .blockchain()
            .get_sc_balance(EgldOrEsdtTokenIdentifier::esdt(come_token.clone()), 0u64);
        require!(
            sc_balance >= amount_scaled,
            "INSUFFICIENT_CONTRACT_BALANCE: contract does not hold enough COME to pay this claim"
        );

        // M-07 (AUD-013) reassessment, 2026-04-20:
        //
        // The audit finding proposed that "claim state committed before
        // transfer means a transfer failure leaves an orphan claim." That
        // concern assumes EVM-style non-atomic failure semantics. On
        // MultiversX, `self.send().direct_esdt(...)` resolves to a
        // `Tx::new_tx_from_sc().transfer()` in the send-wrapper, which
        // queues an ESDT transfer action on the current transaction. If
        // that transfer fails (recipient blocked by DRWA gate,
        // insufficient contract balance at dispatch, etc.), the ENTIRE
        // transaction reverts — including the `self.claimed().insert()`
        // mutation below. There is no reachable state where the claim
        // flag is set but the holder has not received COME.
        //
        // The current layout is the canonical Checks → Effects →
        // Interactions order: checks (lines 140-192), effects (the two
        // `insert` / `and_modify` calls below), interactions (the
        // `send().direct_esdt` call). Do NOT invert this order without
        // re-verifying tx-atomicity guarantees against the framework
        // version in use.
        self.claimed().insert(claim_key, amount_scaled.clone());
        self.distributions()
            .entry(distribution_id.clone())
            .and_modify(|r| {
                r.total_claimed_scaled += &amount_scaled;
            });
        self.distribution_escrow(&distribution_id)
            .update(|balance| *balance -= &amount_scaled);

        self.send()
            .direct_esdt(&holder, &come_token, 0u64, &amount_scaled);

        self.yield_claimed_event(&distribution_id, &holder, &amount_scaled);
    }

    fn managed_buffer_lex_le(&self, left: &ManagedBuffer, right: &ManagedBuffer) -> bool {
        let left_len = left.len();
        let right_len = right.len();
        let shared_len = core::cmp::min(left_len, right_len);
        let mut left_byte = [0u8; 1];
        let mut right_byte = [0u8; 1];

        for index in 0..shared_len {
            left.load_slice(index, &mut left_byte);
            right.load_slice(index, &mut right_byte);
            if left_byte[0] < right_byte[0] {
                return true;
            }
            if left_byte[0] > right_byte[0] {
                return false;
            }
        }

        left_len <= right_len
    }

    /// Pauses claims for a distribution.
    #[endpoint(pauseDistribution)]
    fn pause_distribution(&self, distribution_id: ManagedBuffer) {
        self.require_not_paused();
        self.require_governance_or_owner();
        self.distribution_paused(&distribution_id).set(true);
        self.distribution_paused_event(&distribution_id);
    }

    /// Resumes claims for a distribution.
    #[endpoint(unpauseDistribution)]
    fn unpause_distribution(&self, distribution_id: ManagedBuffer) {
        self.require_not_paused();
        self.require_governance_or_owner();
        self.distribution_paused(&distribution_id).clear();
        self.distribution_unpaused_event(&distribution_id);
    }

    /// Returns unclaimed funds to the issuer after the distribution expires.
    #[endpoint(reclaimExpired)]
    fn reclaim_expired(&self, distribution_id: ManagedBuffer) {
        self.require_not_paused();
        self.require_governance_or_owner();
        let dist = self.distributions().get(&distribution_id);
        require!(dist.is_some(), "distribution not found");
        let dist = dist.unwrap();

        let current_epoch = self.blockchain().get_block_epoch();
        require!(
            current_epoch > dist.expiry_epoch,
            "distribution not yet expired"
        );
        require!(!dist.reclaimed, "already reclaimed");

        let unclaimed = &dist.total_amount_scaled - &dist.total_claimed_scaled;

        self.distributions()
            .entry(distribution_id.clone())
            .and_modify(|r| {
                r.reclaimed = true;
            });

        if unclaimed > 0u64 {
            let escrow_balance = self.distribution_escrow(&distribution_id).get();
            let escrow_available = if escrow_balance <= unclaimed {
                escrow_balance
            } else {
                unclaimed.clone()
            };
            let sc_balance = self.blockchain().get_sc_balance(
                EgldOrEsdtTokenIdentifier::esdt(self.come_token_id().get()),
                0u64,
            );
            let transfer_amount = if escrow_available <= sc_balance {
                escrow_available.clone()
            } else {
                sc_balance.clone()
            };
            if transfer_amount < unclaimed {
                let shortfall = &unclaimed - &transfer_amount;
                self.reclaim_shortfall(&distribution_id)
                    .set(shortfall.clone());
                self.shortfall_detected_event(
                    &distribution_id,
                    &unclaimed,
                    &transfer_amount,
                    &shortfall,
                );
            }
            if transfer_amount > 0u64 {
                self.distribution_escrow(&distribution_id)
                    .update(|balance| *balance -= &transfer_amount);
                self.send().direct_esdt(
                    &dist.issuer,
                    &self.come_token_id().get(),
                    0u64,
                    &transfer_amount,
                );
            }
        }

        self.distribution_reclaimed_event(&distribution_id);
    }

    /// Recovers a shortfall by accepting a COME payment and forwarding it
    /// to the original issuer of the distribution. Only governance or owner
    /// may call. The shortfall amount is reduced accordingly.
    #[payable("*")]
    #[endpoint(recoverShortfall)]
    fn recover_shortfall(&self, distribution_id: ManagedBuffer) {
        self.require_not_paused();
        self.require_governance_or_owner();
        let dist = self.distributions().get(&distribution_id);
        require!(dist.is_some(), "distribution not found");
        let dist = dist.unwrap();

        let shortfall = self.reclaim_shortfall(&distribution_id).get();
        require!(
            shortfall > 0u64,
            "NO_SHORTFALL: no shortfall recorded for this distribution"
        );

        let payment = self.call_value().single_esdt();
        require!(
            payment.token_identifier == self.come_token_id().get(),
            "must pay with COME token"
        );
        require!(payment.amount > 0u64, "must recover with positive amount");
        require!(
            payment.amount <= shortfall,
            "RECOVERY_EXCEEDS_SHORTFALL: payment exceeds recorded shortfall"
        );

        let new_shortfall = &shortfall - &payment.amount;
        if new_shortfall > 0u64 {
            self.reclaim_shortfall(&distribution_id).set(new_shortfall);
        } else {
            self.reclaim_shortfall(&distribution_id).clear();
        }

        self.send().direct_esdt(
            &dist.issuer,
            &self.come_token_id().get(),
            0u64,
            &payment.amount,
        );

        self.shortfall_recovered_event(&distribution_id, &payment.amount);
    }

    #[view(getDistribution)]
    fn get_distribution(
        &self,
        distribution_id: ManagedBuffer,
    ) -> OptionalValue<DistributionRecord<Self::Api>> {
        match self.distributions().get(&distribution_id) {
            Some(r) => OptionalValue::Some(r),
            None => OptionalValue::None,
        }
    }

    #[view(isClaimed)]
    fn is_claimed(&self, distribution_id: ManagedBuffer, holder: ManagedAddress) -> bool {
        let key = (distribution_id, holder.as_managed_buffer().clone());
        self.claimed().contains_key(&key)
    }

    /// Returns the number of epochs remaining before the distribution expires,
    /// or 0 if the distribution does not exist or is already expired.
    #[view(getClaimWindow)]
    fn get_claim_window(&self, distribution_id: ManagedBuffer) -> u64 {
        match self.distributions().get(&distribution_id) {
            Some(dist) => {
                let current_epoch = self.blockchain().get_block_epoch();
                if current_epoch >= dist.expiry_epoch {
                    0u64
                } else {
                    dist.expiry_epoch - current_epoch
                }
            }
            None => 0u64,
        }
    }

    #[storage_mapper("comeTokenId")]
    fn come_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    #[view(getGovernanceReadAddress)]
    #[storage_mapper("governanceReadAddress")]
    fn governance_read_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[storage_mapper("distributions")]
    fn distributions(&self) -> MapMapper<ManagedBuffer, DistributionRecord<Self::Api>>;

    #[storage_mapper("claimed")]
    fn claimed(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), BigUint>;

    #[view(getDistributionEscrow)]
    #[storage_mapper("distributionEscrow")]
    fn distribution_escrow(&self, distribution_id: &ManagedBuffer) -> SingleValueMapper<BigUint>;

    /// Pause flag keyed by distribution identifier.
    #[storage_mapper("distributionPaused")]
    fn distribution_paused(&self, distribution_id: &ManagedBuffer) -> SingleValueMapper<bool>;

    #[event("distributionFunded")]
    fn distribution_funded_event(
        &self,
        #[indexed] distribution_id: &ManagedBuffer,
        total_amount: &BigUint,
    );

    #[event("yieldClaimed")]
    fn yield_claimed_event(
        &self,
        #[indexed] distribution_id: &ManagedBuffer,
        #[indexed] holder: &ManagedAddress,
        amount: &BigUint,
    );

    /// Shortfall amount recorded when sc_balance < unclaimed during reclaim.
    #[storage_mapper("reclaimShortfall")]
    fn reclaim_shortfall(&self, distribution_id: &ManagedBuffer) -> SingleValueMapper<BigUint>;

    #[event("shortfallDetected")]
    fn shortfall_detected_event(
        &self,
        #[indexed] distribution_id: &ManagedBuffer,
        #[indexed] expected_amount: &BigUint,
        #[indexed] actual_amount: &BigUint,
        shortfall: &BigUint,
    );

    #[event("shortfallRecovered")]
    fn shortfall_recovered_event(
        &self,
        #[indexed] distribution_id: &ManagedBuffer,
        recovered_amount: &BigUint,
    );

    #[event("distributionReclaimed")]
    fn distribution_reclaimed_event(&self, #[indexed] distribution_id: &ManagedBuffer);

    #[event("distributionPaused")]
    fn distribution_paused_event(&self, #[indexed] distribution_id: &ManagedBuffer);

    #[event("distributionUnpaused")]
    fn distribution_unpaused_event(&self, #[indexed] distribution_id: &ManagedBuffer);

    #[event("governanceReadAddressUpdated")]
    fn governance_read_address_updated_event(
        &self,
        #[indexed] governance_read_address: &ManagedAddress,
    );

    #[event("governanceReadAddressCleared")]
    fn governance_read_address_cleared_event(&self);

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    fn require_not_paused(&self) {
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

        use governance_proxy::GovernanceProxy;

        let governance_read_address = self.governance_read_address().get();
        let gas_for_query = self.blockchain().get_gas_left() / 16;

        let paused: bool = self
            .tx()
            .to(&governance_read_address)
            .gas(gas_for_query)
            .typed(GovernanceProxy)
            .get_paused()
            .returns(ReturnsResult)
            .sync_call_readonly();

        require!(!paused, "MRV_GOVERNANCE_PAUSED");
    }

    #[upgrade]
    fn upgrade(&self) {
        let stored = self.storage_version().get();
        let target = resolve_storage_version_upgrade(stored, 1u32, 1u32)
            .unwrap_or_else(|message| sc_panic!(message));
        if stored != target {
            self.storage_version().set(target);
        }
    }

    fn push_len_prefixed(&self, dest: &mut ManagedBuffer, value: &ManagedBuffer) {
        let len = value.len() as u32;
        dest.append_bytes(&len.to_be_bytes());
        dest.append(value);
    }
}
