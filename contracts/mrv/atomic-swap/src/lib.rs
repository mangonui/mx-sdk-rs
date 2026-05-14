#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use mrv_common::resolve_storage_version_upgrade;

pub mod governance_proxy;

/// Escrowed RFQ lifecycle record tracking margin deposit, settlement, and expiry.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct EscrowedRfqRecord<M: ManagedTypeApi> {
    pub rfq_id: ManagedBuffer<M>,
    pub buyer: ManagedAddress<M>,
    pub dealer: ManagedAddress<M>,
    pub token_id: TokenIdentifier<M>,
    pub quantity: BigUint<M>,
    pub margin_amount: BigUint<M>,
    pub price_come_per_unit: BigUint<M>,
    pub funded_epoch: u64,
    pub expiry_epoch: u64,
    pub status: u8,
}

/// RFQ status code for records awaiting the buyer's margin deposit.
const RFQ_PENDING_DEPOSIT: u8 = 0;
/// RFQ status code for records with margin deposited in escrow.
const RFQ_DEPOSITED: u8 = 1;
/// RFQ status code for settled records (code `2` reserved for future use).
const RFQ_COMPLETED: u8 = 3;
/// RFQ status code for records reclaimed after expiry.
const RFQ_EXPIRED: u8 = 4;
/// Status used when a deposited RFQ is cancelled before expiry.
const RFQ_CANCELLED: u8 = 5;

/// Escrow-based RFQ settlement contract for COME-funded atomic swaps.
///
/// Buyer deposits COME margin, dealer delivers RWA tokens, and margin is
/// released on settlement or returned to buyer on expiry or cancellation.
#[multiversx_sc::contract]
pub trait AtomicSwap {
    #[init]
    fn init(&self, come_token_id: TokenIdentifier) {
        require!(
            come_token_id.is_valid_esdt_identifier(),
            "invalid COME token ID"
        );
        self.come_token_id().set(come_token_id);
        self.storage_version().set(1u32);
    }

    #[only_owner]
    #[endpoint(setGovernanceReadAddress)]
    fn set_governance_read_address(&self, addr: ManagedAddress) {
        require!(!addr.is_zero(), "governance_read_address must not be zero");
        self.governance_read_address().set(addr);
    }

    #[only_owner]
    #[endpoint(clearGovernanceReadAddress)]
    fn clear_governance_read_address(&self) {
        self.governance_read_address().clear();
    }

    #[only_owner]
    #[endpoint(allowAssetToken)]
    fn allow_asset_token(&self, token_id: TokenIdentifier) {
        require!(token_id.is_valid_esdt_identifier(), "invalid token_id");
        require!(
            token_id != self.come_token_id().get(),
            "asset token must differ from COME"
        );

        let mut allowed_tokens = self.allowed_asset_tokens();
        allowed_tokens.insert(token_id.clone());
        self.asset_token_allowed_event(&token_id);
    }

    #[only_owner]
    #[endpoint(removeAssetToken)]
    fn remove_asset_token(&self, token_id: TokenIdentifier) {
        require!(
            self.allowed_asset_tokens().contains(&token_id),
            "asset token not allowed"
        );

        self.allowed_asset_tokens().swap_remove(&token_id);
        self.asset_token_removed_event(&token_id);
    }

    /// Creates an RFQ record in `pending_deposit` state.
    #[endpoint(createRfq)]
    fn create_rfq(
        &self,
        rfq_id: ManagedBuffer,
        buyer: ManagedAddress,
        dealer: ManagedAddress,
        token_id: TokenIdentifier,
        quantity: BigUint,
        margin_amount: BigUint,
        price_come_per_unit: BigUint,
        expiry_epoch: u64,
    ) {
        self.require_not_paused();
        let caller = self.blockchain().get_caller();
        require!(
            caller == self.blockchain().get_owner_address(),
            "only owner can create RFQs"
        );
        require!(!rfq_id.is_empty(), "empty rfq_id");
        require!(!buyer.is_zero(), "buyer must not be zero");
        require!(!dealer.is_zero(), "dealer must not be zero");
        require!(buyer != dealer, "buyer and dealer must be distinct");
        require!(token_id.is_valid_esdt_identifier(), "invalid token_id");
        require!(
            self.allowed_asset_tokens().contains(&token_id),
            "asset token not allowed"
        );
        require!(quantity > 0u64, "quantity must be positive");
        require!(margin_amount > 0u64, "margin must be positive");
        require!(price_come_per_unit > 0u64, "price must be positive");
        require!(
            expiry_epoch > self.blockchain().get_block_epoch(),
            "expiry must be in the future"
        );
        require!(!self.rfqs().contains_key(&rfq_id), "RFQ already exists");

        let record = EscrowedRfqRecord {
            rfq_id: rfq_id.clone(),
            buyer,
            dealer,
            token_id,
            quantity,
            margin_amount,
            price_come_per_unit,
            funded_epoch: 0u64,
            expiry_epoch,
            status: RFQ_PENDING_DEPOSIT,
        };

        self.rfqs().insert(rfq_id.clone(), record);
        self.rfq_created_event(&rfq_id);
    }

