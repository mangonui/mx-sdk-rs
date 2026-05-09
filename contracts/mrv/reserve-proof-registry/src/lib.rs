#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use mrv_common::resolve_storage_version_upgrade;

pub mod buffer_pool_proxy;
pub mod carbon_credit_proxy;
pub mod governance_proxy;
pub mod reserve_proof_registry_proxy;

const VM0042_MERKLE_LEAF_PREFIX: &[u8] = b"\x00";
const VM0042_MERKLE_NODE_PREFIX: &[u8] = b"\x01";

/// Reserve proof for the VM0042 track.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct ReserveProof<M: ManagedTypeApi> {
    pub token_id: ManagedBuffer<M>,
    pub total_supply_scaled: BigUint<M>,
    pub total_buffer_scaled: BigUint<M>,
    pub total_retired_scaled: BigUint<M>,
    pub net_circulating_scaled: BigUint<M>,
    pub merkle_root: ManagedBuffer<M>,
    pub snapshot_block: u64,
    pub anchored_at: u64,
}

/// Reserve proof for the GSOC track.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct GsocReserveProof<M: ManagedTypeApi> {
    pub project_id: ManagedBuffer<M>,
    pub total_issued: u64,
    pub total_retired: u64,
    pub net_active: u64,
    pub serial_count: u64,
    pub itmo_serial_hash: ManagedBuffer<M>,
    pub snapshot_block: u64,
    pub anchored_at: u64,
}

