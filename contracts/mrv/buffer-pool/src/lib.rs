#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

/// Replenishment threshold above which governance approval is required.
const REPLENISHMENT_GOVERNANCE_THRESHOLD_BPS: u64 = 1_000;
/// Minimum epoch interval between replenishments for the same project.
const REPLENISHMENT_COOLDOWN_EPOCHS: u64 = 1_500;

/// Per-project buffer balance tracking deposits, cancellations, and replenishments.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
pub struct BufferRecord<M: ManagedTypeApi> {
    pub project_id: ManagedBuffer<M>,
    pub total_deposited: BigUint<M>,
    pub total_cancelled: BigUint<M>,
    pub total_replenished: BigUint<M>,
    pub last_replenishment_epoch: u64,
}

/// Non-permanence buffer pool contract.
///
/// Tracks per-project buffer deposits, cancellations, and replenishments.
/// Replenishments above a 10% threshold or on fully depleted projects
/// require governance approval and are rate-limited.
#[multiversx_sc::contract]
pub trait BufferPool: mrv_common::MrvGovernanceModule {
    #[init]
    fn init(&self, governance: ManagedAddress, carbon_credit_addr: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        require!(!carbon_credit_addr.is_zero(), "carbon_credit_addr must not be zero");
        self.governance().set(governance);
        self.carbon_credit_addr().set(carbon_credit_addr);
        self.total_pool_balance().set(BigUint::zero());
        self.storage_version().set(1u32);
    }

    /// Updates the authorized carbon-credit contract address.
    #[endpoint(setCarbonCreditAddr)]
    fn set_carbon_credit_addr(&self, addr: ManagedAddress) {
        self.require_governance_or_owner();
        require!(!addr.is_zero(), "carbon_credit_addr must not be zero");
        self.carbon_credit_addr().set(addr);
    }

    /// Records a buffer contribution for a project and monitoring period.
    ///
    /// Callable only by an authorized contract or governance actor.
    #[endpoint(depositBufferCredits)]
    fn deposit_buffer_credits(
        &self,
        project_id: ManagedBuffer,
        amount_scaled: BigUint,
        monitoring_period_n: u64,
    ) {
        self.require_authorized_caller();
        require!(!project_id.is_empty(), "empty project_id");
        require!(amount_scaled > 0u64, "amount must be positive");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");

        if !self.buffer_records().contains_key(&project_id) {
            self.buffer_records().insert(project_id.clone(), BufferRecord {
                project_id: project_id.clone(),
                total_deposited: BigUint::zero(),
                total_cancelled: BigUint::zero(),
                total_replenished: BigUint::zero(),
                last_replenishment_epoch: 0u64,
            });
        }

        self.buffer_records().entry(project_id.clone()).and_modify(|r| {
            r.total_deposited += &amount_scaled;
        });

        self.total_pool_balance().update(|b| *b += &amount_scaled);
        self.buffer_deposited_event(&project_id, &amount_scaled);
    }

    /// Cancels previously deposited buffer credits for a project.
    ///
    /// This updates accounting only and does not transfer funds.
    #[endpoint(cancelBufferCredits)]
    fn cancel_buffer_credits(
        &self,
        project_id: ManagedBuffer,
        reversal_amount_scaled: BigUint,
        reason_cid: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        require!(!project_id.is_empty(), "empty project_id");
        require!(reversal_amount_scaled > 0u64, "amount must be positive");
        require!(!reason_cid.is_empty(), "empty reason_cid");

        require!(
            self.buffer_records().contains_key(&project_id),
            "no buffer record for project"
        );

        let record = self.buffer_records().get(&project_id).unwrap();
        let already_cancelled = &record.total_cancelled;
        let max_cancellable = &record.total_deposited - already_cancelled;
        require!(
            reversal_amount_scaled <= max_cancellable,
            "CANCELLATION_EXCEEDS_AVAILABLE: cannot cancel more than deposited minus already cancelled"
        );

        self.buffer_records().entry(project_id.clone()).and_modify(|r| {
            r.total_cancelled += &reversal_amount_scaled;
        });

        let pool_balance = self.total_pool_balance().get();
        require!(pool_balance >= reversal_amount_scaled, "POOL_BALANCE_UNDERFLOW: accounting error");
        self.total_pool_balance().set(&pool_balance - &reversal_amount_scaled);

        self.buffer_cancelled_event(&project_id, &reason_cid, &reversal_amount_scaled);
    }

