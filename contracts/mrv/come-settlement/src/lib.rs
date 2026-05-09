#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use mrv_common::resolve_storage_version_upgrade;

pub mod governance_proxy;

/// Maximum lifetime, in rounds, for a funded settlement before it can be expired.
const MAX_SETTLEMENT_LIFETIME_ROUNDS: u64 = 1_000_000;

const STATUS_PENDING: u8 = 0;
const STATUS_FUNDED: u8 = 1;
const STATUS_SETTLED: u8 = 2;
const STATUS_CANCELLED: u8 = 3;
const STATUS_EXPIRED: u8 = 4;

/// Per-settlement escrow record following a `pending -> funded -> settled` lifecycle.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct SettlementRecord<M: ManagedTypeApi> {
    pub settlement_id: ManagedBuffer<M>,
    pub from: ManagedAddress<M>,
    pub to: ManagedAddress<M>,
    pub token_id: TokenIdentifier<M>,
    pub amount_scaled: BigUint<M>,
    pub status: u8,
    pub reason_cid: ManagedBuffer<M>,
    pub cancel_reason_cid: ManagedBuffer<M>,
    pub created_at: u64,
    pub settled_at: u64,
    /// Block round after which the settlement can be expired and refunded.
    /// Zero for pre-migration records or settlements still in pending state.
    pub expiry_round: u64,
}

/// COME settlement contract with per-settlement escrow accounting.
///
/// Settlements move through `pending`, `funded`, `settled`, `cancelled`, and
/// `expired` states.
/// Funding records the escrowed token amount and sets an expiry round.
/// Execution transfers only the escrow recorded for the referenced settlement.
#[multiversx_sc::contract]
pub trait ComeSettlement: mrv_common::MrvGovernanceModule {
    #[init]
    fn init(&self, governance: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        self.governance().set(governance);
        self.storage_version().set(2u32);
    }

    #[endpoint(setGovernanceReadAddress)]
    fn set_governance_read_address(&self, addr: ManagedAddress) {
        self.require_governance_or_owner();
        require!(!addr.is_zero(), "governance_read_address must not be zero");
        self.governance_read_address().set(addr);
    }

    #[endpoint(clearGovernanceReadAddress)]
    fn clear_governance_read_address(&self) {
        self.require_governance_or_owner();
        self.governance_read_address().clear();
    }

    /// Creates a settlement instruction in `pending` state without moving funds.
    #[endpoint(createSettlement)]
    fn create_settlement(
        &self,
        settlement_id: ManagedBuffer,
        from: ManagedAddress,
        to: ManagedAddress,
        token_id: TokenIdentifier,
        amount_scaled: BigUint,
        reason_cid: ManagedBuffer,
    ) {
        self.require_not_paused();
        self.require_governance_or_owner();
        require!(!settlement_id.is_empty(), "empty settlement_id");
        require!(!from.is_zero(), "from must not be zero");
        require!(!to.is_zero(), "to must not be zero");
        require!(token_id.is_valid_esdt_identifier(), "invalid token_id");
        require!(amount_scaled > BigUint::zero(), "amount must be positive");
        require!(
            !self.settlements().contains_key(&settlement_id),
            "settlement already exists"
        );

        let record = SettlementRecord {
            settlement_id: settlement_id.clone(),
            from: from.clone(),
            to: to.clone(),
            token_id,
            amount_scaled,
            status: STATUS_PENDING,
            reason_cid,
            cancel_reason_cid: ManagedBuffer::new(),
            created_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
            settled_at: 0u64,
            expiry_round: 0u64,
        };

        self.settlements().insert(settlement_id.clone(), record);
        self.settlement_created_event(&settlement_id, &from, &to);
    }

