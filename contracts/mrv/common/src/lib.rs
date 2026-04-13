//! Shared governance helpers and ABI types for MRV contracts.
#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub type PublicId<M> = ManagedBuffer<M>;

const PENDING_GOVERNANCE_ACCEPTANCE_ROUNDS: u64 = 1_000;

/// Shared two-step governance transfer and `require_governance_or_owner`
/// guard. MRV contracts that need governance access control should
/// implement this trait (via `#[multiversx_sc::module]`).
#[multiversx_sc::module]
pub trait MrvGovernanceModule {
    /// Proposes a new governance address and starts the acceptance window.
    #[only_owner]
    #[endpoint(setGovernance)]
    fn set_governance(&self, governance: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        let expires_at_round = self
            .blockchain()
            .get_block_round()
            .saturating_add(PENDING_GOVERNANCE_ACCEPTANCE_ROUNDS);
        self.pending_governance().set(&governance);
        self.pending_governance_expires_at_round()
            .set(expires_at_round);
        self.mrv_governance_proposed_event(&governance);
    }

    /// Accepts a pending governance transfer before the acceptance window expires.
    #[endpoint(acceptGovernance)]
    fn accept_governance(&self) {
        require!(
            !self.pending_governance().is_empty(),
            "pending governance not set"
        );

        let caller = self.blockchain().get_caller();
        let pending = self.pending_governance().get();
        let expires_at_round = self.pending_governance_expires_at_round().get();
        require!(
            self.blockchain().get_block_round() <= expires_at_round,
            "pending governance acceptance expired"
        );
        require!(caller == pending, "caller not pending governance");

        self.governance().set(&pending);
        self.pending_governance().clear();
        self.pending_governance_expires_at_round().clear();
        self.mrv_governance_accepted_event(&pending);
    }

    /// Allows either the configured governance address or the contract owner.
    fn require_governance_or_owner(&self) {
        let caller = self.blockchain().get_caller();
        if !self.governance().is_empty() && caller == self.governance().get() {
            return;
        }

        require!(
            caller == self.blockchain().get_owner_address(),
            "caller not authorized"
        );
    }

    #[view(getGovernance)]
    #[storage_mapper("governance")]
    fn governance(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getPendingGovernance)]
    #[storage_mapper("pendingGovernance")]
    fn pending_governance(&self) -> SingleValueMapper<ManagedAddress>;

    /// Block round after which the pending governance proposal expires.
    #[storage_mapper("pendingGovernanceExpiresAtRound")]
    fn pending_governance_expires_at_round(&self) -> SingleValueMapper<u64>;

    #[event("mrvGovernanceProposed")]
    fn mrv_governance_proposed_event(&self, #[indexed] governance: &ManagedAddress);

    #[event("mrvGovernanceAccepted")]
    fn mrv_governance_accepted_event(&self, #[indexed] governance: &ManagedAddress);

    /// Revokes the current governance address, returning to owner-only control.
    #[only_owner]
    #[endpoint(revokeGovernance)]
    fn revoke_governance(&self) {
        let current = if !self.governance().is_empty() {
            self.governance().get()
        } else {
            ManagedAddress::zero()
        };
        self.governance().clear();
        self.pending_governance().clear();
        self.pending_governance_expires_at_round().clear();
        self.mrv_governance_revoked_event(&current);
    }

    #[event("mrvGovernanceRevoked")]
    fn mrv_governance_revoked_event(&self, #[indexed] previous: &ManagedAddress);
}

/// Encodes a `u64` as an 8-byte big-endian `ManagedBuffer` for use as a
/// composite storage key component (monitoring periods, snapshot blocks, etc.).
pub fn period_key<M: ManagedTypeApi>(n: u64) -> ManagedBuffer<M> {
    let mut buf = ManagedBuffer::new();
    buf.append_bytes(&n.to_be_bytes());
    buf
}

/// Encodes a `u8` source tag as a 1-byte `ManagedBuffer` for use as a
/// composite storage key component.
pub fn source_key<M: ManagedTypeApi>(s: u8) -> ManagedBuffer<M> {
    let mut buf = ManagedBuffer::new();
    buf.append_bytes(&[s]);
    buf
}

/// VVB accreditation record with role assignment and approval state.
///
/// Shared across MRV contracts that need to reference verifier accreditations
/// in cross-contract interactions or ABI generation.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct VerifierAccreditation<M: ManagedTypeApi> {
    pub verifier: ManagedAddress<M>,
    pub approved: bool,
    pub role: ManagedBuffer<M>,
    pub updated_at: u64,
}

/// GSOC verifier registry entry with credentials, jurisdiction, and approval state.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct GsocVerifierEntry<M: ManagedTypeApi> {
    pub credentials_cid: ManagedBuffer<M>,
    pub jurisdiction: ManagedBuffer<M>,
    pub registered_at: u64,
    pub approved: bool,
}

/// Anchored MRV report proof binding a `(tenant, farm, season)` tuple to a
/// content-addressed report hash and its evidence manifest.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct MrvReportProof<M: ManagedTypeApi> {
    pub report_id: PublicId<M>,
    pub public_tenant_id: PublicId<M>,
    pub public_farm_id: PublicId<M>,
    pub public_season_id: PublicId<M>,
    pub public_project_id: PublicId<M>,
    pub report_hash: ManagedBuffer<M>,
    pub hash_algo: ManagedBuffer<M>,
    pub canonicalization: ManagedBuffer<M>,
    pub methodology_version: u64,
    pub anchored_at: u64,
    pub evidence_manifest_hash: ManagedBuffer<M>,
}
