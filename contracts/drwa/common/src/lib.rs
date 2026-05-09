#![no_std]
//! Shared types, sync primitives, and validation utilities for the DRWA
//! contract suite. All four canonical contracts (`identity-registry`,
//! `policy-registry`, `asset-manager`, `attestation`) depend on this crate
//! for envelope construction, mirror sync invocation, and token-ID validation.

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use multiversx_sc::api::HandleConstraints;

#[cfg(not(target_arch = "wasm32"))]
extern crate std;

pub type TokenId<M> = ManagedBuffer<M>;
pub type HolderId<M> = ManagedAddress<M>;

pub const DRWA_SYNC_ENVELOPE_SCHEMA_VERSION: u16 = 1;
pub const DRWA_SYNC_ENVELOPE_SCHEMA_VERSION_WITH_RECOVERY: u16 = 2;

#[cfg(not(target_arch = "wasm32"))]
std::thread_local! {
    static DRWA_SYNC_HOOK_TEST_RESULT: core::cell::Cell<i32> = const { core::cell::Cell::new(0) };
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    fn managedDRWASyncMirror(payloadHandle: i32) -> i32;
    fn managedDRWANativeGovernanceQuery(queryType: i32, keyHandle: i32, destHandle: i32) -> i32;
}

pub const DRWA_NATIVE_GOVERNANCE_QUERY_CONFIG: i32 = 0;
pub const DRWA_NATIVE_GOVERNANCE_QUERY_PROPOSAL: i32 = 1;
pub const DRWA_NATIVE_GOVERNANCE_QUERY_AUDIT_RECORD: i32 = 2;
pub const DRWA_NATIVE_GOVERNANCE_QUERY_RECOVERY_LAST_BLOCK: i32 = 3;

/// Invokes the native DRWA mirror sync hook.
///
/// **Important:** On non-wasm targets (i.e. `cargo test`), this function uses
/// a process-local test result configured through
/// `set_drwa_sync_hook_test_result`. The default remains `0` (success), while
/// negative tests can force non-zero hook failures and prove contract rollback
/// behavior. The actual `managedDRWASyncMirror` hook is still exercised only by
/// chain simulator integration tests.
#[inline]
pub fn invoke_drwa_sync_hook(payload_handle: i32) -> i32 {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        managedDRWASyncMirror(payload_handle)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = payload_handle;
        DRWA_SYNC_HOOK_TEST_RESULT.with(|result| result.get())
    }
}