    /// Funds a pending settlement with the exact token and amount required for execution.
    ///
    /// Only the payer recorded in the settlement may fund it.
    #[payable("*")]
    #[endpoint(fundSettlement)]
    fn fund_settlement(&self, settlement_id: ManagedBuffer) {
        self.require_not_paused();
        let settlement = self.settlements().get(&settlement_id);
        require!(settlement.is_some(), "settlement not found");
        let settlement = settlement.unwrap();
        require!(
            settlement.status == STATUS_PENDING,
            "settlement not in pending state"
        );

        let caller = self.blockchain().get_caller();
        require!(
            caller == settlement.from,
            "only the payer (settlement.from) can fund"
        );

        let payment = self.call_value().single_esdt();
        require!(
            payment.token_identifier == settlement.token_id,
            "wrong token"
        );
        require!(
            payment.token_nonce == 0,
            "FUNGIBLE_ONLY: token nonce must be 0"
        );
        require!(payment.amount == settlement.amount_scaled, "wrong amount");
        require!(
            self.settlement_escrow(&settlement_id).is_empty(),
            "settlement escrow already funded"
        );

        self.settlement_escrow(&settlement_id)
            .set(payment.amount.clone());

        let current_round = self.blockchain().get_block_round();
        self.settlements()
            .entry(settlement_id.clone())
            .and_modify(|r| {
                r.status = STATUS_FUNDED;
                r.expiry_round = current_round + MAX_SETTLEMENT_LIFETIME_ROUNDS;
            });

        self.settlement_funded_event(&settlement_id, &caller);
    }

    /// Executes a funded settlement.
    ///
    /// This transfers the escrowed ESDT payment to the recorded recipient.
    #[endpoint(executeSettlement)]
    fn execute_settlement(&self, settlement_id: ManagedBuffer) {
        self.require_not_paused();
        self.require_governance_or_owner();
        let settlement = self.settlements().get(&settlement_id);
        require!(settlement.is_some(), "settlement not found");
        let settlement = settlement.unwrap();
        require!(
            settlement.status == STATUS_FUNDED,
            "settlement not funded — call fundSettlement first"
        );
        require!(
            settlement.expiry_round == 0
                || self.blockchain().get_block_round() <= settlement.expiry_round,
            "SETTLEMENT_EXPIRED: use expireSettlement to reclaim funds"
        );

        let escrowed = self.settlement_escrow(&settlement_id).get();
        require!(
            escrowed >= settlement.amount_scaled,
            "ESCROW_NOT_RECORDED: settlement was not properly funded via fundSettlement"
        );

        self.settlement_escrow(&settlement_id).clear();

        self.send().direct_esdt(
            &settlement.to,
            &settlement.token_id,
            0u64,
            &settlement.amount_scaled,
        );

        let settled_ts = self
            .blockchain()
            .get_block_timestamp_seconds()
            .as_u64_seconds();
        self.settlements()
            .entry(settlement_id.clone())
            .and_modify(|r| {
                r.status = STATUS_SETTLED;
                r.settled_at = settled_ts;
            });

        self.settlement_executed_event(&settlement_id);
    }

    /// Cancels a pending or funded settlement.
    ///
    /// When the settlement is already funded, the escrowed tokens are returned
    /// to the payer.
    #[endpoint(cancelSettlement)]
    fn cancel_settlement(&self, settlement_id: ManagedBuffer, cancel_reason_cid: ManagedBuffer) {
        self.require_not_paused();
        self.require_governance_or_owner();
        let settlement = self.settlements().get(&settlement_id);
        require!(settlement.is_some(), "settlement not found");
        let settlement = settlement.unwrap();
        require!(
            settlement.status == STATUS_PENDING || settlement.status == STATUS_FUNDED,
            "settlement not in pending or funded state"
        );

        if settlement.status == STATUS_FUNDED {
            let escrowed_amount = self.settlement_escrow(&settlement_id).get();
            self.send().direct_esdt(
                &settlement.from,
                &settlement.token_id,
                0u64,
                &escrowed_amount,
            );
            self.settlement_escrow(&settlement_id).clear();
        }

        self.settlements()
            .entry(settlement_id.clone())
            .and_modify(|r| {
                r.status = STATUS_CANCELLED;
                r.cancel_reason_cid = cancel_reason_cid.clone();
            });

        self.settlement_cancelled_event(&settlement_id);
    }

