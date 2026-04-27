#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod drwa_policy_registry_proxy;

use drwa_common::{
    DrwaCallerDomain, DrwaSyncEnvelope, DrwaSyncOperation, DrwaSyncOperationType, DrwaTokenPolicy,
    require_valid_token_id,
};

const MAX_INVESTOR_CLASSES: usize = 100;
const MAX_JURISDICTIONS: usize = 200;

/// Manages per-token compliance policies (pause, auditor mode, investor-class
/// and jurisdiction allow-lists) and syncs policy state to the native DRWA
/// mirror on every mutation.
///
/// Governance is transferable via a propose-accept pattern with a time-limited
/// acceptance window.
#[multiversx_sc::contract]
pub trait DrwaPolicyRegistry: drwa_common::DrwaGovernanceModule {
    /// Initializes the contract with the governance address.
    #[init]
    fn init(&self, governance: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        self.governance().set(&governance);
        self.storage_version().set(1u32);
    }

    /// Creates or updates a token policy, increments its version, and syncs it
    /// to the native mirror.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if the token identifier is invalid or the input lists exceed the
    /// configured maximum sizes.
    #[endpoint(setTokenPolicy)]
    fn set_token_policy(
        &self,
        token_id: ManagedBuffer,
        drwa_enabled: bool,
        global_pause: bool,
        strict_auditor_mode: bool,
        metadata_protection_enabled: bool,
        allowed_investor_classes: ManagedVec<ManagedBuffer>,
        allowed_jurisdictions: ManagedVec<ManagedBuffer>,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();

        self.require_valid_token_id(&token_id);
        require!(
            allowed_investor_classes.len() <= MAX_INVESTOR_CLASSES,
            "too many investor classes: max 100"
        );
        require!(
            allowed_jurisdictions.len() <= MAX_JURISDICTIONS,
            "too many jurisdictions: max 200"
        );

        if !self.token_policy(&token_id).is_empty() {
            let current = self.token_policy(&token_id).get();
            let requested = DrwaTokenPolicy {
                drwa_enabled,
                global_pause,
                strict_auditor_mode,
                metadata_protection_enabled,
                token_policy_version: current.token_policy_version,
                allowed_investor_classes: allowed_investor_classes.clone(),
                allowed_jurisdictions: allowed_jurisdictions.clone(),
            };
            if current == requested {
                return self.emit_sync_noop_envelope(DrwaCallerDomain::PolicyRegistry);
            }
        }

        let next_version = self
            .token_policy_version(&token_id)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));

        let policy = DrwaTokenPolicy {
            drwa_enabled,
            global_pause,
            strict_auditor_mode,
            metadata_protection_enabled,
            token_policy_version: next_version,
            allowed_investor_classes,
            allowed_jurisdictions,
        };

        self.token_policy(&token_id).set(policy.clone());
        self.token_policy_version(&token_id).set(next_version);
        self.drwa_token_policy_event(
            &token_id,
            policy.drwa_enabled,
            policy.global_pause,
            policy.strict_auditor_mode,
            next_version,
        );

        let body = self.serialize_policy_json(&policy);
        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::TokenPolicy,
            token_id: token_id.clone(),
            holder: ManagedAddress::default(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::PolicyRegistry, operations)
    }

    /// Deactivates an existing token policy by setting `drwa_enabled = false`,
    /// incrementing its version, and syncing the update to the native mirror.
    ///
    /// Access is limited to the governance address or the contract owner.
    /// Reverts if the token policy does not exist.
    #[endpoint(deactivateTokenPolicy)]
    fn deactivate_token_policy(&self, token_id: ManagedBuffer) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        self.require_valid_token_id(&token_id);
        require!(
            !self.token_policy(&token_id).is_empty(),
            "token policy does not exist"
        );

        let mut policy = self.token_policy(&token_id).get();
        if !policy.drwa_enabled {
            return self.emit_sync_noop_envelope(DrwaCallerDomain::PolicyRegistry);
        }
        policy.drwa_enabled = false;

        let next_version = self
            .token_policy_version(&token_id)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));
        policy.token_policy_version = next_version;

        self.token_policy(&token_id).set(policy.clone());
        self.token_policy_version(&token_id).set(next_version);
        self.drwa_token_policy_event(
            &token_id,
            policy.drwa_enabled,
            policy.global_pause,
            policy.strict_auditor_mode,
            next_version,
        );

        let body = self.serialize_policy_json(&policy);
        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::TokenPolicy,
            token_id: token_id.clone(),
            holder: ManagedAddress::default(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::PolicyRegistry, operations)
    }

    /// Maps a token identifier to its full compliance policy.
    #[view(getTokenPolicy)]
    #[storage_mapper("tokenPolicy")]
    fn token_policy(
        &self,
        token_id: &ManagedBuffer,
    ) -> SingleValueMapper<DrwaTokenPolicy<Self::Api>>;

    /// Monotonically increasing version counter per token, used for staleness
    /// detection.
    #[view(getTokenPolicyVersion)]
    #[storage_mapper("tokenPolicyVersion")]
    fn token_policy_version(&self, token_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    /// Emits when a token policy is created or updated.
    #[event("drwaTokenPolicy")]
    fn drwa_token_policy_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] drwa_enabled: bool,
        #[indexed] global_pause: bool,
        #[indexed] strict_auditor_mode: bool,
        #[indexed] token_policy_version: u64,
    );

    // ── MiCA White Paper CID & Registration Status ──────────────────────

    /// Sets the IPFS CID of the MiCA white paper for a regulated token and
    /// syncs the updated policy to the native mirror.
    ///
    /// CID format: must start with "Qm" (CIDv0) or "bafy" (CIDv1) and be
    /// 46-64 bytes long. Access is limited to governance or owner.
    #[endpoint(setWhitePaperCid)]
    fn set_white_paper_cid(
        &self,
        token_id: ManagedBuffer,
        cid: ManagedBuffer,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        self.require_valid_token_id(&token_id);
        require!(!cid.is_empty(), "white paper CID is required");

        let cid_len = cid.len();
        require!(
            cid_len >= 46 && cid_len <= 64,
            "invalid CID length: must be 46-64 characters"
        );
        let mut cid_bytes = [0u8; 64];
        cid.load_slice(0, &mut cid_bytes[..cid_len]);
        let cid_slice = &cid_bytes[..cid_len];
        require!(
            cid_slice.starts_with(b"Qm") || cid_slice.starts_with(b"bafy"),
            "CID must start with Qm (v0) or bafy (v1)"
        );

        // F-017: reject CID bytes outside Base58/Base32 alphabet to prevent JSON injection
        for &b in cid_slice {
            require!(
                b.is_ascii_alphanumeric(),
                "CID contains invalid byte: only alphanumeric characters (a-z, A-Z, 0-9) allowed"
            );
        }

        if !self.white_paper_cid(&token_id).is_empty()
            && self.white_paper_cid(&token_id).get() == cid
        {
            return self.emit_sync_noop_envelope(DrwaCallerDomain::PolicyRegistry);
        }

        self.white_paper_cid(&token_id).set(cid.clone());

        self.drwa_white_paper_cid_set_event(&token_id, &cid);

        // Bundled as a TokenPolicy sync so the Go gate receives the CID
        // alongside existing enforcement fields in a single atomic update.
        let next_version = self
            .token_policy_version(&token_id)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));
        self.token_policy_version(&token_id).set(next_version);

        let body = self.serialize_full_policy_with_mica_json(&token_id, Some(&cid), None);
        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::TokenPolicy,
            token_id: token_id.clone(),
            holder: ManagedAddress::default(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::PolicyRegistry, operations)
    }

    /// Sets the MiCA registration status for a regulated token and syncs
    /// the update to the native mirror.
    ///
    /// Valid statuses: `draft`, `submitted`, `approved`, `rejected`, `withdrawn`.
    #[endpoint(setRegistrationStatus)]
    fn set_registration_status(
        &self,
        token_id: ManagedBuffer,
        status: ManagedBuffer,
    ) -> DrwaSyncEnvelope<Self::Api> {
        self.require_governance_or_owner();
        self.require_valid_token_id(&token_id);

        let status_len = status.len();
        let mut status_bytes = [0u8; 10];
        require!(
            status_len <= status_bytes.len(),
            "invalid registration status: must be draft, submitted, approved, rejected, or withdrawn"
        );
        status.load_slice(0, &mut status_bytes[..status_len]);
        let status_str = &status_bytes[..status_len];
        require!(
            status_str == b"draft"
                || status_str == b"submitted"
                || status_str == b"approved"
                || status_str == b"rejected"
                || status_str == b"withdrawn",
            "invalid registration status: must be draft, submitted, approved, rejected, or withdrawn"
        );

        if !self.registration_status(&token_id).is_empty()
            && self.registration_status(&token_id).get() == status
        {
            return self.emit_sync_noop_envelope(DrwaCallerDomain::PolicyRegistry);
        }

        self.registration_status(&token_id).set(status.clone());

        self.drwa_registration_status_set_event(&token_id, &status);

        let next_version = self
            .token_policy_version(&token_id)
            .get()
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("version overflow"));
        self.token_policy_version(&token_id).set(next_version);

        let body = self.serialize_full_policy_with_mica_json(&token_id, None, Some(&status));
        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::TokenPolicy,
            token_id: token_id.clone(),
            holder: ManagedAddress::default(),
            version: next_version,
            body,
        });

        self.emit_sync_envelope(DrwaCallerDomain::PolicyRegistry, operations)
    }

    /// Returns the white paper CID for a token, or empty if not set.
    #[view(getWhitePaperCid)]
    fn get_white_paper_cid(&self, token_id: ManagedBuffer) -> ManagedBuffer {
        self.white_paper_cid(&token_id).get()
    }

    /// Returns the registration status for a token, or empty if not set.
    #[view(getRegistrationStatus)]
    fn get_registration_status(&self, token_id: ManagedBuffer) -> ManagedBuffer {
        self.registration_status(&token_id).get()
    }

    /// Maps a token identifier to its MiCA white paper IPFS CID.
    #[storage_mapper("whitePaperCid")]
    fn white_paper_cid(&self, token_id: &ManagedBuffer) -> SingleValueMapper<ManagedBuffer>;

    /// Maps a token identifier to its MiCA registration status.
    #[storage_mapper("registrationStatus")]
    fn registration_status(&self, token_id: &ManagedBuffer) -> SingleValueMapper<ManagedBuffer>;

    /// Emits when a white paper CID is set for a token.
    #[event("drwaWhitePaperCidSet")]
    fn drwa_white_paper_cid_set_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] cid: &ManagedBuffer,
    );

    /// Emits when a registration status is set for a token.
    #[event("drwaRegistrationStatusSet")]
    fn drwa_registration_status_set_event(
        &self,
        #[indexed] token_id: &ManagedBuffer,
        #[indexed] status: &ManagedBuffer,
    );

    /// Serializes a complete policy JSON body with optional MiCA fields
    /// (`white_paper_cid`, `registration_status`) appended.
    ///
    /// Loads the current policy from storage so the Go-side full-replacement
    /// `SaveKeyValue` preserves all enforcement fields. Falls back to safe
    /// defaults (`drwa_enabled=false`, `global_pause=false`, etc.) when no
    /// policy exists yet.
    fn serialize_full_policy_with_mica_json(
        &self,
        token_id: &ManagedBuffer,
        white_paper_cid: Option<&ManagedBuffer>,
        registration_status: Option<&ManagedBuffer>,
    ) -> ManagedBuffer {
        let policy = if !self.token_policy(token_id).is_empty() {
            self.token_policy(token_id).get()
        } else {
            DrwaTokenPolicy {
                drwa_enabled: false,
                global_pause: false,
                strict_auditor_mode: false,
                metadata_protection_enabled: false,
                token_policy_version: 0,
                allowed_investor_classes: ManagedVec::new(),
                allowed_jurisdictions: ManagedVec::new(),
            }
        };
        let resolved_white_paper_cid = match white_paper_cid {
            Some(cid) => Some(cid.clone()),
            None if !self.white_paper_cid(token_id).is_empty() => {
                Some(self.white_paper_cid(token_id).get())
            }
            None => None,
        };
        let resolved_registration_status = match registration_status {
            Some(status) => Some(status.clone()),
            None if !self.registration_status(token_id).is_empty() => {
                Some(self.registration_status(token_id).get())
            }
            None => None,
        };

        // Strip the closing brace from the base policy JSON so MiCA fields
        // can be appended before re-closing. Validate the last byte IS `}`
        // to guard against future changes to serialize_policy_json that could
        // silently corrupt the sync payload.
        let full = self.serialize_policy_json(&policy);
        let mut body = ManagedBuffer::new();
        full.with_buffer_contents(|full_slice| {
            require!(
                !full_slice.is_empty() && full_slice[full_slice.len() - 1] == b'}',
                "INVARIANT_VIOLATED: serialize_policy_json did not produce valid JSON — last byte is not '}'"
            );
            body.append_bytes(&full_slice[..full_slice.len() - 1]);
        });

        if let Some(cid) = resolved_white_paper_cid {
            body.append_bytes(b",\"white_paper_cid\":\"");
            body.append(&cid);
            body.append_bytes(b"\"");
        }
        if let Some(status) = resolved_registration_status {
            body.append_bytes(b",\"registration_status\":\"");
            body.append(&status);
            body.append_bytes(b"\"");
        }
        body.append_bytes(b"}");
        body
    }

    /// Validates that a policy key is safe to embed in the hand-built JSON
    /// payload sent to the native enforcement decoder.
    ///
    /// Accepted bytes are limited to ASCII alphanumeric, `.`, `_`, and `-`.
    fn require_json_safe_key(&self, key: &ManagedBuffer) {
        require!(!key.is_empty(), "policy key must not be empty");
        let len = key.len();
        require!(len <= 64, "policy key is too long");
        let mut bytes = [0u8; 64];
        key.load_slice(0, &mut bytes[..len]);
        for &b in &bytes[..len] {
            let is_ascii_alpha = b.is_ascii_alphabetic();
            let is_ascii_digit = b.is_ascii_digit();
            let is_safe_punct = b == b'.' || b == b'_' || b == b'-';
            require!(
                is_ascii_alpha || is_ascii_digit || is_safe_punct,
                "policy key contains unsupported character"
            );
        }
    }

    /// Validates that a policy value is safe to embed in the hand-built JSON
    /// payload sent to the native enforcement decoder.
    ///
    /// Accepted bytes are limited to ASCII alphanumeric, `.`, `_`, and `-`.
    /// Uses the same character set restriction as `require_json_safe_key`.
    fn require_json_safe_value(&self, value: &ManagedBuffer) {
        require!(!value.is_empty(), "policy value must not be empty");
        let len = value.len();
        require!(len <= 64, "policy value is too long");
        let mut bytes = [0u8; 64];
        value.load_slice(0, &mut bytes[..len]);
        for &b in &bytes[..len] {
            let is_ascii_alpha = b.is_ascii_alphabetic();
            let is_ascii_digit = b.is_ascii_digit();
            let is_safe_punct = b == b'.' || b == b'_' || b == b'-';
            require!(
                is_ascii_alpha || is_ascii_digit || is_safe_punct,
                "policy value contains unsupported character"
            );
        }
    }

    /// Serializes the JSON policy body expected by the native mirror.
    ///
    /// SECURITY: JSON is constructed by concatenation with validated keys.
    /// `require_json_safe_key` restricts keys to `[a-zA-Z0-9._-]` only.
    /// This approach is used because `no_std` environments lack serde_json.
    /// Do NOT extend the key character set without reviewing injection risk.
    ///
    /// The policy version is carried both in `DrwaSyncOperation.version` and in
    /// the JSON body so the native transfer gate can compare holder mirrors
    /// against the active policy version and fail closed after policy bumps.
    fn serialize_policy_json(&self, policy: &DrwaTokenPolicy<Self::Api>) -> ManagedBuffer {
        for class in policy.allowed_investor_classes.iter() {
            self.require_json_safe_key(&class);
            self.require_json_safe_value(&class);
        }
        for jur in policy.allowed_jurisdictions.iter() {
            self.require_json_safe_key(&jur);
            self.require_json_safe_value(&jur);
        }

        let mut body = ManagedBuffer::new();
        body.append_bytes(b"{\"drwa_enabled\":");
        body.append_bytes(if policy.drwa_enabled {
            b"true"
        } else {
            b"false"
        });
        body.append_bytes(b",\"global_pause\":");
        body.append_bytes(if policy.global_pause {
            b"true"
        } else {
            b"false"
        });
        body.append_bytes(b",\"strict_auditor_mode\":");
        body.append_bytes(if policy.strict_auditor_mode {
            b"true"
        } else {
            b"false"
        });
        body.append_bytes(b",\"metadata_protection_enabled\":");
        body.append_bytes(if policy.metadata_protection_enabled {
            b"true"
        } else {
            b"false"
        });
        body.append_bytes(b",\"token_policy_version\":");
        self.append_u64_decimal(&mut body, policy.token_policy_version);
        if !policy.allowed_investor_classes.is_empty() {
            body.append_bytes(b",\"allowed_investor_classes\":{");
            let mut first = true;
            for class in policy.allowed_investor_classes.iter() {
                if !first {
                    body.append_bytes(b",");
                }
                body.append_bytes(b"\"");
                body.append(&class);
                body.append_bytes(b"\":true");
                first = false;
            }
            body.append_bytes(b"}");
        }
        if !policy.allowed_jurisdictions.is_empty() {
            body.append_bytes(b",\"allowed_jurisdictions\":{");
            let mut first = true;
            for jur in policy.allowed_jurisdictions.iter() {
                if !first {
                    body.append_bytes(b",");
                }
                body.append_bytes(b"\"");
                body.append(&jur);
                body.append_bytes(b"\":true");
                first = false;
            }
            body.append_bytes(b"}");
        }
        body.append_bytes(b"}");
        body
    }

    fn append_u64_decimal(&self, body: &mut ManagedBuffer, value: u64) {
        let mut digits = [0u8; 20];
        let mut pos = 20usize;
        let mut val = value;
        if val == 0 {
            pos -= 1;
            digits[pos] = b'0';
        } else {
            while val > 0 {
                pos -= 1;
                digits[pos] = b'0' + (val % 10) as u8;
                val /= 10;
            }
        }
        body.append_bytes(&digits[pos..20]);
    }

    /// Validates the token identifier format accepted by this contract.
    fn require_valid_token_id(&self, token_id: &ManagedBuffer) {
        require_valid_token_id(token_id);
    }

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    /// Upgrades storage layout version if needed and preserves existing state.
    #[upgrade]
    fn upgrade(&self) {
        let current = self.storage_version().get();
        if current < 1u32 {
            self.storage_version().set(1u32);
        }
    }
}