/// Queries native DRWA governance state through the VM hook.
///
/// The result is VM-provided bytes. Governance config, proposal, and audit
/// queries return JSON encoded records; recovery-last-block returns an 8-byte
/// big-endian nonce. Non-wasm tests return `None` because native chain-go state
/// is not available in the Rust unit-test host.
#[inline]
pub fn invoke_drwa_native_governance_query<M: ManagedTypeApi>(
    query_type: i32,
    key: &ManagedBuffer<M>,
) -> OptionalValue<ManagedBuffer<M>> {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        let mut result = ManagedBuffer::new();
        let rc = managedDRWANativeGovernanceQuery(
            query_type,
            key.get_handle().get_raw_handle(),
            result.get_handle().get_raw_handle(),
        );
        if rc == 0 {
            OptionalValue::Some(result)
        } else {
            OptionalValue::None
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = query_type;
        let _ = key;
        OptionalValue::None
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_drwa_sync_hook_test_result(result: i32) {
    DRWA_SYNC_HOOK_TEST_RESULT.with(|stored| stored.set(result));
}

/// Validates the MultiversX token identifier format: `TICKER-abcdef`, where
/// `TICKER` is 3-10 uppercase alphanumeric characters and the suffix is
/// exactly 6 lowercase hexadecimal characters.
pub fn require_valid_token_id<M: ManagedTypeApi>(token_id: &ManagedBuffer<M>) {
    if token_id.is_empty() {
        M::error_api_impl().signal_error(b"token_id must not be empty");
    }

    let len = token_id.len();
    if len < 8 {
        M::error_api_impl().signal_error(b"token_id is too short");
    }
    if len > 17 {
        M::error_api_impl().signal_error(b"token_id is too long");
    }

    let mut bytes = [0u8; 17];
    token_id.load_slice(0, &mut bytes[..len]);
    let token_id_bytes = &bytes[..len];

    if token_id_bytes.contains(&0) {
        M::error_api_impl().signal_error(b"token_id must not contain null bytes");
    }
    let hyphen_pos = token_id_bytes
        .iter()
        .position(|b| *b == b'-')
        .unwrap_or(token_id_bytes.len());
    if token_id_bytes.iter().filter(|b| **b == b'-').count() != 1 {
        M::error_api_impl().signal_error(b"token_id must contain exactly one hyphen");
    }
    if hyphen_pos < 3 {
        M::error_api_impl().signal_error(b"token_id ticker is too short");
    }
    if hyphen_pos > 10 {
        M::error_api_impl().signal_error(b"token_id ticker is too long (max 10 chars)");
    }
    if hyphen_pos + 7 != token_id_bytes.len() {
        M::error_api_impl().signal_error(b"token_id suffix must be 6 characters");
    }

    for (index, byte) in token_id_bytes.iter().enumerate() {
        if index < hyphen_pos {
            if !(byte.is_ascii_uppercase() || byte.is_ascii_digit()) {
                M::error_api_impl().signal_error(b"token_id ticker must be uppercase alphanumeric");
            }
        } else if index > hyphen_pos && !(byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)) {
            M::error_api_impl().signal_error(b"token_id suffix must be lowercase hex");
        }
    }
}

/// Validates that a KYC status string is one of the allowed values.
/// Prevents operator typos from silently denying holders.
pub fn require_valid_kyc_status<M: ManagedTypeApi>(status: &ManagedBuffer<M>) {
    let len = status.len();
    require_status_len::<M>(len);
    let mut bytes = [0u8; 16];
    status.load_slice(0, &mut bytes[..len]);
    let s = &bytes[..len];
    let allowed: &[&[u8]] = &[
        b"approved",
        b"pending",
        b"rejected",
        b"expired",
        b"not_started",
        b"deactivated",
    ];
    if !allowed.contains(&s) {
        M::error_api_impl().signal_error(
            b"invalid kyc_status: must be one of approved, pending, rejected, expired, not_started, deactivated",
        );
    }
}

/// Validates that an AML status string is one of the allowed values.
pub fn require_valid_aml_status<M: ManagedTypeApi>(status: &ManagedBuffer<M>) {
    let len = status.len();
    require_status_len::<M>(len);
    let mut bytes = [0u8; 16];
    status.load_slice(0, &mut bytes[..len]);
    let s = &bytes[..len];
    let allowed: &[&[u8]] = &[
        b"clear",
        b"pending",
        b"flagged",
        b"review",
        b"blocked",
        b"not_started",
        b"deactivated",
    ];
    if !allowed.contains(&s) {
        M::error_api_impl().signal_error(
            b"invalid aml_status: must be one of clear, pending, flagged, review, blocked, not_started, deactivated",
        );
    }
}

fn require_status_len<M: ManagedTypeApi>(len: usize) {
    if len == 0 || len > 16 {
        M::error_api_impl().signal_error(b"invalid status length");
    }
}

/// Enumerates the sync operation payloads accepted by the native DRWA mirror.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub enum DrwaSyncOperationType {
    TokenPolicy,
    AssetRecord,
    HolderMirror,
    HolderProfile,
    HolderAuditorAuthorization,
    HolderMirrorDelete,
    AuthorizedCallerUpdate,
    GovernanceApprove,
    GovernanceExecute,
}