    /// Re-encodes settlement records to include the `expiry_round` field
    /// added post-migration. Re-inserting already migrated records is a no-op.
    #[only_owner]
    #[endpoint(migrateSettlements)]
    fn migrate_settlements(&self, settlement_ids: MultiValueEncoded<ManagedBuffer>) {
        self.require_not_paused();
        for sid in settlement_ids.into_iter() {
            if let Some(record) = self.settlements().get(&sid) {
                self.settlements().insert(sid, record);
            }
        }
    }

    /// Expires a funded settlement after its expiry round and refunds the
    /// escrowed tokens to the payer.
    /// Any caller may trigger the expiry once the round check passes.
    #[endpoint(expireSettlement)]
    fn expire_settlement(&self, settlement_id: ManagedBuffer) {
        self.require_not_paused();
        let settlement = self.settlements().get(&settlement_id);
        require!(settlement.is_some(), "settlement not found");
        let settlement = settlement.unwrap();
        require!(
            settlement.status == STATUS_FUNDED,
            "only funded settlements can expire"
        );
        require!(
            settlement.expiry_round > 0
                && self.blockchain().get_block_round() > settlement.expiry_round,
            "settlement has not expired yet"
        );

        let escrowed_amount = self.settlement_escrow(&settlement_id).get();
        self.send().direct_esdt(
            &settlement.from,
            &settlement.token_id,
            0u64,
            &escrowed_amount,
        );
        self.settlement_escrow(&settlement_id).clear();

        self.settlements()
            .entry(settlement_id.clone())
            .and_modify(|r| {
                r.status = STATUS_EXPIRED;
            });

        self.settlement_expired_event(&settlement_id);
    }

    #[view(getSettlement)]
    fn get_settlement(
        &self,
        settlement_id: ManagedBuffer,
    ) -> OptionalValue<SettlementRecord<Self::Api>> {
        match self.settlements().get(&settlement_id) {
            Some(r) => OptionalValue::Some(r),
            None => OptionalValue::None,
        }
    }

    #[storage_mapper("settlements")]
    fn settlements(&self) -> MapMapper<ManagedBuffer, SettlementRecord<Self::Api>>;

    /// Tracks escrowed funds by settlement identifier.
    #[storage_mapper("settlementEscrow")]
    fn settlement_escrow(&self, settlement_id: &ManagedBuffer) -> SingleValueMapper<BigUint>;

    #[view(getGovernanceReadAddress)]
    #[storage_mapper("governanceReadAddress")]
    fn governance_read_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[event("settlementFunded")]
    fn settlement_funded_event(
        &self,
        #[indexed] settlement_id: &ManagedBuffer,
        #[indexed] funder: &ManagedAddress,
    );

    #[event("settlementCreated")]
    fn settlement_created_event(
        &self,
        #[indexed] settlement_id: &ManagedBuffer,
        #[indexed] from: &ManagedAddress,
        #[indexed] to: &ManagedAddress,
    );

    #[event("settlementExecuted")]
    fn settlement_executed_event(&self, #[indexed] settlement_id: &ManagedBuffer);

    #[event("settlementCancelled")]
    fn settlement_cancelled_event(&self, #[indexed] settlement_id: &ManagedBuffer);

    /// Emitted when a funded settlement expires and its escrow is refunded.
    #[event("settlementExpired")]
    fn settlement_expired_event(&self, #[indexed] settlement_id: &ManagedBuffer);

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
        let target = resolve_storage_version_upgrade(stored, 2u32, 2u32)
            .unwrap_or_else(|message| sc_panic!(message));
        if stored != target {
            self.storage_version().set(target);
        }
    }
}
