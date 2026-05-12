//! Shared governance helpers and ABI types for MRV contracts.
#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub type PublicId<M> = ManagedBuffer<M>;

const PENDING_GOVERNANCE_ACCEPTANCE_ROUNDS: u64 = 1_000;

pub const STORAGE_VERSION_UNINITIALIZED: u32 = 0;

/// Resolves an MRV contract upgrade target in a fail-closed way.
///
/// Rules:
/// - `0` means the legacy contract never wrote a storage-version slot; this is
///   treated as bootstrap initialization and upgrades directly to `current`.
/// - `minimum_supported..=current` are accepted.
/// - older legacy versions and any future version are rejected explicitly.
pub fn resolve_storage_version_upgrade(
    stored: u32,
    current: u32,
    minimum_supported: u32,
) -> Result<u32, &'static str> {
    if stored == STORAGE_VERSION_UNINITIALIZED {
        return Ok(current);
    }

    if stored > current {
        return Err("unsupported future storage version");
    }

    if stored < minimum_supported {
        return Err("unsupported legacy storage version; explicit migration required");
    }

    Ok(current)
}

/// Shared two-step governance transfer and `require_governance_or_owner`
/// guard. MRV contracts that need governance access control should
/// implement this trait (via `#[multiversx_sc::module]`).
#[multiversx_sc::module]
pub trait MrvGovernanceModule {
    /// Proposes the initial governance address during bootstrap, or rotates
    /// governance when called by the currently active governance address.
    #[endpoint(setGovernance)]
    fn set_governance(&self, governance: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        let caller = self.blockchain().get_caller();
        if !self.governance().is_empty() {
            require!(caller == self.governance().get(), "caller not authorized");
        } else {
            require!(
                caller == self.blockchain().get_owner_address(),
                "caller not authorized"
            );
        }
        // checked_add over saturating_add: an unbounded clamp at
        // u64::MAX would silently turn the acceptance-window
        // expiration into "never expires", which violates the
        // governance-transfer invariant. The overflow is unreachable
        // in practice (~10^19 rounds horizon) but the project's
        // standing arithmetic policy is checked_add with explicit
        // panic on the impossible case.
        let expires_at_round = self
            .blockchain()
            .get_block_round()
            .checked_add(PENDING_GOVERNANCE_ACCEPTANCE_ROUNDS)
            .unwrap_or_else(|| sc_panic!("block round + acceptance window overflow"));
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

    /// Allows the configured governance address once governance is active.
    /// Falls back to owner-only during bootstrap before governance exists.
    fn require_governance_or_owner(&self) {
        let caller = self.blockchain().get_caller();
        if !self.governance().is_empty() {
            require!(caller == self.governance().get(), "caller not authorized");
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

    /// Cancels bootstrap-time pending governance only while no active
    /// governance has yet been accepted. Once governance is active, control is
    /// irreversible and cannot be revoked back to owner-only.
    #[endpoint(revokeGovernance)]
    fn revoke_governance(&self) {
        require!(
            self.blockchain().get_caller() == self.blockchain().get_owner_address(),
            "caller not authorized"
        );
        require!(
            self.governance().is_empty(),
            "active governance cannot be revoked"
        );
        self.governance().clear();
        self.pending_governance().clear();
        self.pending_governance_expires_at_round().clear();
        self.mrv_governance_revoked_event(&ManagedAddress::zero());
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

#[cfg(test)]
mod storage_version_tests {
    use super::resolve_storage_version_upgrade;

    #[test]
    fn storage_version_upgrade_bootstraps_uninitialized_slot() {
        assert_eq!(resolve_storage_version_upgrade(0, 2, 2), Ok(2));
    }

    #[test]
    fn storage_version_upgrade_accepts_current_version() {
        assert_eq!(resolve_storage_version_upgrade(1, 1, 1), Ok(1));
    }

    #[test]
    fn storage_version_upgrade_rejects_unsupported_legacy_version() {
        assert_eq!(
            resolve_storage_version_upgrade(1, 2, 2),
            Err("unsupported legacy storage version; explicit migration required")
        );
    }

    #[test]
    fn storage_version_upgrade_rejects_future_version() {
        assert_eq!(
            resolve_storage_version_upgrade(3, 2, 2),
            Err("unsupported future storage version")
        );
    }
}