/// Identifies the contract domain that produced a sync envelope.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub enum DrwaCallerDomain {
    PolicyRegistry,
    AssetManager,
    IdentityRegistry,
    Attestation,
    RecoveryAdmin,
    AuthAdmin,
}

/// Represents the per-token policy mirrored to the native DRWA layer.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct DrwaTokenPolicy<M: ManagedTypeApi> {
    pub drwa_enabled: bool,
    pub global_pause: bool,
    pub strict_auditor_mode: bool,
    pub metadata_protection_enabled: bool,
    pub token_policy_version: u64,
    pub allowed_investor_classes: ManagedVec<M, ManagedBuffer<M>>,
    pub allowed_jurisdictions: ManagedVec<M, ManagedBuffer<M>>,
}

/// Per-holder, per-token compliance state mirrored to the native DRWA layer.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct DrwaHolderMirror<M: ManagedTypeApi> {
    pub holder_policy_version: u64,
    pub kyc_status: ManagedBuffer<M>,
    pub aml_status: ManagedBuffer<M>,
    pub investor_class: ManagedBuffer<M>,
    pub jurisdiction_code: ManagedBuffer<M>,
    pub expiry_round: u64,
    pub transfer_locked: bool,
    pub receive_locked: bool,
    pub auditor_authorized: bool,
}

/// Per-holder identity profile mirrored to the native DRWA layer.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct DrwaHolderProfile<M: ManagedTypeApi> {
    pub holder_profile_version: u64,
    pub kyc_status: ManagedBuffer<M>,
    pub aml_status: ManagedBuffer<M>,
    pub investor_class: ManagedBuffer<M>,
    pub jurisdiction_code: ManagedBuffer<M>,
    pub expiry_round: u64,
}

/// Per-holder auditor authorization state mirrored to the native DRWA layer.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct DrwaHolderAuditorAuthorization {
    pub holder_auditor_authorization_version: u64,
    pub auditor_authorized: bool,
}

/// Carries a single versioned sync operation inside an envelope.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct DrwaSyncOperation<M: ManagedTypeApi> {
    pub operation_type: DrwaSyncOperationType,
    pub token_id: ManagedBuffer<M>,
    pub holder: ManagedAddress<M>,
    pub version: u64,
    pub body: ManagedBuffer<M>,
}

/// Wraps the caller domain, payload hash, and batched operations for mirror sync.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct DrwaSyncEnvelope<M: ManagedTypeApi> {
    pub schema_version: u16,
    pub caller_domain: DrwaCallerDomain,
    pub payload_hash: ManagedBuffer<M>,
    pub operations: ManagedVec<M, DrwaSyncOperation<M>>,
    pub pre_recovery_state_hash: ManagedBuffer<M>,
    pub recovery_scope: ManagedVec<M, ManagedBuffer<M>>,
}

/// Serializes the canonical payload hashed and forwarded to the native mirror.
pub fn serialize_sync_envelope_payload<M: ManagedTypeApi>(
    caller_domain: &DrwaCallerDomain,
    operations: &ManagedVec<M, DrwaSyncOperation<M>>,
) -> ManagedBuffer<M> {
    let mut result = ManagedBuffer::new();
    result.append_bytes(&DRWA_SYNC_ENVELOPE_SCHEMA_VERSION.to_be_bytes());
    let caller_tag = match caller_domain {
        DrwaCallerDomain::PolicyRegistry => 0u8,
        DrwaCallerDomain::AssetManager => 1u8,
        DrwaCallerDomain::IdentityRegistry => 2u8,
        DrwaCallerDomain::Attestation => 3u8,
        DrwaCallerDomain::RecoveryAdmin => 4u8,
        DrwaCallerDomain::AuthAdmin => 5u8,
    };
    result.append_bytes(&[caller_tag]);

    for operation in operations.iter() {
        let op_tag = match operation.operation_type {
            DrwaSyncOperationType::TokenPolicy => 0u8,
            DrwaSyncOperationType::AssetRecord => 1u8,
            DrwaSyncOperationType::HolderMirror => 2u8,
            DrwaSyncOperationType::HolderProfile => 3u8,
            DrwaSyncOperationType::HolderAuditorAuthorization => 4u8,
            DrwaSyncOperationType::HolderMirrorDelete => 5u8,
            DrwaSyncOperationType::AuthorizedCallerUpdate => 6u8,
            DrwaSyncOperationType::GovernanceApprove => 7u8,
            DrwaSyncOperationType::GovernanceExecute => 8u8,
        };
        result.append_bytes(&[op_tag]);
        push_len_prefixed(&mut result, &operation.token_id);
        push_len_prefixed(&mut result, operation.holder.as_managed_buffer());
        result.append_bytes(&operation.version.to_be_bytes());
        push_len_prefixed(&mut result, &operation.body);
    }

    result
}