    /// Deposits the buyer's COME margin into escrow.
    #[payable("*")]
    #[endpoint(depositMargin)]
    fn deposit_margin(&self, rfq_id: ManagedBuffer) {
        self.require_not_paused();
        let rfq = self.rfqs().get(&rfq_id);
        require!(rfq.is_some(), "RFQ not found");
        let rfq = rfq.unwrap();
        require!(rfq.status == RFQ_PENDING_DEPOSIT, "NOT_AWAITING_DEPOSIT");

        let caller = self.blockchain().get_caller();
        require!(caller == rfq.buyer, "only buyer can deposit margin");

        let payment = self.call_value().single_esdt();
        require!(
            payment.token_identifier == self.come_token_id().get(),
            "must deposit COME token"
        );
        require!(
            payment.token_nonce == 0,
            "FUNGIBLE_ONLY: token nonce must be 0"
        );
        require!(payment.amount == rfq.margin_amount, "WRONG_MARGIN_AMOUNT");

        self.rfqs().entry(rfq_id.clone()).and_modify(|r| {
            r.status = RFQ_DEPOSITED;
            r.funded_epoch = self.blockchain().get_block_epoch();
        });

        self.locked_balances(&rfq.buyer)
            .update(|b| *b += &payment.amount);
        self.require_locked_balance_backed(&rfq.buyer);
        self.margin_deposited_event(&rfq_id, &payment.amount);
    }

    /// Settles the RFQ: dealer delivers RWA tokens, escrowed margin releases to dealer.
    #[payable("*")]
    #[endpoint(settle)]
    fn settle(&self, rfq_id: ManagedBuffer) {
        self.require_not_paused();
        let rfq = self.rfqs().get(&rfq_id);
        require!(rfq.is_some(), "RFQ not found");
        let rfq = rfq.unwrap();
        require!(rfq.status == RFQ_DEPOSITED, "NOT_DEPOSITED");
        require!(
            self.blockchain().get_block_epoch() <= rfq.expiry_epoch,
            "EXPIRED"
        );

        let caller = self.blockchain().get_caller();
        require!(caller == rfq.dealer, "ONLY_DEALER");

        let payment = self.call_value().single_esdt();
        require!(payment.token_identifier == rfq.token_id, "wrong RWA token");
        require!(payment.amount == rfq.quantity, "wrong RWA quantity");

        // F-018: checks-effects-interactions — validate before transfer
        self.require_locked_balance_backed(&rfq.buyer);
        let buyer_locked = self.locked_balances(&rfq.buyer).get();
        if buyer_locked < rfq.margin_amount {
            self.settlement_failed_event(
                &rfq_id,
                &rfq.buyer,
                &rfq.dealer,
                &ManagedBuffer::from(b"LOCKED_BALANCE_UNDERFLOW"),
            );
            sc_panic!("LOCKED_BALANCE_UNDERFLOW: accounting discrepancy");
        }

        // Effects: update state before external calls
        self.locked_balances(&rfq.buyer)
            .set(&buyer_locked - &rfq.margin_amount);

        self.rfqs().entry(rfq_id.clone()).and_modify(|r| {
            r.status = RFQ_COMPLETED;
        });

        // Interactions: external transfers last
        self.send()
            .direct_esdt(&rfq.buyer, &rfq.token_id, 0u64, &rfq.quantity);

        self.send().direct_esdt(
            &rfq.dealer,
            &self.come_token_id().get(),
            0u64,
            &rfq.margin_amount,
        );

        self.rfq_settled_event(&rfq_id, &rfq.buyer, &rfq.dealer);
    }

    /// Returns escrowed margin to the buyer after expiry. Callable by anyone.
    #[endpoint(autoReclaim)]
    fn auto_reclaim(&self, rfq_id: ManagedBuffer) {
        self.require_not_paused();
        let rfq = self.rfqs().get(&rfq_id);
        require!(rfq.is_some(), "RFQ not found");
        let rfq = rfq.unwrap();
        require!(rfq.status == RFQ_DEPOSITED, "NOT_DEPOSITED");
        require!(
            self.blockchain().get_block_epoch() > rfq.expiry_epoch,
            "NOT_EXPIRED"
        );

        self.require_locked_balance_backed(&rfq.buyer);
        let buyer_locked = self.locked_balances(&rfq.buyer).get();
        require!(
            buyer_locked >= rfq.margin_amount,
            "LOCKED_BALANCE_UNDERFLOW: accounting discrepancy"
        );
        self.locked_balances(&rfq.buyer)
            .set(&buyer_locked - &rfq.margin_amount);

        self.rfqs().entry(rfq_id.clone()).and_modify(|r| {
            r.status = RFQ_EXPIRED;
        });

        self.send().direct_esdt(
            &rfq.buyer,
            &self.come_token_id().get(),
            0u64,
            &rfq.margin_amount,
        );

        self.margin_returned_event(&rfq_id, &rfq.margin_amount, &rfq.buyer);
    }