/// On-chain registry for VM0042 and GSOC reserve proof snapshots.
///
/// Off-chain jobs compute the reserve state and anchor the resulting
/// snapshots here. Snapshot blocks must be strictly monotonic per token
/// or project.
#[multiversx_sc::contract]
pub trait ReserveProofRegistry: mrv_common::MrvGovernanceModule {
    #[init]
    fn init(&self, governance: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        self.governance().set(governance);
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

    /// Configures the canonical carbon-credit lifecycle contract used for
    /// VM0042 supply reconciliation.
    #[endpoint(setCarbonCreditAddr)]
    fn set_carbon_credit_addr(&self, addr: ManagedAddress) {
        self.require_not_paused();
        self.require_governance_or_owner();
        require!(!addr.is_zero(), "carbon_credit_addr must not be zero");
        self.carbon_credit_addr().set(&addr);
        self.carbon_credit_addr_updated_event(&addr);
    }

    /// Configures the canonical buffer-pool reserve contract used for
    /// VM0042 reserve reconciliation.
    #[endpoint(setBufferPoolAddr)]
    fn set_buffer_pool_addr(&self, addr: ManagedAddress) {
        self.require_not_paused();
        self.require_governance_or_owner();
        require!(!addr.is_zero(), "buffer_pool_addr must not be zero");
        self.buffer_pool_addr().set(&addr);
        self.buffer_pool_addr_updated_event(&addr);
    }

    /// Anchors a VM0042 reserve proof for a token at a given snapshot block.
    #[endpoint(anchorReserveProof)]
    fn anchor_reserve_proof(
        &self,
        token_id: ManagedBuffer,
        total_supply_scaled: BigUint,
        total_buffer_scaled: BigUint,
        total_retired_scaled: BigUint,
        merkle_root: ManagedBuffer,
        snapshot_block: u64,
    ) {
        self.require_not_paused();
        self.require_governance_or_owner();
        require!(!token_id.is_empty(), "empty token_id");
        require!(!merkle_root.is_empty(), "empty merkle_root");
        require!(merkle_root.len() == 32, "merkle_root must be 32 bytes");
        require!(snapshot_block > 0, "invalid snapshot_block");

        let (canonical_token_id, canonical_supply, canonical_buffer, canonical_retired) =
            self.get_canonical_vm0042_totals();

        require!(
            token_id == canonical_token_id,
            "TOKEN_ID_MISMATCH: token_id must match configured carbon-credit dVCU token"
        );
        let current_latest = self.latest_reserve_proof_block(&token_id).get();
        require!(
            snapshot_block > current_latest,
            "SNAPSHOT_BLOCK_NOT_MONOTONIC: new block must be greater than current latest"
        );

        require!(
            total_supply_scaled >= &total_buffer_scaled + &total_retired_scaled,
            "INVALID_RESERVE_ARITHMETIC: supply < buffer + retired"
        );
        require!(
            total_supply_scaled == canonical_supply,
            "CANONICAL_SUPPLY_MISMATCH: supplied total_supply_scaled does not match lifecycle counters"
        );
        require!(
            total_buffer_scaled == canonical_buffer,
            "CANONICAL_BUFFER_MISMATCH: supplied total_buffer_scaled does not match reserve counters"
        );
        require!(
            total_retired_scaled == canonical_retired,
            "CANONICAL_RETIRED_MISMATCH: supplied total_retired_scaled does not match lifecycle counters"
        );

        let net_circulating = &total_supply_scaled - &total_buffer_scaled - &total_retired_scaled;

        let proof = ReserveProof {
            token_id: token_id.clone(),
            total_supply_scaled,
            total_buffer_scaled,
            total_retired_scaled,
            net_circulating_scaled: net_circulating,
            merkle_root: merkle_root.clone(),
            snapshot_block,
            anchored_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
        };

        let key = (token_id.clone(), mrv_common::period_key(snapshot_block));
        self.reserve_proofs().insert(key, proof);
        self.latest_reserve_proof_block(&token_id)
            .set(snapshot_block);

        self.reserve_proof_anchored_event(&token_id, &merkle_root, snapshot_block);
    }

    /// Anchors a GSOC reserve proof for a project at a given snapshot block.
    #[endpoint(anchorGsocReserveProof)]
    fn anchor_gsoc_reserve_proof(
        &self,
        project_id: ManagedBuffer,
        total_issued: u64,
        total_retired: u64,
        serial_count: u64,
        itmo_serial_hash: ManagedBuffer,
        snapshot_block: u64,
    ) {
        self.require_not_paused();
        self.require_governance_or_owner();
        require!(!project_id.is_empty(), "empty project_id");
        require!(!itmo_serial_hash.is_empty(), "empty itmo_serial_hash");
        require!(
            itmo_serial_hash.len() == 32,
            "itmo_serial_hash must be 32 bytes"
        );
        require!(snapshot_block > 0, "invalid snapshot_block");

        let current_latest = self.latest_gsoc_proof_block(&project_id).get();
        require!(
            snapshot_block > current_latest,
            "SNAPSHOT_BLOCK_NOT_MONOTONIC: new block must be greater than current latest"
        );

        let (canonical_total_issued, canonical_total_retired, canonical_serial_count) =
            self.get_canonical_gsoc_totals(&project_id);
        let canonical_itmo_serial_hash = self.get_canonical_gsoc_serial_inventory_hash(&project_id);

        require!(
            total_issued >= total_retired,
            "INVALID_RESERVE_ARITHMETIC: issued < retired"
        );
        require!(
            BigUint::from(total_issued) == canonical_total_issued,
            "CANONICAL_GSOC_ISSUED_MISMATCH: supplied total_issued does not match lifecycle counters"
        );
        require!(
            BigUint::from(total_retired) == canonical_total_retired,
            "CANONICAL_GSOC_RETIRED_MISMATCH: supplied total_retired does not match lifecycle counters"
        );
        require!(
            serial_count == canonical_serial_count,
            "CANONICAL_GSOC_SERIAL_COUNT_MISMATCH: supplied serial_count does not match lifecycle counters"
        );
        require!(
            itmo_serial_hash == canonical_itmo_serial_hash,
            "CANONICAL_GSOC_SERIAL_HASH_MISMATCH: supplied hash does not match canonical serial inventory"
        );

        let net_active = total_issued.saturating_sub(total_retired);

        let proof = GsocReserveProof {
            project_id: project_id.clone(),
            total_issued,
            total_retired,
            net_active,
            serial_count,
            itmo_serial_hash: itmo_serial_hash.clone(),
            snapshot_block,
            anchored_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
        };

        let key = (project_id.clone(), mrv_common::period_key(snapshot_block));
        self.gsoc_reserve_proofs().insert(key, proof);
        self.latest_gsoc_proof_block(&project_id)
            .set(snapshot_block);

        self.gsoc_reserve_proof_anchored_event(&project_id, &itmo_serial_hash, snapshot_block);
    }

    #[view(getReserveProof)]
    fn get_reserve_proof(
        &self,
        token_id: ManagedBuffer,
        snapshot_block: u64,
    ) -> OptionalValue<ReserveProof<Self::Api>> {
        let key = (token_id, mrv_common::period_key(snapshot_block));
        match self.reserve_proofs().get(&key) {
            Some(p) => OptionalValue::Some(p),
            None => OptionalValue::None,
        }
    }

    #[view(getLatestReserveProof)]
    fn get_latest_reserve_proof(
        &self,
        token_id: ManagedBuffer,
    ) -> OptionalValue<ReserveProof<Self::Api>> {
        if self.latest_reserve_proof_block(&token_id).is_empty() {
            return OptionalValue::None;
        }
        let block = self.latest_reserve_proof_block(&token_id).get();
        self.get_reserve_proof(token_id, block)
    }

    /// Verifies that a holder balance belongs to the anchored VM0042 reserve
    /// snapshot Merkle root for the given token and snapshot block.
    ///
    /// The proof matches the current off-chain cadence contract exactly:
    /// leaves are `sha256(0x00 || "{index}:{holder_address}:{balance_scaled}")`,
    /// and parent nodes hash `0x01` followed by the lexicographically sorted
    /// lowercase hex child hashes concatenated as ASCII.
    #[view(verifyHolderSnapshot)]
    fn verify_holder_snapshot(
        &self,
        token_id: ManagedBuffer,
        snapshot_block: u64,
        leaf_index: u64,
        holder_address: ManagedBuffer,
        balance_scaled: ManagedBuffer,
        merkle_proof: ManagedVec<ManagedBuffer>,
    ) -> bool {
        if token_id.is_empty()
            || snapshot_block == 0
            || holder_address.is_empty()
            || balance_scaled.is_empty()
            || merkle_proof.len() > 64
            || !self.is_ascii_decimal(&balance_scaled)
        {
            return false;
        }

        let proof = match self.get_reserve_proof(token_id, snapshot_block) {
            OptionalValue::Some(proof) => proof,
            OptionalValue::None => return false,
        };

        let leaf_hash = self.compute_vm0042_leaf_hash(leaf_index, &holder_address, &balance_scaled);
        let mut current_hash = leaf_hash.as_managed_buffer().clone();

        for index in 0..merkle_proof.len() {
            let sibling = merkle_proof.get(index);
            if sibling.len() != 32 {
                return false;
            }

            let current_hex = self.bytes_to_lower_hex(&current_hash);
            let sibling_hex = self.bytes_to_lower_hex(&sibling);

            let mut combined = ManagedBuffer::new();
            combined.append_bytes(VM0042_MERKLE_NODE_PREFIX);
            if self.managed_buffer_lex_le(&current_hash, &sibling) {
                combined.append(&current_hex);
                combined.append(&sibling_hex);
            } else {
                combined.append(&sibling_hex);
                combined.append(&current_hex);
            }

            current_hash = self.crypto().sha256(&combined).as_managed_buffer().clone();
        }

        current_hash == proof.merkle_root
    }

    #[view(getGsocReserveProof)]
    fn get_gsoc_reserve_proof(
        &self,
        project_id: ManagedBuffer,
        snapshot_block: u64,
    ) -> OptionalValue<GsocReserveProof<Self::Api>> {
        let key = (project_id, mrv_common::period_key(snapshot_block));
        match self.gsoc_reserve_proofs().get(&key) {
            Some(p) => OptionalValue::Some(p),
            None => OptionalValue::None,
        }
    }

    #[view(getLatestGsocReserveProof)]
    fn get_latest_gsoc_reserve_proof(
        &self,
        project_id: ManagedBuffer,
    ) -> OptionalValue<GsocReserveProof<Self::Api>> {
        if self.latest_gsoc_proof_block(&project_id).is_empty() {
            return OptionalValue::None;
        }
        let block = self.latest_gsoc_proof_block(&project_id).get();
        self.get_gsoc_reserve_proof(project_id, block)
    }

    #[view(getCanonicalGsocSerialInventoryHash)]
    fn get_canonical_gsoc_serial_inventory_hash_view(
        &self,
        project_id: ManagedBuffer,
    ) -> ManagedBuffer {
        self.get_canonical_gsoc_serial_inventory_hash(&project_id)
    }

    #[view(verifyGsocSerialInventoryHash)]
    fn verify_gsoc_serial_inventory_hash(
        &self,
        project_id: ManagedBuffer,
        expected_hash: ManagedBuffer,
    ) -> bool {
        self.get_canonical_gsoc_serial_inventory_hash(&project_id) == expected_hash
    }

    #[storage_mapper("reserveProofs")]
    fn reserve_proofs(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), ReserveProof<Self::Api>>;

    #[storage_mapper("gsocReserveProofs")]
    fn gsoc_reserve_proofs(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer), GsocReserveProof<Self::Api>>;

    #[storage_mapper("latestReserveProofBlock")]
    fn latest_reserve_proof_block(&self, token_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[storage_mapper("latestGsocProofBlock")]
    fn latest_gsoc_proof_block(&self, project_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[view(getCarbonCreditAddr)]
    #[storage_mapper("carbonCreditAddr")]
    fn carbon_credit_addr(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getBufferPoolAddr)]
    #[storage_mapper("bufferPoolAddr")]
    fn buffer_pool_addr(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getGovernanceReadAddress)]
    #[storage_mapper("governanceReadAddress")]
    fn governance_read_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[event("reserveProofAnchored")]
    fn reserve_proof_anchored_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] merkle_root: &ManagedBuffer,
        snapshot_block: u64,
    );

    #[event("gsocReserveProofAnchored")]
    fn gsoc_reserve_proof_anchored_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        #[indexed] itmo_serial_hash: &ManagedBuffer,
        snapshot_block: u64,
    );

    #[event("governanceReadAddressUpdated")]
    fn governance_read_address_updated_event(
        &self,
        #[indexed] governance_read_address: &ManagedAddress,
    );

    #[event("governanceReadAddressCleared")]
    fn governance_read_address_cleared_event(&self);

    #[event("carbonCreditAddrUpdated")]
    fn carbon_credit_addr_updated_event(&self, #[indexed] carbon_credit_addr: &ManagedAddress);

    #[event("bufferPoolAddrUpdated")]
    fn buffer_pool_addr_updated_event(&self, #[indexed] buffer_pool_addr: &ManagedAddress);

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    #[upgrade]
    fn upgrade(&self) {
        let stored = self.storage_version().get();
        let target = resolve_storage_version_upgrade(stored, 1u32, 1u32)
            .unwrap_or_else(|message| sc_panic!(message));
        if stored != target {
            self.storage_version().set(target);
        }
    }

    fn compute_vm0042_leaf_hash(
        &self,
        leaf_index: u64,
        holder_address: &ManagedBuffer,
        balance_scaled: &ManagedBuffer,
    ) -> ManagedByteArray<Self::Api, 32> {
        let mut leaf_preimage = ManagedBuffer::new();
        leaf_preimage.append_bytes(VM0042_MERKLE_LEAF_PREFIX);
        self.append_u64_ascii_decimal(&mut leaf_preimage, leaf_index);
        leaf_preimage.append_bytes(b":");
        leaf_preimage.append(holder_address);
        leaf_preimage.append_bytes(b":");
        leaf_preimage.append(balance_scaled);
        self.crypto().sha256(&leaf_preimage)
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

    fn is_ascii_decimal(&self, value: &ManagedBuffer) -> bool {
        if value.is_empty() {
            return false;
        }

        let mut valid = true;
        value.for_each_batch::<32, _>(|bytes| {
            for &byte in bytes {
                if !byte.is_ascii_digit() {
                    valid = false;
                    return;
                }
            }
        });

        valid
    }

    fn bytes_to_lower_hex(&self, bytes: &ManagedBuffer) -> ManagedBuffer {
        const HEX: &[u8; 16] = b"0123456789abcdef";

        let mut result = ManagedBuffer::new();
        bytes.for_each_batch::<32, _>(|raw| {
            for &byte in raw {
                result.append_bytes(&[HEX[(byte >> 4) as usize], HEX[(byte & 0x0f) as usize]]);
            }
        });
        result
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

    fn get_canonical_vm0042_totals(&self) -> (ManagedBuffer, BigUint, BigUint, BigUint) {
        use buffer_pool_proxy::BufferPoolProxy;
        use carbon_credit_proxy::CarbonCreditProxy;

        require!(
            !self.carbon_credit_addr().is_empty(),
            "carbon_credit_addr not configured"
        );
        require!(
            !self.buffer_pool_addr().is_empty(),
            "buffer_pool_addr not configured"
        );

        let carbon_credit_addr = self.carbon_credit_addr().get();
        let buffer_pool_addr = self.buffer_pool_addr().get();
        let gas_for_query = self.blockchain().get_gas_left() / 16;

        let dvcu_token_id: TokenIdentifier = self
            .tx()
            .to(&carbon_credit_addr)
            .gas(gas_for_query)
            .typed(CarbonCreditProxy)
            .get_dvcu_token_id()
            .returns(ReturnsResult)
            .sync_call_readonly();
        require!(
            !dvcu_token_id.as_managed_buffer().is_empty(),
            "DVCU token not configured on carbon-credit"
        );

        let buffer_token_id: TokenIdentifier = self
            .tx()
            .to(&buffer_pool_addr)
            .gas(gas_for_query)
            .typed(BufferPoolProxy)
            .get_buffer_token_id()
            .returns(ReturnsResult)
            .sync_call_readonly();
        require!(
            !buffer_token_id.as_managed_buffer().is_empty(),
            "buffer token not configured on buffer-pool"
        );

        let dvcu_minted: BigUint = self
            .tx()
            .to(&carbon_credit_addr)
            .gas(gas_for_query)
            .typed(CarbonCreditProxy)
            .get_total_dvcu_minted()
            .returns(ReturnsResult)
            .sync_call_readonly();
        let dvcu_burned: BigUint = self
            .tx()
            .to(&carbon_credit_addr)
            .gas(gas_for_query)
            .typed(CarbonCreditProxy)
            .get_total_dvcu_burned()
            .returns(ReturnsResult)
            .sync_call_readonly();
        let buffer_minted: BigUint = self
            .tx()
            .to(&buffer_pool_addr)
            .gas(gas_for_query)
            .typed(BufferPoolProxy)
            .get_total_buffer_minted()
            .returns(ReturnsResult)
            .sync_call_readonly();
        let buffer_burned: BigUint = self
            .tx()
            .to(&buffer_pool_addr)
            .gas(gas_for_query)
            .typed(BufferPoolProxy)
            .get_total_buffer_burned()
            .returns(ReturnsResult)
            .sync_call_readonly();
        let total_pool_balance: BigUint = self
            .tx()
            .to(&buffer_pool_addr)
            .gas(gas_for_query)
            .typed(BufferPoolProxy)
            .get_total_pool_balance()
            .returns(ReturnsResult)
            .sync_call_readonly();

        require!(
            dvcu_minted >= dvcu_burned,
            "INVALID_LIFECYCLE_COUNTERS: total dVCU burned exceeds minted"
        );
        require!(
            buffer_minted >= buffer_burned,
            "INVALID_RESERVE_COUNTERS: total reserve burned exceeds minted"
        );

        let canonical_supply = &dvcu_minted - &dvcu_burned;
        let canonical_buffer = &buffer_minted - &buffer_burned;
        let canonical_retired = dvcu_burned;

        require!(
            canonical_buffer == total_pool_balance,
            "BUFFER_BALANCE_MISMATCH: minted-burned reserve does not equal live pool balance"
        );

        (
            dvcu_token_id.as_managed_buffer().clone(),
            canonical_supply,
            canonical_buffer,
            canonical_retired,
        )
    }

    fn get_canonical_gsoc_totals(&self, project_id: &ManagedBuffer) -> (BigUint, BigUint, u64) {
        use carbon_credit_proxy::CarbonCreditProxy;

        require!(
            !self.carbon_credit_addr().is_empty(),
            "carbon_credit_addr not configured"
        );

        let carbon_credit_addr = self.carbon_credit_addr().get();
        let gas_for_query = self.blockchain().get_gas_left() / 16;

        let canonical_total_issued: BigUint = self
            .tx()
            .to(&carbon_credit_addr)
            .gas(gas_for_query)
            .typed(CarbonCreditProxy)
            .get_gsoc_project_total_issued(project_id.clone())
            .returns(ReturnsResult)
            .sync_call_readonly();
        let canonical_total_retired: BigUint = self
            .tx()
            .to(&carbon_credit_addr)
            .gas(gas_for_query)
            .typed(CarbonCreditProxy)
            .get_gsoc_project_total_retired(project_id.clone())
            .returns(ReturnsResult)
            .sync_call_readonly();
        let canonical_serial_count: u64 = self
            .tx()
            .to(&carbon_credit_addr)
            .gas(gas_for_query)
            .typed(CarbonCreditProxy)
            .get_gsoc_project_serial_count(project_id.clone())
            .returns(ReturnsResult)
            .sync_call_readonly();

        require!(
            canonical_total_issued >= canonical_total_retired,
            "INVALID_GSOC_LIFECYCLE_COUNTERS: retired exceeds issued"
        );

        (
            canonical_total_issued,
            canonical_total_retired,
            canonical_serial_count,
        )
    }

    fn get_canonical_gsoc_serial_inventory_hash(
        &self,
        project_id: &ManagedBuffer,
    ) -> ManagedBuffer {
        use carbon_credit_proxy::CarbonCreditProxy;

        require!(
            !self.carbon_credit_addr().is_empty(),
            "carbon_credit_addr not configured"
        );

        let carbon_credit_addr = self.carbon_credit_addr().get();
        let gas_for_query = self.blockchain().get_gas_left() / 16;

        self.tx()
            .to(&carbon_credit_addr)
            .gas(gas_for_query)
            .typed(CarbonCreditProxy)
            .get_canonical_gsoc_serial_inventory_hash(project_id.clone())
            .returns(ReturnsResult)
            .sync_call_readonly()
    }
}
