#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

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
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
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
/// is capped at 64 levels to bound on-chain execution cost.
///
/// `#[payable("*")]` is used instead of a fixed token identifier because
/// the accepted COME token ID is set dynamically at `init` time and may
/// differ across deployments.
#[multiversx_sc::contract]
pub trait IncomeDistribution: mrv_common::MrvGovernanceModule {
    #[init]
    fn init(&self, governance: ManagedAddress, come_token_id: TokenIdentifier) {
        require!(!governance.is_zero(), "governance must not be zero");
        require!(come_token_id.is_valid_esdt_identifier(), "invalid COME token ID");
        self.governance().set(governance);
        self.come_token_id().set(come_token_id);
        self.storage_version().set(1u32);
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
        self.require_governance_or_owner();
        require!(!distribution_id.is_empty(), "empty distribution_id");
        require!(
            distribution_id.len() <= MAX_DISTRIBUTION_ID_LEN,
            "distribution_id exceeds maximum length"
        );
        require!(merkle_root.len() == 32, "merkle_root must be 32 bytes");
        let zero_root = ManagedBuffer::from(&[0u8; 32]);
        require!(merkle_root != zero_root, "merkle_root must not be all zeros");
        require!(!manifest_cid.is_empty(), "empty manifest_cid");

        let current_epoch = self.blockchain().get_block_epoch();
        require!(
            expiry_epoch >= current_epoch + MINIMUM_CLAIM_WINDOW_EPOCHS,
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
            funded_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
            reclaimed: false,
        };

        self.distributions().insert(distribution_id.clone(), record);
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
        require!(
            !self.claimed().contains_key(&claim_key),
            "ALREADY_CLAIMED"
        );

        let mut leaf_preimage = ManagedBuffer::new();
        leaf_preimage.append(&distribution_id);
        leaf_preimage.append(holder.as_managed_buffer());
        leaf_preimage.append(&amount_scaled.to_bytes_be_buffer());
        let leaf = self.crypto().keccak256(&leaf_preimage);

        let mut current_hash = leaf.as_managed_buffer().clone();
        for i in 0..merkle_proof.len() {
            let sibling = merkle_proof.get(i);
            let mut combined = ManagedBuffer::new();
            let current_bytes = current_hash.to_boxed_bytes();
            let sibling_bytes = sibling.to_boxed_bytes();
            if current_bytes.as_slice() <= sibling_bytes.as_slice() {
                combined.append(&current_hash);
                combined.append(&sibling);
            } else {
                combined.append(&sibling);
                combined.append(&current_hash);
            }
            current_hash = self.crypto().keccak256(&combined).as_managed_buffer().clone();
        }
        require!(current_hash == dist.merkle_root, "INVALID_MERKLE_PROOF");

        require!(
            &dist.total_claimed_scaled + &amount_scaled <= dist.total_amount_scaled,
            "CLAIMS_EXCEED_FUNDED: cumulative claims would exceed distribution total"
        );

        let come_token = self.come_token_id().get();
        let sc_balance = self.blockchain().get_sc_balance(
            EgldOrEsdtTokenIdentifier::esdt(come_token.clone()),
            0u64,
        );
        require!(
            sc_balance >= amount_scaled,
            "INSUFFICIENT_CONTRACT_BALANCE: contract does not hold enough COME to pay this claim"
        );

        self.claimed().insert(claim_key, amount_scaled.clone());
        self.distributions().entry(distribution_id.clone()).and_modify(|r| {
            r.total_claimed_scaled += &amount_scaled;
        });

        self.send().direct_esdt(
            &holder,
            &come_token,
            0u64,
            &amount_scaled,
        );

        self.yield_claimed_event(&distribution_id, &holder, &amount_scaled);
    }

    /// Pauses claims for a distribution.
    #[endpoint(pauseDistribution)]
    fn pause_distribution(&self, distribution_id: ManagedBuffer) {
        self.require_governance_or_owner();
        self.distribution_paused(&distribution_id).set(true);
        self.distribution_paused_event(&distribution_id);
    }

    /// Resumes claims for a distribution.
    #[endpoint(unpauseDistribution)]
    fn unpause_distribution(&self, distribution_id: ManagedBuffer) {
        self.require_governance_or_owner();
        self.distribution_paused(&distribution_id).clear();
        self.distribution_unpaused_event(&distribution_id);
    }

    /// Returns unclaimed funds to the issuer after the distribution expires.
    #[endpoint(reclaimExpired)]
    fn reclaim_expired(&self, distribution_id: ManagedBuffer) {
        self.require_governance_or_owner();
        let dist = self.distributions().get(&distribution_id);
        require!(dist.is_some(), "distribution not found");
        let dist = dist.unwrap();

        let current_epoch = self.blockchain().get_block_epoch();
        require!(current_epoch > dist.expiry_epoch, "distribution not yet expired");
        require!(!dist.reclaimed, "already reclaimed");

        let unclaimed = &dist.total_amount_scaled - &dist.total_claimed_scaled;

        self.distributions().entry(distribution_id.clone()).and_modify(|r| {
            r.reclaimed = true;
        });

        if unclaimed > 0u64 {
            let sc_balance = self.blockchain().get_sc_balance(EgldOrEsdtTokenIdentifier::esdt(self.come_token_id().get()), 0u64);
            let transfer_amount = if unclaimed <= sc_balance { unclaimed.clone() } else { sc_balance.clone() };
            if sc_balance < unclaimed {
                let shortfall = &unclaimed - &sc_balance;
                self.reclaim_shortfall(&distribution_id).set(shortfall.clone());
                self.shortfall_detected_event(&distribution_id, &unclaimed, &sc_balance, &shortfall);
            }
            if transfer_amount > 0u64 {
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
        self.require_governance_or_owner();
        let dist = self.distributions().get(&distribution_id);
        require!(dist.is_some(), "distribution not found");
        let dist = dist.unwrap();

        let shortfall = self.reclaim_shortfall(&distribution_id).get();
        require!(shortfall > 0u64, "NO_SHORTFALL: no shortfall recorded for this distribution");

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

    #[storage_mapper("distributions")]
    fn distributions(&self) -> MapMapper<ManagedBuffer, DistributionRecord<Self::Api>>;

    #[storage_mapper("claimed")]
    fn claimed(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), BigUint>;

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

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    #[upgrade]
    fn upgrade(&self) {
        // Reserved for future storage migrations. Version is set to 1
        // during init; bump this value when a layout-breaking change is
        // introduced and add the corresponding migration logic here.
    }
}
