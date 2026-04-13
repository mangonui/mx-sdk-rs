#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod reserve_proof_registry_proxy;

/// Reserve proof for the VM0042 track.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
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
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
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
        self.require_governance_or_owner();
        require!(!token_id.is_empty(), "empty token_id");
        require!(!merkle_root.is_empty(), "empty merkle_root");
        require!(merkle_root.len() == 32, "merkle_root must be 32 bytes");
        require!(snapshot_block > 0, "invalid snapshot_block");

        let current_latest = self.latest_reserve_proof_block(&token_id).get();
        require!(
            snapshot_block > current_latest,
            "SNAPSHOT_BLOCK_NOT_MONOTONIC: new block must be greater than current latest"
        );

        require!(
            total_supply_scaled >= &total_buffer_scaled + &total_retired_scaled,
            "INVALID_RESERVE_ARITHMETIC: supply < buffer + retired"
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
            anchored_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
        };

        let key = (token_id.clone(), mrv_common::period_key(snapshot_block));
        self.reserve_proofs().insert(key, proof);
        self.latest_reserve_proof_block(&token_id).set(snapshot_block);

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
        self.require_governance_or_owner();
        require!(!project_id.is_empty(), "empty project_id");
        require!(!itmo_serial_hash.is_empty(), "empty itmo_serial_hash");
        require!(itmo_serial_hash.len() == 32, "itmo_serial_hash must be 32 bytes");
        require!(snapshot_block > 0, "invalid snapshot_block");

        let current_latest = self.latest_gsoc_proof_block(&project_id).get();
        require!(
            snapshot_block > current_latest,
            "SNAPSHOT_BLOCK_NOT_MONOTONIC: new block must be greater than current latest"
        );

        require!(
            total_issued >= total_retired,
            "INVALID_RESERVE_ARITHMETIC: issued < retired"
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
            anchored_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
        };

        let key = (project_id.clone(), mrv_common::period_key(snapshot_block));
        self.gsoc_reserve_proofs().insert(key, proof);
        self.latest_gsoc_proof_block(&project_id).set(snapshot_block);

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

    #[storage_mapper("reserveProofs")]
    fn reserve_proofs(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), ReserveProof<Self::Api>>;

    #[storage_mapper("gsocReserveProofs")]
    fn gsoc_reserve_proofs(&self) -> MapMapper<(ManagedBuffer, ManagedBuffer), GsocReserveProof<Self::Api>>;

    #[storage_mapper("latestReserveProofBlock")]
    fn latest_reserve_proof_block(&self, token_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[storage_mapper("latestGsocProofBlock")]
    fn latest_gsoc_proof_block(&self, project_id: &ManagedBuffer) -> SingleValueMapper<u64>;

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