    /// Cancels a deposited RFQ and returns margin to the buyer.
    /// Only the buyer may cancel because cancellation refunds the buyer's escrow.
    #[endpoint(cancelRfq)]
    fn cancel_rfq(&self, rfq_id: ManagedBuffer) {
        self.require_not_paused();
        let rfq = self.rfqs().get(&rfq_id);
        require!(rfq.is_some(), "RFQ not found");
        let rfq = rfq.unwrap();
        require!(rfq.status == RFQ_DEPOSITED, "NOT_DEPOSITED");

        let caller = self.blockchain().get_caller();
        require!(caller == rfq.buyer, "only buyer can cancel");

        self.require_locked_balance_backed(&rfq.buyer);
        let buyer_locked = self.locked_balances(&rfq.buyer).get();
        require!(
            buyer_locked >= rfq.margin_amount,
            "LOCKED_BALANCE_UNDERFLOW: accounting discrepancy"
        );
        self.locked_balances(&rfq.buyer)
            .set(&buyer_locked - &rfq.margin_amount);

        self.rfqs().entry(rfq_id.clone()).and_modify(|r| {
            r.status = RFQ_CANCELLED;
        });

        self.send().direct_esdt(
            &rfq.buyer,
            &self.come_token_id().get(),
            0u64,
            &rfq.margin_amount,
        );

        self.margin_returned_event(&rfq_id, &rfq.margin_amount, &rfq.buyer);
    }

    #[view(getRfq)]
    fn get_rfq(&self, rfq_id: ManagedBuffer) -> OptionalValue<EscrowedRfqRecord<Self::Api>> {
        match self.rfqs().get(&rfq_id) {
            Some(r) => OptionalValue::Some(r),
            None => OptionalValue::None,
        }
    }

    #[view(getLockedBalance)]
    fn get_locked_balance(&self, holder: ManagedAddress) -> BigUint {
        self.locked_balances(&holder).get()
    }

    #[view(isAssetTokenAllowed)]
    fn is_asset_token_allowed(&self, token_id: TokenIdentifier) -> bool {
        self.allowed_asset_tokens().contains(&token_id)
    }

    fn require_locked_balance_backed(&self, holder: &ManagedAddress) {
        let locked = self.locked_balances(holder).get();
        if locked == 0u64 {
            return;
        }

        let escrow_balance = self.blockchain().get_sc_balance(
            EgldOrEsdtTokenIdentifier::esdt(self.come_token_id().get()),
            0u64,
        );
        require!(
            locked <= escrow_balance,
            "LOCKED_BALANCE_NOT_BACKED: escrow balance mismatch"
        );
    }

    #[storage_mapper("comeTokenId")]
    fn come_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    #[storage_mapper("rfqs")]
    fn rfqs(&self) -> MapMapper<ManagedBuffer, EscrowedRfqRecord<Self::Api>>;

    #[storage_mapper("lockedBalances")]
    fn locked_balances(&self, holder: &ManagedAddress) -> SingleValueMapper<BigUint>;

    #[storage_mapper("governanceReadAddress")]
    fn governance_read_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[storage_mapper("allowedAssetTokens")]
    fn allowed_asset_tokens(&self) -> UnorderedSetMapper<TokenIdentifier>;

    #[event("assetTokenAllowed")]
    fn asset_token_allowed_event(&self, #[indexed] token_id: &TokenIdentifier);

    #[event("assetTokenRemoved")]
    fn asset_token_removed_event(&self, #[indexed] token_id: &TokenIdentifier);

    #[event("rfqCreated")]
    fn rfq_created_event(&self, #[indexed] rfq_id: &ManagedBuffer);

    #[event("marginDeposited")]
    fn margin_deposited_event(&self, #[indexed] rfq_id: &ManagedBuffer, amount: &BigUint);

    #[event("rfqSettled")]
    fn rfq_settled_event(
        &self,
        #[indexed] rfq_id: &ManagedBuffer,
        #[indexed] buyer: &ManagedAddress,
        #[indexed] dealer: &ManagedAddress,
    );

    /// Emitted when a settlement fails due to escrow or transfer issues.
    /// Off-chain consumers should monitor this event for operational alerting.
    #[event("settlementFailed")]
    fn settlement_failed_event(
        &self,
        #[indexed] rfq_id: &ManagedBuffer,
        #[indexed] buyer: &ManagedAddress,
        #[indexed] dealer: &ManagedAddress,
        reason: &ManagedBuffer,
    );

    #[event("marginReturned")]
    fn margin_returned_event(
        &self,
        #[indexed] rfq_id: &ManagedBuffer,
        #[indexed] amount: &BigUint,
        #[indexed] recipient: &ManagedAddress,
    );

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    fn require_not_paused(&self) {
        if self.governance_read_address().is_empty() {
            let authority = self.blockchain().get_owner_address();
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
}