    /// Replenishes a project's buffer balance.
    ///
    /// Amounts above the configured threshold require governance, and
    /// replenishments are rate-limited per project.
    #[endpoint(replenishBufferCredits)]
    fn replenish_buffer_credits(
        &self,
        project_id: ManagedBuffer,
        amount_scaled: BigUint,
        justification_cid: ManagedBuffer,
    ) {
        self.require_authorized_caller();
        require!(!project_id.is_empty(), "empty project_id");
        require!(amount_scaled > 0u64, "amount must be positive");
        require!(!justification_cid.is_empty(), "empty justification_cid");

        require!(
            self.buffer_records().contains_key(&project_id),
            "no buffer record for project"
        );

        let record = self.buffer_records().get(&project_id).unwrap();

        let net_live = if record.total_deposited >= record.total_cancelled {
            &record.total_deposited - &record.total_cancelled
        } else {
            BigUint::zero()
        };
        if net_live == BigUint::zero() {
            let caller = self.blockchain().get_caller();
            require!(
                caller == self.governance().get(),
                "buffer fully depleted — governance approval required for any replenishment"
            );
        }
        let threshold = &net_live * REPLENISHMENT_GOVERNANCE_THRESHOLD_BPS / 10_000u64;
        if amount_scaled > threshold {
            let caller = self.blockchain().get_caller();
            require!(
                caller == self.governance().get(),
                "replenishment exceeds 10% threshold — governance approval required"
            );
            self.buffer_replenishment_governance_required_event(&project_id, &amount_scaled);
        }

        let current_epoch = self.blockchain().get_block_epoch();
        // Skip cooldown check for the first-ever replenishment (total_replenished == 0)
        if record.total_replenished > 0u64 {
            require!(
                current_epoch >= record.last_replenishment_epoch + REPLENISHMENT_COOLDOWN_EPOCHS,
                "replenishment rate limit: 1 per 90 days per project"
            );
        }

        self.buffer_records().entry(project_id.clone()).and_modify(|r| {
            r.total_replenished += &amount_scaled;
            r.last_replenishment_epoch = current_epoch;
        });

        self.total_pool_balance().update(|b| *b += &amount_scaled);
        self.buffer_replenished_event(&project_id, &amount_scaled);
    }

    #[view(getBufferRecord)]
    fn get_buffer_record(
        &self,
        project_id: ManagedBuffer,
    ) -> OptionalValue<BufferRecord<Self::Api>> {
        match self.buffer_records().get(&project_id) {
            Some(r) => OptionalValue::Some(r),
            None => OptionalValue::None,
        }
    }

    #[view(getTotalPoolBalance)]
    fn get_total_pool_balance(&self) -> BigUint {
        self.total_pool_balance().get()
    }

    fn require_authorized_caller(&self) {
        let caller = self.blockchain().get_caller();
        let is_governance = !self.governance().is_empty() && caller == self.governance().get();
        let is_owner = caller == self.blockchain().get_owner_address();
        let is_carbon_credit = !self.carbon_credit_addr().is_empty() && caller == self.carbon_credit_addr().get();
        let is_whitelisted = self.authorized_callers().contains(&caller);
        require!(is_governance || is_owner || is_carbon_credit || is_whitelisted, "caller not authorized");
    }

    #[storage_mapper("carbonCreditAddr")]
    fn carbon_credit_addr(&self) -> SingleValueMapper<ManagedAddress>;

    #[storage_mapper("bufferRecords")]
    fn buffer_records(&self) -> MapMapper<ManagedBuffer, BufferRecord<Self::Api>>;

    #[storage_mapper("totalPoolBalance")]
    fn total_pool_balance(&self) -> SingleValueMapper<BigUint>;

    #[event("bufferDeposited")]
    fn buffer_deposited_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        amount: &BigUint,
    );

    #[event("bufferCancelled")]
    fn buffer_cancelled_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        #[indexed] reason_cid: &ManagedBuffer,
        amount: &BigUint,
    );

    #[event("bufferReplenished")]
    fn buffer_replenished_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        amount: &BigUint,
    );

    #[event("bufferReplenishmentGovernanceRequired")]
    fn buffer_replenishment_governance_required_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        amount: &BigUint,
    );

    /// Adds an address to the authorized caller whitelist.
    #[endpoint(addAuthorizedCaller)]
    fn add_authorized_caller(&self, caller: ManagedAddress) {
        self.require_governance_or_owner();
        require!(!caller.is_zero(), "caller must not be zero");
        self.authorized_callers().insert(caller.clone());
        self.authorized_caller_added_event(&caller);
    }

    /// Removes an address from the authorized caller whitelist.
    #[endpoint(removeAuthorizedCaller)]
    fn remove_authorized_caller(&self, caller: ManagedAddress) {
        self.require_governance_or_owner();
        require!(
            self.authorized_callers().contains(&caller),
            "caller not in whitelist"
        );
        self.authorized_callers().swap_remove(&caller);
        self.authorized_caller_removed_event(&caller);
    }

    #[view(isAuthorizedCaller)]
    fn is_authorized_caller(&self, caller: ManagedAddress) -> bool {
        self.authorized_callers().contains(&caller)
    }

    #[storage_mapper("authorizedCallers")]
    fn authorized_callers(&self) -> UnorderedSetMapper<ManagedAddress>;

    #[event("authorizedCallerAdded")]
    fn authorized_caller_added_event(&self, #[indexed] caller: &ManagedAddress);

    #[event("authorizedCallerRemoved")]
    fn authorized_caller_removed_event(&self, #[indexed] caller: &ManagedAddress);

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    #[upgrade]
    fn upgrade(&self) {
        let current = self.storage_version().get();
        if current < 1u32 {
            self.storage_version().set(1u32);
        }
    }
}