/// Appends a value as a 4-byte big-endian length followed by its raw bytes.
pub fn push_len_prefixed<M: ManagedTypeApi>(dest: &mut ManagedBuffer<M>, value: &ManagedBuffer<M>) {
    let len = value.len() as u32;
    dest.append_bytes(&len.to_be_bytes());
    dest.append(value);
}

const PENDING_GOVERNANCE_ACCEPTANCE_ROUNDS: u64 = 1_000;

/// Shared two-step governance transfer, privileged-call guard, and
/// `emit_sync_envelope` helper. DRWA contracts that need governance access
/// control and native mirror sync should inherit this trait as a supertrait.
#[multiversx_sc::module]
pub trait DrwaGovernanceModule {
    /// Proposes a new governance address and starts the acceptance window.
    #[endpoint(setGovernance)]
    fn set_governance(&self, governance: ManagedAddress) {
        self.require_governance_transfer_authority();
        require!(!governance.is_zero(), "governance must not be zero");
        let expires_at_round = self
            .blockchain()
            .get_block_round()
            .checked_add(PENDING_GOVERNANCE_ACCEPTANCE_ROUNDS)
            .unwrap_or_else(|| sc_panic!("governance acceptance round overflow"));
        self.pending_governance().set(&governance);
        self.pending_governance_expires_at_round()
            .set(expires_at_round);
        self.drwa_governance_proposed_event(&governance);
    }

    /// Accepts a pending governance transfer before the acceptance window
    /// expires.
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
        self.drwa_governance_accepted_event(&pending);
    }

    /// Revokes the current governance address, clearing all governance and
    /// pending governance state. Once governance is configured, only that
    /// governance address may call this endpoint.
    #[endpoint(revokeGovernance)]
    fn revoke_governance(&self) {
        self.require_governance_transfer_authority();
        let previous = self.governance().get();
        self.drwa_governance_revoked_event(&previous);
        self.governance().clear();
        self.pending_governance().clear();
        self.pending_governance_expires_at_round().clear();
    }

    /// Allows the configured governance address. The contract owner is a
    /// bootstrap fallback only while no governance address is configured.
    fn require_governance_or_owner(&self) {
        self.require_governance_transfer_authority();
    }

    /// Enforces governance authority after bootstrap. This prevents the
    /// deployer owner from bypassing an already-configured DRWA governor.
    fn require_governance_transfer_authority(&self) {
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

    /// The active governance address authorized to manage compliance state.
    #[view(getGovernance)]
    #[storage_mapper("governance")]
    fn governance(&self) -> SingleValueMapper<ManagedAddress>;

    /// The proposed governance address awaiting acceptance.
    #[view(getPendingGovernance)]
    #[storage_mapper("pendingGovernance")]
    fn pending_governance(&self) -> SingleValueMapper<ManagedAddress>;

    /// Block round after which the pending governance proposal expires.
    #[storage_mapper("pendingGovernanceExpiresAtRound")]
    fn pending_governance_expires_at_round(&self) -> SingleValueMapper<u64>;

    /// Emits when a new governance address is proposed.
    #[event("drwaGovernanceProposed")]
    fn drwa_governance_proposed_event(&self, #[indexed] governance: &ManagedAddress);

    /// Emits when a pending governance address accepts the role.
    #[event("drwaGovernanceAccepted")]
    fn drwa_governance_accepted_event(&self, #[indexed] governance: &ManagedAddress);

    /// Emits when the governance address is revoked by the owner.
    #[event("drwaGovernanceRevoked")]
    fn drwa_governance_revoked_event(&self, #[indexed] previous_governance: &ManagedAddress);

    /// Computes the keccak256 payload hash, invokes the native DRWA mirror
    /// sync hook, verifies success, and returns the constructed envelope.
    ///
    /// INTENTIONAL: The `require!` reverts the entire transaction if the sync
    /// hook returns non-zero. There is no retry or queueing by design — the
    /// contract and the Go-side native mirror must never diverge. See
    /// `docs/DRWA-Binary-Sync-Format.md` "Sync Failure Handling" for the
    /// operational mitigation and recovery procedure.
    fn emit_sync_envelope(
        &self,
        caller_domain: DrwaCallerDomain,
        operations: ManagedVec<DrwaSyncOperation<Self::Api>>,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let payload_hash = self
            .crypto()
            .keccak256(serialize_sync_envelope_payload(&caller_domain, &operations))
            .as_managed_buffer()
            .clone();

        let hook_payload = build_sync_hook_payload(&caller_domain, &operations, &payload_hash);
        require!(
            invoke_drwa_sync_hook(hook_payload.get_handle().get_raw_handle()) == 0,
            "native mirror sync failed"
        );

        DrwaSyncEnvelope {
            schema_version: DRWA_SYNC_ENVELOPE_SCHEMA_VERSION,
            caller_domain,
            payload_hash,
            operations,
            pre_recovery_state_hash: ManagedBuffer::new(),
            recovery_scope: ManagedVec::new(),
        }
    }

    fn emit_recovery_sync_envelope(
        &self,
        operations: ManagedVec<DrwaSyncOperation<Self::Api>>,
        pre_recovery_state_hash: ManagedBuffer,
        recovery_scope: ManagedVec<ManagedBuffer>,
    ) -> DrwaSyncEnvelope<Self::Api> {
        require!(!recovery_scope.is_empty(), "recovery scope required");
        let caller_domain = DrwaCallerDomain::RecoveryAdmin;
        let payload_hash = self
            .crypto()
            .keccak256(serialize_sync_envelope_payload(&caller_domain, &operations))
            .as_managed_buffer()
            .clone();

        let hook_payload = build_sync_hook_payload_with_recovery_metadata(
            &caller_domain,
            &operations,
            &payload_hash,
            &pre_recovery_state_hash,
            &recovery_scope,
        );
        require!(
            invoke_drwa_sync_hook(hook_payload.get_handle().get_raw_handle()) == 0,
            "native mirror sync failed"
        );

        DrwaSyncEnvelope {
            schema_version: DRWA_SYNC_ENVELOPE_SCHEMA_VERSION_WITH_RECOVERY,
            caller_domain,
            payload_hash,
            operations,
            pre_recovery_state_hash,
            recovery_scope,
        }
    }

    /// Builds a valid envelope without invoking the native sync hook.
    ///
    /// Used for idempotent write attempts where the requested state already
    /// matches storage and no mirror update is required.
    fn emit_sync_noop_envelope(
        &self,
        caller_domain: DrwaCallerDomain,
    ) -> DrwaSyncEnvelope<Self::Api> {
        let operations = ManagedVec::new();
        let payload_hash = self
            .crypto()
            .keccak256(serialize_sync_envelope_payload(&caller_domain, &operations))
            .as_managed_buffer()
            .clone();

        DrwaSyncEnvelope {
            schema_version: DRWA_SYNC_ENVELOPE_SCHEMA_VERSION,
            caller_domain,
            payload_hash,
            operations,
            pre_recovery_state_hash: ManagedBuffer::new(),
            recovery_scope: ManagedVec::new(),
        }
    }
}

/// Builds the binary hook payload passed to `managedDRWASyncMirror`.
///
/// Format:
/// `[32-byte keccak256 payload_hash] || [schema_version:u16] || [canonical binary payload]`.
/// The Go-side decoder detects this binary form by checking that the first
/// byte is not `{`, then splitting the payload at offset `32`.
pub fn build_sync_hook_payload<M: ManagedTypeApi>(
    caller_domain: &DrwaCallerDomain,
    operations: &ManagedVec<M, DrwaSyncOperation<M>>,
    payload_hash: &ManagedBuffer<M>,
) -> ManagedBuffer<M> {
    let canonical_payload = serialize_sync_envelope_payload(caller_domain, operations);
    let mut result = ManagedBuffer::new();
    result.append(payload_hash);
    result.append(&canonical_payload);
    result
}

pub fn build_sync_hook_payload_with_recovery_metadata<M: ManagedTypeApi>(
    caller_domain: &DrwaCallerDomain,
    operations: &ManagedVec<M, DrwaSyncOperation<M>>,
    payload_hash: &ManagedBuffer<M>,
    pre_recovery_state_hash: &ManagedBuffer<M>,
    recovery_scope: &ManagedVec<M, ManagedBuffer<M>>,
) -> ManagedBuffer<M> {
    let mut canonical_payload = ManagedBuffer::new();
    canonical_payload.append_bytes(&DRWA_SYNC_ENVELOPE_SCHEMA_VERSION_WITH_RECOVERY.to_be_bytes());
    canonical_payload.append_bytes(&[match caller_domain {
        DrwaCallerDomain::PolicyRegistry => 0u8,
        DrwaCallerDomain::AssetManager => 1u8,
        DrwaCallerDomain::IdentityRegistry => 2u8,
        DrwaCallerDomain::Attestation => 3u8,
        DrwaCallerDomain::RecoveryAdmin => 4u8,
        DrwaCallerDomain::AuthAdmin => 5u8,
    }]);
    push_len_prefixed(&mut canonical_payload, pre_recovery_state_hash);
    canonical_payload.append_bytes(&(recovery_scope.len() as u16).to_be_bytes());
    for scope_index in 0..recovery_scope.len() {
        push_len_prefixed(&mut canonical_payload, &recovery_scope.get(scope_index));
    }
    canonical_payload.append_bytes(&(operations.len() as u16).to_be_bytes());
    append_sync_operations(&mut canonical_payload, operations);

    let mut result = ManagedBuffer::new();
    result.append(payload_hash);
    result.append(&canonical_payload);
    result
}

fn append_sync_operations<M: ManagedTypeApi>(
    result: &mut ManagedBuffer<M>,
    operations: &ManagedVec<M, DrwaSyncOperation<M>>,
) {
    for operation in operations.iter() {
        let op_tag = match operation.operation_type {
            DrwaSyncOperationType::TokenPolicy => 0u8,
            DrwaSyncOperationType::AssetRecord => 1u8,
            DrwaSyncOperationType::HolderMirror => 2u8,
            DrwaSyncOperationType::HolderProfile => 3u8,
            DrwaSyncOperationType::HolderAuditorAuthorization => 4u8,
            DrwaSyncOperationType::HolderMirrorDelete => 5u8,
            DrwaSyncOperationType::AuthorizedCallerUpdate => 6u8,
            DrwaSyncOperationType::GovernanceApprove => 7u8,
            DrwaSyncOperationType::GovernanceExecute => 8u8,
        };
        result.append_bytes(&[op_tag]);
        push_len_prefixed(result, &operation.token_id);
        push_len_prefixed(result, operation.holder.as_managed_buffer());
        result.append_bytes(&operation.version.to_be_bytes());
        push_len_prefixed(result, &operation.body);
    }
}
