#![no_std]

pub mod governance_proxy;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use mrv_common::MrvReportProof;
use mrv_common::resolve_storage_version_upgrade;

const MAX_VERIFIER_ADJUSTMENTS_PER_PERIOD: u64 = 5;
const METHODOLOGY_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_methodology_record_v1";
const PROJECT_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_project_record_v1";
const EVIDENCE_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_evidence_record_v1";
const VERIFICATION_CASE_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_verification_case_record_v1";
const ISSUANCE_LOT_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_issuance_lot_record_v1";
const REPORT_CANONICAL_ID_DOMAIN: &[u8] = b"mrv_report_proof_v1";
const MAX_EXECUTION_BUNDLE_CID_LEN: usize = 256;

/// Versioned methodology record with approval lifecycle and supersession tracking.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct MethodologyRecord<M: ManagedTypeApi> {
    pub methodology_id: ManagedBuffer<M>,
    pub version_label: ManagedBuffer<M>,
    pub pack_digest: ManagedBuffer<M>,
    pub approval_status: ManagedBuffer<M>,
    pub effective_from: u64,
    pub effective_to: u64,
    pub superseded_by: ManagedBuffer<M>,
}

/// MRV project record linking a tenant, asset, reporting period, and methodology.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct ProjectRecord<M: ManagedTypeApi> {
    pub project_id: ManagedBuffer<M>,
    pub tenant_id: ManagedBuffer<M>,
    pub asset_id: ManagedBuffer<M>,
    pub reporting_period_id: ManagedBuffer<M>,
    pub methodology_id: ManagedBuffer<M>,
    pub methodology_version_label: ManagedBuffer<M>,
    pub status: ManagedBuffer<M>,
}

/// Content-addressed evidence record anchored to an entity (project, farm, season).
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct EvidenceRecord<M: ManagedTypeApi> {
    pub evidence_id: ManagedBuffer<M>,
    pub entity_type: ManagedBuffer<M>,
    pub entity_id: ManagedBuffer<M>,
    pub evidence_hash: ManagedBuffer<M>,
    pub manifest_hash: ManagedBuffer<M>,
    pub submitted_at: u64,
}

/// Verification case tracking VVB assignment, status transitions, and attestation.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct VerificationCaseRecord<M: ManagedTypeApi> {
    pub case_id: ManagedBuffer<M>,
    pub target_type: ManagedBuffer<M>,
    pub target_id: ManagedBuffer<M>,
    pub status: ManagedBuffer<M>,
    pub assignee: ManagedAddress<M>,
    pub verifier_statement_hash: ManagedBuffer<M>,
    pub verifier_attestation_ref: ManagedBuffer<M>,
    pub updated_at: u64,
}

/// Issuance lot record following a `minted -> retired | reversed` lifecycle.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct IssuanceLotRecord<M: ManagedTypeApi> {
    pub lot_id: ManagedBuffer<M>,
    pub project_id: ManagedBuffer<M>,
    pub verification_case_id: ManagedBuffer<M>,
    pub vintage: u64,
    pub quantity_scaled: BigUint<M>,
    pub status: ManagedBuffer<M>,
    pub replacement_for_lot_id: ManagedBuffer<M>,
    pub reversed_amount_scaled: BigUint<M>,
}

/// Event payload for the legacy `mrvReportAnchored` event.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct MrvReportAnchoredEventPayload<M: ManagedTypeApi> {
    pub report_hash: ManagedBuffer<M>,
    pub hash_algo: ManagedBuffer<M>,
    pub canonicalization: ManagedBuffer<M>,
    pub methodology_version: u64,
    pub anchored_at: u64,
}

/// Event payload for `mrvReportAnchoredV2` including project ID and evidence manifest.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct MrvReportAnchoredV2EventPayload<M: ManagedTypeApi> {
    pub report_hash: ManagedBuffer<M>,
    pub hash_algo: ManagedBuffer<M>,
    pub canonicalization: ManagedBuffer<M>,
    pub methodology_version: u64,
    pub anchored_at: u64,
    pub public_project_id: ManagedBuffer<M>,
    pub evidence_manifest_hash: ManagedBuffer<M>,
}

/// Event payload for `mrvReportAmendedV2`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct MrvReportAmendedV2EventPayload<M: ManagedTypeApi> {
    pub report_hash: ManagedBuffer<M>,
    pub hash_algo: ManagedBuffer<M>,
    pub canonicalization: ManagedBuffer<M>,
    pub methodology_version: u64,
    pub anchored_at: u64,
    pub public_project_id: ManagedBuffer<M>,
    pub evidence_manifest_hash: ManagedBuffer<M>,
}

/// Event payload for `mrvMethodologyRegistered`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct MethodologyRegisteredEventPayload<M: ManagedTypeApi> {
    pub pack_digest: ManagedBuffer<M>,
    pub approval_status: ManagedBuffer<M>,
    pub effective_from: u64,
    pub effective_to: u64,
}

/// Event payload for `mrvMethodologySuperseded`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct MethodologySupersededEventPayload<M: ManagedTypeApi> {
    pub replacement_version_label: ManagedBuffer<M>,
    pub effective_to: u64,
}

/// Event payload for `mrvProjectRegistered`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct ProjectRegisteredEventPayload<M: ManagedTypeApi> {
    pub asset_id: ManagedBuffer<M>,
    pub reporting_period_id: ManagedBuffer<M>,
    pub methodology_id: ManagedBuffer<M>,
    pub methodology_version_label: ManagedBuffer<M>,
    pub status: ManagedBuffer<M>,
}

/// Event payload for `mrvEvidenceRegistered`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct EvidenceRegisteredEventPayload<M: ManagedTypeApi> {
    pub evidence_hash: ManagedBuffer<M>,
    pub manifest_hash: ManagedBuffer<M>,
    pub submitted_at: u64,
}

/// Event payload for `mrvVerificationCaseUpdated`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct VerificationCaseUpdatedEventPayload<M: ManagedTypeApi> {
    pub status: ManagedBuffer<M>,
    pub assignee: ManagedAddress<M>,
    pub verifier_statement_hash: ManagedBuffer<M>,
    pub verifier_attestation_ref: ManagedBuffer<M>,
    pub updated_at: u64,
}

/// Event payload for `mrvIssuanceLotCreated`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct IssuanceLotCreatedEventPayload<M: ManagedTypeApi> {
    pub vintage: u64,
    pub quantity_scaled: BigUint<M>,
    pub replacement_for_lot_id: ManagedBuffer<M>,
}

/// Event payload for `mrvIssuanceLotReversed`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct IssuanceLotReversedEventPayload<M: ManagedTypeApi> {
    pub reversed_amount_scaled: BigUint<M>,
    pub replacement_lot_id: ManagedBuffer<M>,
}

/// Event payload emitted whenever the contract derives a canonical
/// on-chain fingerprint for an externally supplied MRV identifier.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct IdentityCanonicalizedEventPayload<M: ManagedTypeApi> {
    pub canonical_id: ManagedBuffer<M>,
    pub writer: ManagedAddress<M>,
}

/// Execution bundle committed for a PAI monitoring period.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct ExecutionBundleRecord<M: ManagedTypeApi> {
    pub pai_id: ManagedBuffer<M>,
    pub monitoring_period_n: u64,
    pub bundle_cid: ManagedBuffer<M>,
    pub bundle_hash: ManagedBuffer<M>,
    pub committed_at: u64,
}

/// Verification statement submitted for a PAI monitoring period.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct VerificationStatementRecord<M: ManagedTypeApi> {
    pub pai_id: ManagedBuffer<M>,
    pub monitoring_period_n: u64,
    pub vvb_did: ManagedAddress<M>,
    pub statement_cid: ManagedBuffer<M>,
    pub outcome: ManagedBuffer<M>,
    pub submitted_at: u64,
}

/// Post-verification adjustment submitted after the initial statement.
#[type_abi]
#[derive(
    TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq,
)]
pub struct VerifierAdjustmentRecord<M: ManagedTypeApi> {
    pub pai_id: ManagedBuffer<M>,
    pub monitoring_period_n: u64,
    pub adjustment_cid: ManagedBuffer<M>,
    pub sequence: u64,
    pub submitted_at: u64,
}

/// On-chain MRV registry contract.
///
/// Anchors report proofs, methodology records, project records, evidence,
/// verification cases, issuance lots, execution bundles, and verification
/// statements. All mutating endpoints require governance or owner access.
#[multiversx_sc::contract]
pub trait MrvRegistry: mrv_common::MrvGovernanceModule {
    #[init]
    fn init(&self, governance: ManagedAddress) {
        require!(!governance.is_zero(), "governance must not be zero");
        self.governance().set(&governance);
        self.storage_version().set(1u32);
    }

    #[endpoint(setGovernanceReadAddress)]
    fn set_governance_read_address(&self, governance_read_address: ManagedAddress) {
        self.require_governance_or_owner();
        require!(
            !governance_read_address.is_zero(),
            "governance_read_address must not be zero"
        );
        self.governance_read_address().set(&governance_read_address);
        self.governance_read_address_updated_event(&governance_read_address);
    }

    #[endpoint(clearGovernanceReadAddress)]
    fn clear_governance_read_address(&self) {
        self.require_governance_or_owner();
        self.governance_read_address().clear();
        self.governance_read_address_cleared_event();
    }

    /// Configures the carbon-credit contract as the sole authority allowed to
    /// drive terminal issuance-lot lifecycle states.
    #[endpoint(setCarbonCreditLifecycleAddress)]
    fn set_carbon_credit_lifecycle_address(&self, carbon_credit_address: ManagedAddress) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(
            !carbon_credit_address.is_zero(),
            "carbon_credit_address must not be zero"
        );
        self.carbon_credit_lifecycle_address()
            .set(&carbon_credit_address);
        self.carbon_credit_lifecycle_address_updated_event(&carbon_credit_address);
    }

    #[endpoint(clearCarbonCreditLifecycleAddress)]
    fn clear_carbon_credit_lifecycle_address(&self) {
        self.require_governance_or_owner();
        self.require_not_paused();
        self.carbon_credit_lifecycle_address().clear();
        self.carbon_credit_lifecycle_address_cleared_event();
    }

    /// Registers a methodology version. Idempotent when the existing record
    /// matches; reverts on conflicting fields.
    #[endpoint(registerMethodology)]
    fn register_methodology(
        &self,
        methodology_id: ManagedBuffer,
        version_label: ManagedBuffer,
        pack_digest: ManagedBuffer,
        approval_status: ManagedBuffer,
        effective_from: u64,
        effective_to: u64,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();

        require!(!methodology_id.is_empty(), "empty methodology id");
        require!(!version_label.is_empty(), "empty version label");
        require!(!pack_digest.is_empty(), "empty pack digest");
        require!(!approval_status.is_empty(), "empty approval status");
        require!(
            self.is_valid_methodology_status(&approval_status),
            "invalid methodology approval status"
        );
        require!(effective_from > 0, "invalid effective from");
        require!(
            effective_to == 0 || effective_to >= effective_from,
            "invalid effective window"
        );

        let key = (methodology_id.clone(), version_label.clone());
        let existing = self.methodology_records().get(&key);
        if let Some(record) = existing {
            require!(
                record.pack_digest == pack_digest
                    && record.approval_status == approval_status
                    && record.effective_from == effective_from
                    && record.effective_to == effective_to,
                "conflicting methodology record"
            );
            return;
        }

        let record = MethodologyRecord {
            methodology_id: methodology_id.clone(),
            version_label: version_label.clone(),
            pack_digest: pack_digest.clone(),
            approval_status: approval_status.clone(),
            effective_from,
            effective_to,
            superseded_by: ManagedBuffer::new(),
        };
        self.methodology_records().insert(key, record);
        self.mrv_methodology_registered_event(
            &methodology_id,
            &version_label,
            &MethodologyRegisteredEventPayload {
                pack_digest: pack_digest.clone(),
                approval_status,
                effective_from,
                effective_to,
            },
        );
        self.mrv_identity_canonicalized_event(
            &ManagedBuffer::from(METHODOLOGY_CANONICAL_ID_DOMAIN),
            &methodology_id,
            &IdentityCanonicalizedEventPayload {
                canonical_id: self.derive_methodology_canonical_id(
                    &methodology_id,
                    &version_label,
                    &pack_digest,
                ),
                writer: self.blockchain().get_caller(),
            },
        );
    }

    /// Updates the approval status of an existing methodology version.
    #[endpoint(setMethodologyApprovalStatus)]
    fn set_methodology_approval_status(
        &self,
        methodology_id: ManagedBuffer,
        version_label: ManagedBuffer,
        approval_status: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!methodology_id.is_empty(), "empty methodology id");
        require!(!version_label.is_empty(), "empty version label");
        require!(!approval_status.is_empty(), "empty approval status");
        require!(
            self.is_valid_methodology_status(&approval_status),
            "invalid methodology approval status"
        );

        let key = (methodology_id.clone(), version_label.clone());
        require!(
            self.methodology_records().contains_key(&key),
            "ENTITY_NOT_FOUND: methodology_record"
        );
        let mut record = self.methodology_records().get(&key).unwrap();
        require!(
            self.is_valid_methodology_transition(&record.approval_status, &approval_status),
            "invalid methodology transition"
        );
        record.approval_status = approval_status.clone();
        self.methodology_records().insert(key, record);
        self.mrv_methodology_status_changed_event(
            &methodology_id,
            &version_label,
            &approval_status,
        );
    }

    /// Marks a methodology version as superseded and sets the replacement version.
    #[endpoint(supersedeMethodology)]
    fn supersede_methodology(
        &self,
        methodology_id: ManagedBuffer,
        version_label: ManagedBuffer,
        replacement_version_label: ManagedBuffer,
        effective_to: u64,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!methodology_id.is_empty(), "empty methodology id");
        require!(!version_label.is_empty(), "empty version label");
        require!(
            !replacement_version_label.is_empty(),
            "empty replacement version label"
        );
        require!(effective_to > 0, "invalid supersession effective to");

        let key = (methodology_id.clone(), version_label.clone());
        let replacement_key = (methodology_id.clone(), replacement_version_label.clone());
        require!(
            self.methodology_records().contains_key(&key),
            "ENTITY_NOT_FOUND: methodology_record"
        );
        require!(
            self.methodology_records().contains_key(&replacement_key),
            "ENTITY_NOT_FOUND: replacement_methodology_record"
        );
        let mut record = self.methodology_records().get(&key).unwrap();
        require!(
            record.approval_status == ManagedBuffer::from(b"approved_internal"),
            "methodology must be approved_internal before supersession"
        );
        let replacement = self.methodology_records().get(&replacement_key).unwrap();
        require!(
            replacement.approval_status == ManagedBuffer::from(b"approved_internal"),
            "replacement methodology must be approved_internal"
        );
        record.effective_to = effective_to;
        record.superseded_by = replacement_version_label.clone();
        record.approval_status = ManagedBuffer::from(b"superseded");
        self.methodology_records().insert(key, record);
        self.mrv_methodology_superseded_event(
            &methodology_id,
            &version_label,
            &MethodologySupersededEventPayload {
                replacement_version_label,
                effective_to,
            },
        );
    }

    /// Registers a project record. Idempotent when identity fields match;
    /// reverts on conflicting fields.
    #[endpoint(registerProject)]
    fn register_project(
        &self,
        project_id: ManagedBuffer,
        tenant_id: ManagedBuffer,
        asset_id: ManagedBuffer,
        reporting_period_id: ManagedBuffer,
        methodology_id: ManagedBuffer,
        methodology_version_label: ManagedBuffer,
        status: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!project_id.is_empty(), "empty project id");
        require!(!tenant_id.is_empty(), "empty tenant id");
        require!(!asset_id.is_empty(), "empty asset id");
        require!(!reporting_period_id.is_empty(), "empty reporting period id");
        require!(!methodology_id.is_empty(), "empty methodology id");
        require!(
            !methodology_version_label.is_empty(),
            "empty methodology version label"
        );
        require!(!status.is_empty(), "empty project status");
        require!(
            self.is_valid_project_status(&status),
            "invalid project status"
        );
        self.require_approved_methodology_record(&methodology_id, &methodology_version_label);

        let record = ProjectRecord {
            project_id: project_id.clone(),
            tenant_id: tenant_id.clone(),
            asset_id: asset_id.clone(),
            reporting_period_id: reporting_period_id.clone(),
            methodology_id: methodology_id.clone(),
            methodology_version_label: methodology_version_label.clone(),
            status: status.clone(),
        };
        let existing = self.project_records().get(&project_id);
        if let Some(current) = existing {
            require!(
                current.tenant_id == record.tenant_id
                    && current.asset_id == record.asset_id
                    && current.reporting_period_id == record.reporting_period_id
                    && current.methodology_id == record.methodology_id,
                "conflicting project record"
            );
            require!(
                current.methodology_version_label == record.methodology_version_label,
                "conflicting project record"
            );
            return;
        }

        self.project_records().insert(project_id.clone(), record);
        self.mrv_project_registered_event(
            &project_id,
            &tenant_id,
            &ProjectRegisteredEventPayload {
                asset_id: asset_id.clone(),
                reporting_period_id: reporting_period_id.clone(),
                methodology_id: methodology_id.clone(),
                methodology_version_label: methodology_version_label.clone(),
                status,
            },
        );
        self.mrv_identity_canonicalized_event(
            &ManagedBuffer::from(PROJECT_CANONICAL_ID_DOMAIN),
            &project_id,
            &IdentityCanonicalizedEventPayload {
                canonical_id: self.derive_project_canonical_id(
                    &project_id,
                    &tenant_id,
                    &asset_id,
                    &reporting_period_id,
                    &methodology_id,
                    &methodology_version_label,
                ),
                writer: self.blockchain().get_caller(),
            },
        );
    }

    /// Updates the status of an existing project record.
    #[endpoint(setProjectStatus)]
    fn set_project_status(&self, project_id: ManagedBuffer, status: ManagedBuffer) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!project_id.is_empty(), "empty project id");
        require!(!status.is_empty(), "empty project status");
        require!(
            self.is_valid_project_status(&status),
            "invalid project status"
        );

        require!(
            self.project_records().contains_key(&project_id),
            "ENTITY_NOT_FOUND: project_record"
        );
        let mut record = self.project_records().get(&project_id).unwrap();
        require!(
            self.is_valid_project_transition(&record.status, &status),
            "invalid project status transition"
        );
        record.status = status.clone();
        self.project_records().insert(project_id.clone(), record);
        self.mrv_project_status_changed_event(&project_id, &status);
    }

    /// Registers an evidence record. Idempotent when all fields match;
    /// reverts on conflicting records.
    #[endpoint(registerEvidence)]
    fn register_evidence(
        &self,
        evidence_id: ManagedBuffer,
        entity_type: ManagedBuffer,
        entity_id: ManagedBuffer,
        evidence_hash: ManagedBuffer,
        manifest_hash: ManagedBuffer,
        submitted_at: u64,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!evidence_id.is_empty(), "empty evidence id");
        require!(!entity_type.is_empty(), "empty entity type");
        require!(!entity_id.is_empty(), "empty entity id");
        require!(!evidence_hash.is_empty(), "empty evidence hash");
        require!(!manifest_hash.is_empty(), "empty manifest hash");
        require!(submitted_at > 0u64, "submitted_at must be positive");

        let record = EvidenceRecord {
            evidence_id: evidence_id.clone(),
            entity_type: entity_type.clone(),
            entity_id: entity_id.clone(),
            evidence_hash: evidence_hash.clone(),
            manifest_hash: manifest_hash.clone(),
            submitted_at,
        };
        let existing = self.evidence_records().get(&evidence_id);
        if let Some(current) = existing {
            require!(current == record, "conflicting evidence record");
            return;
        }

        self.evidence_records().insert(evidence_id.clone(), record);
        self.mrv_evidence_registered_event(
            &evidence_id,
            &entity_type,
            &entity_id,
            &EvidenceRegisteredEventPayload {
                evidence_hash: evidence_hash.clone(),
                manifest_hash: manifest_hash.clone(),
                submitted_at,
            },
        );
        self.mrv_identity_canonicalized_event(
            &ManagedBuffer::from(EVIDENCE_CANONICAL_ID_DOMAIN),
            &evidence_id,
            &IdentityCanonicalizedEventPayload {
                canonical_id: self.derive_evidence_canonical_id(
                    &evidence_id,
                    &entity_type,
                    &entity_id,
                    &evidence_hash,
                    &manifest_hash,
                ),
                writer: self.blockchain().get_caller(),
            },
        );
    }

    /// Creates a new verification case in `pending_assignment` status.
    #[endpoint(createVerificationCase)]
    fn create_verification_case(
        &self,
        case_id: ManagedBuffer,
        target_type: ManagedBuffer,
        target_id: ManagedBuffer,
        assignee: ManagedAddress,
        _updated_at: u64,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!case_id.is_empty(), "empty case id");
        require!(!target_type.is_empty(), "empty target type");
        require!(!target_id.is_empty(), "empty target id");
        require!(!assignee.is_zero(), "empty assignee");
        let updated_at = self
            .blockchain()
            .get_block_timestamp_seconds()
            .as_u64_seconds();
        require!(
            !self.verification_cases().contains_key(&case_id),
            "verification case already exists"
        );

        let record = VerificationCaseRecord {
            case_id: case_id.clone(),
            target_type: target_type.clone(),
            target_id: target_id.clone(),
            status: ManagedBuffer::from(b"pending_assignment"),
            assignee,
            verifier_statement_hash: ManagedBuffer::new(),
            verifier_attestation_ref: ManagedBuffer::new(),
            updated_at,
        };
        self.verification_cases().insert(case_id.clone(), record);
        self.verification_case_version(&case_id).set(1u64);
        self.mrv_verification_case_created_event(&case_id, &target_type, &target_id);
        self.mrv_identity_canonicalized_event(
            &ManagedBuffer::from(VERIFICATION_CASE_CANONICAL_ID_DOMAIN),
            &case_id,
            &IdentityCanonicalizedEventPayload {
                canonical_id: self.derive_verification_case_canonical_id(
                    &case_id,
                    &target_type,
                    &target_id,
                ),
                writer: self.blockchain().get_caller(),
            },
        );
    }

    /// Updates a verification case. Enforces a valid state-machine transition
    /// on the `status` field.
    #[endpoint(updateVerificationCase)]
    fn update_verification_case(
        &self,
        case_id: ManagedBuffer,
        status: ManagedBuffer,
        assignee: ManagedAddress,
        verifier_statement_hash: ManagedBuffer,
        verifier_attestation_ref: ManagedBuffer,
        _updated_at: u64,
    ) {
        self.require_not_paused();
        require!(!case_id.is_empty(), "empty case id");
        require!(!status.is_empty(), "empty verification status");
        require!(!assignee.is_zero(), "empty assignee");
        let caller = self.blockchain().get_caller();
        let updated_at = self
            .blockchain()
            .get_block_timestamp_seconds()
            .as_u64_seconds();

        require!(
            self.verification_cases().contains_key(&case_id),
            "ENTITY_NOT_FOUND: verification_case"
        );
        let mut record = self.verification_cases().get(&case_id).unwrap();
        require!(
            self.is_valid_verification_transition(&record.status, &status),
            "invalid verification transition"
        );
        if status == ManagedBuffer::from(b"approved") {
            require!(
                caller == assignee,
                "VVB_CALLER_MISMATCH: approved verification must be submitted by assignee"
            );
            require!(
                self.is_vvb_accredited_via_governance_or_local(assignee.clone()),
                "VVB_NOT_ACCREDITED: assignee must be accredited via governance or local registry"
            );
            require!(
                !verifier_statement_hash.is_empty(),
                "approved verification requires verifier statement hash"
            );
            require!(
                !verifier_attestation_ref.is_empty(),
                "approved verification requires verifier attestation ref"
            );
        } else {
            self.require_governance_or_owner();
        }

        record.status = status.clone();
        record.assignee = assignee.clone();
        record.verifier_statement_hash = verifier_statement_hash.clone();
        record.verifier_attestation_ref = verifier_attestation_ref.clone();
        record.updated_at = updated_at;
        self.verification_cases().insert(case_id.clone(), record);
        let next_ver = self.verification_case_version(&case_id).get() + 1;
        self.verification_case_version(&case_id).set(next_ver);
        self.mrv_verification_case_updated_event(
            &case_id,
            &VerificationCaseUpdatedEventPayload {
                status,
                assignee,
                verifier_statement_hash,
                verifier_attestation_ref,
                updated_at,
            },
        );
    }

    /// Creates an issuance lot in `minted` status. Idempotent when all fields match.
    ///
    /// B-01 (AUD-001) invariants enforced at write time:
    ///  - `project_id` MUST reference a registered project record.
    ///  - `verification_case_id` MUST reference an existing verification
    ///    case AND that case MUST be in status `approved`.
    ///  - `replacement_for_lot_id`, when non-empty, MUST reference an
    ///    existing issuance lot.
    ///  - `quantity_scaled` is canonical fixed-scale quantity using
    ///    4 decimal places (`10^-4` units).
    #[endpoint(createIssuanceLot)]
    fn create_issuance_lot(
        &self,
        lot_id: ManagedBuffer,
        project_id: ManagedBuffer,
        verification_case_id: ManagedBuffer,
        vintage: u64,
        quantity_scaled: BigUint,
        replacement_for_lot_id: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!lot_id.is_empty(), "empty lot id");
        require!(!project_id.is_empty(), "empty project id");
        require!(
            !verification_case_id.is_empty(),
            "empty verification case id"
        );
        require!(vintage > 0, "invalid vintage");
        require!(quantity_scaled > 0u64, "quantity must be positive");

        require!(
            self.project_records().contains_key(&project_id),
            "ENTITY_NOT_FOUND: project"
        );
        require!(
            self.verification_cases()
                .contains_key(&verification_case_id),
            "ENTITY_NOT_FOUND: verification_case"
        );
        let verification_case = self
            .verification_cases()
            .get(&verification_case_id)
            .unwrap();
        require!(
            verification_case.status == b"approved",
            "VERIFICATION_CASE_NOT_APPROVED: issuance requires an approved verification case"
        );
        require!(
            self.is_vvb_accredited_via_governance_or_local(verification_case.assignee.clone()),
            "VVB_NOT_ACCREDITED: approved verification case assignee must be accredited"
        );
        require!(
            !verification_case.verifier_statement_hash.is_empty(),
            "approved verification case missing verifier statement hash"
        );
        require!(
            !verification_case.verifier_attestation_ref.is_empty(),
            "approved verification case missing verifier attestation ref"
        );
        if !replacement_for_lot_id.is_empty() {
            require!(
                self.issuance_lots().contains_key(&replacement_for_lot_id),
                "ENTITY_NOT_FOUND: replacement issuance_lot"
            );
        }

        let record = IssuanceLotRecord {
            lot_id: lot_id.clone(),
            project_id: project_id.clone(),
            verification_case_id: verification_case_id.clone(),
            vintage,
            quantity_scaled: quantity_scaled.clone(),
            status: ManagedBuffer::from(b"minted"),
            replacement_for_lot_id: replacement_for_lot_id.clone(),
            reversed_amount_scaled: BigUint::zero(),
        };

        let existing = self.issuance_lots().get(&lot_id);
        if let Some(current) = existing {
            require!(
                current.status != b"reversed",
                "REVERSED_LOT_CANNOT_BE_REINSERTED: lot was reversed and cannot be re-created"
            );
            require!(current == record, "conflicting issuance lot");
            return;
        }

        self.issuance_lots().insert(lot_id.clone(), record);
        self.mrv_issuance_lot_created_event(
            &lot_id,
            &project_id,
            &verification_case_id,
            &IssuanceLotCreatedEventPayload {
                vintage,
                quantity_scaled: quantity_scaled.clone(),
                replacement_for_lot_id: replacement_for_lot_id.clone(),
            },
        );
        self.mrv_identity_canonicalized_event(
            &ManagedBuffer::from(ISSUANCE_LOT_CANONICAL_ID_DOMAIN),
            &lot_id,
            &IdentityCanonicalizedEventPayload {
                canonical_id: self.derive_issuance_lot_canonical_id(
                    &lot_id,
                    &project_id,
                    &verification_case_id,
                    vintage,
                    &quantity_scaled,
                    &replacement_for_lot_id,
                ),
                writer: self.blockchain().get_caller(),
            },
        );
    }

    /// Transitions a minted issuance lot to `retired` status.
    #[endpoint(retireIssuanceLot)]
    fn retire_issuance_lot(&self, lot_id: ManagedBuffer) {
        self.require_terminal_lifecycle_authority();
        self.require_not_paused();
        require!(!lot_id.is_empty(), "empty lot id");

        require!(
            self.issuance_lots().contains_key(&lot_id),
            "ENTITY_NOT_FOUND: issuance_lot"
        );
        let mut record = self.issuance_lots().get(&lot_id).unwrap();
        require!(
            record.status == b"minted",
            "lot not eligible for retirement"
        );
        record.status = ManagedBuffer::from(b"retired");
        self.issuance_lots().insert(lot_id.clone(), record);
        self.mrv_issuance_lot_retired_event(&lot_id);
    }

    /// Reverses a minted or retired issuance lot and records the reversed amount.
    ///
    /// B-01 (AUD-001) replacement-lineage invariant: when a
    /// `replacement_lot_id` is supplied, the referenced lot MUST exist
    /// AND its `replacement_for_lot_id` MUST point back at this lot.
    /// This forward/back pointer pair prevents fabricated lineage where
    /// a reversal cites a non-existent or unrelated replacement. Pass
    /// an empty `replacement_lot_id` for reversals without a designated
    /// replacement.
    #[endpoint(reverseIssuanceLot)]
    fn reverse_issuance_lot(
        &self,
        lot_id: ManagedBuffer,
        reversed_amount_scaled: BigUint,
        replacement_lot_id: ManagedBuffer,
    ) {
        self.require_terminal_lifecycle_authority();
        self.require_not_paused();
        require!(!lot_id.is_empty(), "empty lot id");
        require!(
            reversed_amount_scaled > 0u64,
            "reversed amount must be positive"
        );

        require!(
            self.issuance_lots().contains_key(&lot_id),
            "ENTITY_NOT_FOUND: issuance_lot"
        );
        let mut record = self.issuance_lots().get(&lot_id).unwrap();
        require!(
            record.status == b"minted" || record.status == b"retired",
            "lot not eligible for reversal"
        );
        require!(
            reversed_amount_scaled <= record.quantity_scaled,
            "reversed amount exceeds issuance lot quantity"
        );

        if !replacement_lot_id.is_empty() {
            require!(
                self.issuance_lots().contains_key(&replacement_lot_id),
                "ENTITY_NOT_FOUND: replacement_lot"
            );
            let replacement_record = self.issuance_lots().get(&replacement_lot_id).unwrap();
            require!(
                replacement_record.replacement_for_lot_id == lot_id,
                "REPLACEMENT_LINEAGE_MISMATCH: replacement lot does not cite this lot as its predecessor"
            );
        }

        record.status = ManagedBuffer::from(b"reversed");
        record.reversed_amount_scaled = reversed_amount_scaled.clone();
        self.issuance_lots().insert(lot_id.clone(), record);
        self.mrv_issuance_lot_reversed_event(
            &lot_id,
            &IssuanceLotReversedEventPayload {
                reversed_amount_scaled,
                replacement_lot_id,
            },
        );
    }

    /// Anchors a report proof together with its evidence manifest hash.
    ///
    /// `anchorReportV2` replaces the removed `anchorReport` entrypoint and
    /// binds the initial proof to the `(tenant, farm, season)` tuple. Once a
    /// season is bound, subsequent updates for that season must use
    /// `amendReportV2`.
    #[endpoint(anchorReportV2)]
    fn anchor_report_v2(
        &self,
        report_id: ManagedBuffer,
        public_tenant_id: ManagedBuffer,
        public_farm_id: ManagedBuffer,
        public_season_id: ManagedBuffer,
        public_project_id: ManagedBuffer,
        report_hash: ManagedBuffer,
        hash_algo: ManagedBuffer,
        canonicalization: ManagedBuffer,
        methodology_version: u64,
        anchored_at: u64,
        evidence_manifest_hash: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();

        require!(!report_id.is_empty(), "empty report id");
        require!(!public_tenant_id.is_empty(), "empty public tenant id");
        require!(!public_farm_id.is_empty(), "empty public farm id");
        require!(!public_season_id.is_empty(), "empty public season id");
        require!(!public_project_id.is_empty(), "empty public project id");
        require!(!report_hash.is_empty(), "empty report hash");
        require!(!hash_algo.is_empty(), "empty hash algo");
        require!(!canonicalization.is_empty(), "empty canonicalization");
        require!(methodology_version > 0, "invalid methodology version");
        require!(anchored_at > 0, "invalid anchored at");
        require!(
            !evidence_manifest_hash.is_empty(),
            "empty evidence manifest hash"
        );

        let proof = MrvReportProof {
            report_id: report_id.clone(),
            public_tenant_id: public_tenant_id.clone(),
            public_farm_id: public_farm_id.clone(),
            public_season_id: public_season_id.clone(),
            public_project_id: public_project_id.clone(),
            report_hash: report_hash.clone(),
            hash_algo: hash_algo.clone(),
            canonicalization: canonicalization.clone(),
            methodology_version,
            anchored_at,
            evidence_manifest_hash: evidence_manifest_hash.clone(),
        };
        let season_key = (
            public_tenant_id.clone(),
            public_farm_id.clone(),
            public_season_id.clone(),
        );

        if !self.report_proofs().contains_key(&report_id) {
            require!(
                !self.proof_by_season().contains_key(&season_key),
                "SEASON_PROOF_ALREADY_EXISTS: this (tenant,farm,season) already has an anchored report — use amendReportV2 to update"
            );

            self.report_proofs()
                .insert(report_id.clone(), proof.clone());
            self.proof_by_season().insert(season_key, report_id.clone());
            self.mrv_report_anchored_v2(
                &report_id,
                &public_tenant_id,
                &public_farm_id,
                &public_season_id,
                &MrvReportAnchoredV2EventPayload {
                    report_hash: report_hash.clone(),
                    hash_algo: hash_algo.clone(),
                    canonicalization: canonicalization.clone(),
                    methodology_version,
                    anchored_at,
                    public_project_id: public_project_id.clone(),
                    evidence_manifest_hash: evidence_manifest_hash.clone(),
                },
            );
            self.mrv_identity_canonicalized_event(
                &ManagedBuffer::from(REPORT_CANONICAL_ID_DOMAIN),
                &report_id,
                &IdentityCanonicalizedEventPayload {
                    canonical_id: self.derive_report_canonical_id(
                        &report_id,
                        &public_tenant_id,
                        &public_farm_id,
                        &public_season_id,
                        &public_project_id,
                    ),
                    writer: self.blockchain().get_caller(),
                },
            );

            return;
        }

        require!(
            self.report_proofs().contains_key(&report_id),
            "ENTITY_NOT_FOUND: report_proof"
        );
        let existing = self.report_proofs().get(&report_id).unwrap();
        require!(existing == proof, "conflicting report proof");
    }

    /// Replaces an existing report proof and updates the season binding when needed.
    #[endpoint(amendReportV2)]
    fn amend_report_v2(
        &self,
        report_id: ManagedBuffer,
        public_tenant_id: ManagedBuffer,
        public_farm_id: ManagedBuffer,
        public_season_id: ManagedBuffer,
        public_project_id: ManagedBuffer,
        report_hash: ManagedBuffer,
        hash_algo: ManagedBuffer,
        canonicalization: ManagedBuffer,
        methodology_version: u64,
        anchored_at: u64,
        evidence_manifest_hash: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();

        require!(!report_id.is_empty(), "empty report id");
        require!(!public_tenant_id.is_empty(), "empty public tenant id");
        require!(!public_farm_id.is_empty(), "empty public farm id");
        require!(!public_season_id.is_empty(), "empty public season id");
        require!(!public_project_id.is_empty(), "empty public project id");
        require!(!report_hash.is_empty(), "empty report hash");
        require!(!hash_algo.is_empty(), "empty hash algo");
        require!(!canonicalization.is_empty(), "empty canonicalization");
        require!(methodology_version > 0, "invalid methodology version");
        require!(anchored_at > 0, "invalid anchored at");
        require!(
            !evidence_manifest_hash.is_empty(),
            "empty evidence manifest hash"
        );

        require!(
            self.report_proofs().contains_key(&report_id),
            "missing proof"
        );

        let amended = MrvReportProof {
            report_id: report_id.clone(),
            public_tenant_id: public_tenant_id.clone(),
            public_farm_id: public_farm_id.clone(),
            public_season_id: public_season_id.clone(),
            public_project_id: public_project_id.clone(),
            report_hash: report_hash.clone(),
            hash_algo: hash_algo.clone(),
            canonicalization: canonicalization.clone(),
            methodology_version,
            anchored_at,
            evidence_manifest_hash: evidence_manifest_hash.clone(),
        };

        require!(
            self.report_proofs().contains_key(&report_id),
            "ENTITY_NOT_FOUND: report_proof"
        );
        let existing = self.report_proofs().get(&report_id).unwrap();
        let old_season_key = (
            existing.public_tenant_id.clone(),
            existing.public_farm_id.clone(),
            existing.public_season_id.clone(),
        );
        let new_season_key = (
            public_tenant_id.clone(),
            public_farm_id.clone(),
            public_season_id.clone(),
        );

        if old_season_key != new_season_key {
            if let Some(existing_report_id) = self.proof_by_season().get(&new_season_key) {
                require!(
                    existing_report_id == report_id,
                    "season already bound to a different report"
                );
            }
            self.proof_by_season().remove(&old_season_key);
            self.proof_by_season()
                .insert(new_season_key, report_id.clone());
        }

        let history_index = self.report_proof_amendment_count(&report_id).get();
        self.report_proof_amendment_history()
            .insert((report_id.clone(), history_index), existing);
        self.report_proof_amendment_count(&report_id)
            .set(history_index + 1u64);

        self.report_proofs()
            .insert(report_id.clone(), amended.clone());
        self.mrv_report_amended_v2(
            &report_id,
            &public_tenant_id,
            &public_farm_id,
            &public_season_id,
            &MrvReportAmendedV2EventPayload {
                report_hash,
                hash_algo,
                canonicalization,
                methodology_version,
                anchored_at,
                public_project_id: public_project_id.clone(),
                evidence_manifest_hash,
            },
        );
        self.mrv_identity_canonicalized_event(
            &ManagedBuffer::from(REPORT_CANONICAL_ID_DOMAIN),
            &report_id,
            &IdentityCanonicalizedEventPayload {
                canonical_id: self.derive_report_canonical_id(
                    &report_id,
                    &public_tenant_id,
                    &public_farm_id,
                    &public_season_id,
                    &public_project_id,
                ),
                writer: self.blockchain().get_caller(),
            },
        );
    }

    #[view(getReportProof)]
    fn get_report_proof(
        &self,
        report_id: ManagedBuffer,
    ) -> OptionalValue<MrvReportProof<Self::Api>> {
        match self.report_proofs().get(&report_id) {
            Some(proof) => OptionalValue::Some(proof),
            None => OptionalValue::None,
        }
    }

    #[view(getReportProofBySeason)]
    fn get_report_proof_by_season(
        &self,
        public_tenant_id: ManagedBuffer,
        public_farm_id: ManagedBuffer,
        public_season_id: ManagedBuffer,
    ) -> OptionalValue<MrvReportProof<Self::Api>> {
        let key = (public_tenant_id, public_farm_id, public_season_id);
        let report_id = match self.proof_by_season().get(&key) {
            Some(value) => value,
            None => return OptionalValue::None,
        };

        self.get_report_proof(report_id)
    }

    #[view(getReportIdBySeason)]
    fn get_report_id_by_season(
        &self,
        public_tenant_id: ManagedBuffer,
        public_farm_id: ManagedBuffer,
        public_season_id: ManagedBuffer,
    ) -> OptionalValue<ManagedBuffer> {
        let key = (public_tenant_id, public_farm_id, public_season_id);
        match self.proof_by_season().get(&key) {
            Some(report_id) => OptionalValue::Some(report_id),
            None => OptionalValue::None,
        }
    }

    #[view(isReportAnchored)]
    fn is_report_anchored(&self, report_id: ManagedBuffer) -> bool {
        self.report_proofs().contains_key(&report_id)
    }

    #[view(getReportCanonicalId)]
    fn get_report_canonical_id(&self, report_id: ManagedBuffer) -> OptionalValue<ManagedBuffer> {
        match self.report_proofs().get(&report_id) {
            Some(proof) => OptionalValue::Some(self.derive_report_canonical_id(
                &proof.report_id,
                &proof.public_tenant_id,
                &proof.public_farm_id,
                &proof.public_season_id,
                &proof.public_project_id,
            )),
            None => OptionalValue::None,
        }
    }

    #[view(getReportProofAmendmentCount)]
    fn get_report_proof_amendment_count(&self, report_id: ManagedBuffer) -> u64 {
        self.report_proof_amendment_count(&report_id).get()
    }

    #[view(getReportProofAmendment)]
    fn get_report_proof_amendment(
        &self,
        report_id: ManagedBuffer,
        amendment_index: u64,
    ) -> OptionalValue<MrvReportProof<Self::Api>> {
        match self
            .report_proof_amendment_history()
            .get(&(report_id, amendment_index))
        {
            Some(proof) => OptionalValue::Some(proof),
            None => OptionalValue::None,
        }
    }

    #[view(getAnchoredReportsCount)]
    fn get_anchored_reports_count(&self) -> usize {
        self.report_proofs().len()
    }

    #[view(getMethodologyRecord)]
    fn get_methodology_record(
        &self,
        methodology_id: ManagedBuffer,
        version_label: ManagedBuffer,
    ) -> OptionalValue<MethodologyRecord<Self::Api>> {
        let key = (methodology_id, version_label);
        match self.methodology_records().get(&key) {
            Some(record) => OptionalValue::Some(record),
            None => OptionalValue::None,
        }
    }

    #[view(getMethodologyCanonicalId)]
    fn get_methodology_canonical_id(
        &self,
        methodology_id: ManagedBuffer,
        version_label: ManagedBuffer,
    ) -> OptionalValue<ManagedBuffer> {
        let key = (methodology_id, version_label);
        match self.methodology_records().get(&key) {
            Some(record) => OptionalValue::Some(self.derive_methodology_canonical_id(
                &record.methodology_id,
                &record.version_label,
                &record.pack_digest,
            )),
            None => OptionalValue::None,
        }
    }

    #[view(getMethodologyRecordsCount)]
    fn get_methodology_records_count(&self) -> usize {
        self.methodology_records().len()
    }

    #[view(getProjectRecord)]
    fn get_project_record(
        &self,
        project_id: ManagedBuffer,
    ) -> OptionalValue<ProjectRecord<Self::Api>> {
        match self.project_records().get(&project_id) {
            Some(record) => OptionalValue::Some(record),
            None => OptionalValue::None,
        }
    }

    #[view(getProjectCanonicalId)]
    fn get_project_canonical_id(&self, project_id: ManagedBuffer) -> OptionalValue<ManagedBuffer> {
        match self.project_records().get(&project_id) {
            Some(record) => OptionalValue::Some(self.derive_project_canonical_id(
                &record.project_id,
                &record.tenant_id,
                &record.asset_id,
                &record.reporting_period_id,
                &record.methodology_id,
                &record.methodology_version_label,
            )),
            None => OptionalValue::None,
        }
    }

    #[view(getProjectRecordsCount)]
    fn get_project_records_count(&self) -> usize {
        self.project_records().len()
    }

    #[view(getEvidenceRecord)]
    fn get_evidence_record(
        &self,
        evidence_id: ManagedBuffer,
    ) -> OptionalValue<EvidenceRecord<Self::Api>> {
        match self.evidence_records().get(&evidence_id) {
            Some(record) => OptionalValue::Some(record),
            None => OptionalValue::None,
        }
    }

    #[view(getEvidenceCanonicalId)]
    fn get_evidence_canonical_id(
        &self,
        evidence_id: ManagedBuffer,
    ) -> OptionalValue<ManagedBuffer> {
        match self.evidence_records().get(&evidence_id) {
            Some(record) => OptionalValue::Some(self.derive_evidence_canonical_id(
                &record.evidence_id,
                &record.entity_type,
                &record.entity_id,
                &record.evidence_hash,
                &record.manifest_hash,
            )),
            None => OptionalValue::None,
        }
    }

    #[view(getEvidenceRecordsCount)]
    fn get_evidence_records_count(&self) -> usize {
        self.evidence_records().len()
    }

    #[view(getVerificationCase)]
    fn get_verification_case(
        &self,
        case_id: ManagedBuffer,
    ) -> OptionalValue<VerificationCaseRecord<Self::Api>> {
        match self.verification_cases().get(&case_id) {
            Some(record) => OptionalValue::Some(record),
            None => OptionalValue::None,
        }
    }

    #[view(getVerificationCaseCanonicalId)]
    fn get_verification_case_canonical_id(
        &self,
        case_id: ManagedBuffer,
    ) -> OptionalValue<ManagedBuffer> {
        match self.verification_cases().get(&case_id) {
            Some(record) => OptionalValue::Some(self.derive_verification_case_canonical_id(
                &record.case_id,
                &record.target_type,
                &record.target_id,
            )),
            None => OptionalValue::None,
        }
    }

    #[view(getVerificationCasesCount)]
    fn get_verification_cases_count(&self) -> usize {
        self.verification_cases().len()
    }

    #[view(getIssuanceLot)]
    fn get_issuance_lot(
        &self,
        lot_id: ManagedBuffer,
    ) -> OptionalValue<IssuanceLotRecord<Self::Api>> {
        match self.issuance_lots().get(&lot_id) {
            Some(record) => OptionalValue::Some(record),
            None => OptionalValue::None,
        }
    }

    #[view(getIssuanceLotCanonicalId)]
    fn get_issuance_lot_canonical_id(&self, lot_id: ManagedBuffer) -> OptionalValue<ManagedBuffer> {
        match self.issuance_lots().get(&lot_id) {
            Some(record) => OptionalValue::Some(self.derive_issuance_lot_canonical_id(
                &record.lot_id,
                &record.project_id,
                &record.verification_case_id,
                record.vintage,
                &record.quantity_scaled,
                &record.replacement_for_lot_id,
            )),
            None => OptionalValue::None,
        }
    }

    #[view(getIssuanceLotsCount)]
    fn get_issuance_lots_count(&self) -> usize {
        self.issuance_lots().len()
    }

    /// Legacy V1 event retained for ABI backward compatibility.
    ///
    /// The contract emits the V2 report events for current report anchoring.
    #[allow(dead_code)]
    #[event("mrvReportAnchored")]
    fn mrv_report_anchored(
        &self,
        #[indexed] report_id: &ManagedBuffer,
        #[indexed] public_tenant_id: &ManagedBuffer,
        #[indexed] public_farm_id: &ManagedBuffer,
        #[indexed] public_season_id: &ManagedBuffer,
        payload: &MrvReportAnchoredEventPayload<Self::Api>,
    );

    #[event("mrvReportAnchoredV2")]
    fn mrv_report_anchored_v2(
        &self,
        #[indexed] report_id: &ManagedBuffer,
        #[indexed] public_tenant_id: &ManagedBuffer,
        #[indexed] public_farm_id: &ManagedBuffer,
        #[indexed] public_season_id: &ManagedBuffer,
        payload: &MrvReportAnchoredV2EventPayload<Self::Api>,
    );

    #[event("mrvReportAmendedV2")]
    fn mrv_report_amended_v2(
        &self,
        #[indexed] report_id: &ManagedBuffer,
        #[indexed] public_tenant_id: &ManagedBuffer,
        #[indexed] public_farm_id: &ManagedBuffer,
        #[indexed] public_season_id: &ManagedBuffer,
        payload: &MrvReportAmendedV2EventPayload<Self::Api>,
    );

    #[event("mrvMethodologyRegistered")]
    fn mrv_methodology_registered_event(
        &self,
        #[indexed] methodology_id: &ManagedBuffer,
        #[indexed] version_label: &ManagedBuffer,
        payload: &MethodologyRegisteredEventPayload<Self::Api>,
    );

    #[event("mrvMethodologyStatusChanged")]
    fn mrv_methodology_status_changed_event(
        &self,
        #[indexed] methodology_id: &ManagedBuffer,
        #[indexed] version_label: &ManagedBuffer,
        approval_status: &ManagedBuffer,
    );

    #[event("mrvMethodologySuperseded")]
    fn mrv_methodology_superseded_event(
        &self,
        #[indexed] methodology_id: &ManagedBuffer,
        #[indexed] version_label: &ManagedBuffer,
        payload: &MethodologySupersededEventPayload<Self::Api>,
    );

    #[event("mrvProjectRegistered")]
    fn mrv_project_registered_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        #[indexed] tenant_id: &ManagedBuffer,
        payload: &ProjectRegisteredEventPayload<Self::Api>,
    );

    #[event("mrvProjectStatusChanged")]
    fn mrv_project_status_changed_event(
        &self,
        #[indexed] project_id: &ManagedBuffer,
        status: &ManagedBuffer,
    );

    #[event("mrvEvidenceRegistered")]
    fn mrv_evidence_registered_event(
        &self,
        #[indexed] evidence_id: &ManagedBuffer,
        #[indexed] entity_type: &ManagedBuffer,
        #[indexed] entity_id: &ManagedBuffer,
        payload: &EvidenceRegisteredEventPayload<Self::Api>,
    );

    #[event("mrvVerificationCaseCreated")]
    fn mrv_verification_case_created_event(
        &self,
        #[indexed] case_id: &ManagedBuffer,
        #[indexed] target_type: &ManagedBuffer,
        #[indexed] target_id: &ManagedBuffer,
    );

    #[event("mrvVerificationCaseUpdated")]
    fn mrv_verification_case_updated_event(
        &self,
        #[indexed] case_id: &ManagedBuffer,
        payload: &VerificationCaseUpdatedEventPayload<Self::Api>,
    );

    #[event("mrvIssuanceLotCreated")]
    fn mrv_issuance_lot_created_event(
        &self,
        #[indexed] lot_id: &ManagedBuffer,
        #[indexed] project_id: &ManagedBuffer,
        #[indexed] verification_case_id: &ManagedBuffer,
        payload: &IssuanceLotCreatedEventPayload<Self::Api>,
    );

    #[event("mrvIssuanceLotRetired")]
    fn mrv_issuance_lot_retired_event(&self, #[indexed] lot_id: &ManagedBuffer);

    #[event("mrvIssuanceLotReversed")]
    fn mrv_issuance_lot_reversed_event(
        &self,
        #[indexed] lot_id: &ManagedBuffer,
        payload: &IssuanceLotReversedEventPayload<Self::Api>,
    );

    #[event("mrvIdentityCanonicalized")]
    fn mrv_identity_canonicalized_event(
        &self,
        #[indexed] domain: &ManagedBuffer,
        #[indexed] external_id: &ManagedBuffer,
        payload: &IdentityCanonicalizedEventPayload<Self::Api>,
    );

    #[storage_mapper("executionBundles")]
    fn execution_bundles(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer), ExecutionBundleRecord<Self::Api>>;

    #[storage_mapper("verificationStatements")]
    fn verification_statements(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer), VerificationStatementRecord<Self::Api>>;

    #[storage_mapper("verifierAdjustments")]
    fn verifier_adjustments(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer, ManagedBuffer), VerifierAdjustmentRecord<Self::Api>>;

    #[storage_mapper("verifierAdjustmentCount")]
    fn verifier_adjustment_count(&self, pai_id: &ManagedBuffer) -> MapMapper<ManagedBuffer, u64>;

    #[event("mrvExecutionBundleCommitted")]
    fn mrv_execution_bundle_committed_event(
        &self,
        #[indexed] pai_id: &ManagedBuffer,
        #[indexed] bundle_cid: &ManagedBuffer,
        #[indexed] bundle_hash: &ManagedBuffer,
    );

    #[event("mrvVerificationStatementSubmitted")]
    fn mrv_verification_statement_submitted_event(
        &self,
        #[indexed] pai_id: &ManagedBuffer,
        #[indexed] vvb_did: &ManagedAddress,
        #[indexed] statement_cid: &ManagedBuffer,
        outcome: &ManagedBuffer,
    );

    #[event("mrvVerifierAdjustmentSubmitted")]
    fn mrv_verifier_adjustment_submitted_event(
        &self,
        #[indexed] pai_id: &ManagedBuffer,
        #[indexed] adjustment_cid: &ManagedBuffer,
    );

    #[event("governanceReadAddressUpdated")]
    fn governance_read_address_updated_event(
        &self,
        #[indexed] governance_read_address: &ManagedAddress,
    );

    #[event("governanceReadAddressCleared")]
    fn governance_read_address_cleared_event(&self);

    #[event("carbonCreditLifecycleAddressUpdated")]
    fn carbon_credit_lifecycle_address_updated_event(
        &self,
        #[indexed] carbon_credit_address: &ManagedAddress,
    );

    #[event("carbonCreditLifecycleAddressCleared")]
    fn carbon_credit_lifecycle_address_cleared_event(&self);

    #[storage_mapper("reportProofs")]
    fn report_proofs(&self) -> MapMapper<ManagedBuffer, MrvReportProof<Self::Api>>;

    #[storage_mapper("reportProofAmendmentHistory")]
    fn report_proof_amendment_history(
        &self,
    ) -> MapMapper<(ManagedBuffer, u64), MrvReportProof<Self::Api>>;

    #[storage_mapper("reportProofAmendmentCount")]
    fn report_proof_amendment_count(&self, report_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[storage_mapper("proofBySeason")]
    fn proof_by_season(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer, ManagedBuffer), ManagedBuffer>;

    #[storage_mapper("methodologyRecords")]
    fn methodology_records(
        &self,
    ) -> MapMapper<(ManagedBuffer, ManagedBuffer), MethodologyRecord<Self::Api>>;

    #[storage_mapper("projectRecords")]
    fn project_records(&self) -> MapMapper<ManagedBuffer, ProjectRecord<Self::Api>>;

    #[storage_mapper("evidenceRecords")]
    fn evidence_records(&self) -> MapMapper<ManagedBuffer, EvidenceRecord<Self::Api>>;

    #[storage_mapper("verificationCases")]
    fn verification_cases(&self) -> MapMapper<ManagedBuffer, VerificationCaseRecord<Self::Api>>;

    #[storage_mapper("issuanceLots")]
    fn issuance_lots(&self) -> MapMapper<ManagedBuffer, IssuanceLotRecord<Self::Api>>;

    /// Stores accredited VVB addresses that may submit verification
    /// statements.
    #[storage_mapper("accreditedVvbs")]
    fn accredited_vvbs(&self) -> UnorderedSetMapper<ManagedAddress>;

    /// Registers an accredited VVB address for verification statement
    /// submission.
    #[endpoint(registerAccreditedVvb)]
    fn register_accredited_vvb(&self, vvb_did: ManagedAddress) {
        self.require_governance_or_owner();
        self.require_not_paused();
        self.require_local_vvb_registry_mode();
        require!(!vvb_did.is_zero(), "vvb_did must not be zero");
        self.accredited_vvbs().insert(vvb_did);
    }

    /// Removes an accredited VVB address.
    #[endpoint(deregisterAccreditedVvb)]
    fn deregister_accredited_vvb(&self, vvb_did: ManagedAddress) {
        self.require_governance_or_owner();
        self.require_not_paused();
        self.require_local_vvb_registry_mode();
        self.accredited_vvbs().swap_remove(&vvb_did);
    }

    #[view(isVvbAccredited)]
    fn is_vvb_accredited(&self, vvb_did: ManagedAddress) -> bool {
        self.is_vvb_accredited_via_governance_or_local(vvb_did)
    }

    /// Commits an execution bundle reference for a PAI monitoring period.
    #[endpoint(commitExecutionBundle)]
    fn commit_execution_bundle(
        &self,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
        bundle_cid: ManagedBuffer,
        bundle_hash: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        self.require_not_paused();
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(!bundle_cid.is_empty(), "empty bundle_cid");
        require!(
            bundle_cid.len() <= MAX_EXECUTION_BUNDLE_CID_LEN,
            "bundle_cid exceeds maximum length"
        );
        require!(
            bundle_hash.len() == 32,
            "bundle_hash must be 32 bytes (SHA-256)"
        );

        let pk = mrv_common::period_key(monitoring_period_n);
        let key = (pai_id.clone(), pk);
        require!(
            !self.execution_bundles().contains_key(&key),
            "execution bundle already committed for this PAI/period"
        );

        let record = ExecutionBundleRecord {
            pai_id: pai_id.clone(),
            monitoring_period_n,
            bundle_cid: bundle_cid.clone(),
            bundle_hash: bundle_hash.clone(),
            committed_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
        };

        self.execution_bundles().insert(key, record);
        self.mrv_execution_bundle_committed_event(&pai_id, &bundle_cid, &bundle_hash);
    }

    /// Submits the initial verification statement for a previously committed
    /// execution bundle.
    ///
    /// The first statement remains immutable. Later corrections must be
    /// recorded through `submitVerifierAdjustment`.
    #[endpoint(submitVerificationStatement)]
    fn submit_verification_statement(
        &self,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
        vvb_did: ManagedAddress,
        statement_cid: ManagedBuffer,
        outcome: ManagedBuffer,
    ) {
        self.require_not_paused();
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(!vvb_did.is_zero(), "empty vvb_did");
        require!(!statement_cid.is_empty(), "empty statement_cid");
        require!(
            outcome == b"approved"
                || outcome == b"rejected"
                || outcome == b"needs_more_information",
            "outcome must be approved, rejected, or needs_more_information"
        );

        require!(
            self.is_vvb_accredited_via_governance_or_local(vvb_did.clone()),
            "VVB_NOT_ACCREDITED: vvb_did must be accredited via governance or local registry"
        );
        require!(
            self.blockchain().get_caller() == vvb_did,
            "VVB_CALLER_MISMATCH: statement must be submitted by vvb_did"
        );

        let pk = mrv_common::period_key(monitoring_period_n);
        let bundle_key = (pai_id.clone(), pk.clone());
        require!(
            self.execution_bundles().contains_key(&bundle_key),
            "execution bundle not committed for this PAI/period"
        );

        let key = (pai_id.clone(), pk);
        require!(
            !self.verification_statements().contains_key(&key),
            "STATEMENT_ALREADY_SUBMITTED: use submitVerifierAdjustment for corrections"
        );
        let record = VerificationStatementRecord {
            pai_id: pai_id.clone(),
            monitoring_period_n,
            vvb_did: vvb_did.clone(),
            statement_cid: statement_cid.clone(),
            outcome: outcome.clone(),
            submitted_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
        };

        self.verification_statements().insert(key, record);
        self.mrv_verification_statement_submitted_event(
            &pai_id,
            &vvb_did,
            &statement_cid,
            &outcome,
        );
    }

    /// Appends a verifier adjustment after the initial statement has been
    /// submitted.
    #[endpoint(submitVerifierAdjustment)]
    fn submit_verifier_adjustment(
        &self,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
        adjustment_cid: ManagedBuffer,
    ) {
        self.require_not_paused();
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(!adjustment_cid.is_empty(), "empty adjustment_cid");

        let pk = mrv_common::period_key(monitoring_period_n);
        let stmt_key = (pai_id.clone(), pk.clone());
        require!(
            self.verification_statements().contains_key(&stmt_key),
            "verification statement not submitted for this PAI/period"
        );
        let statement = self.verification_statements().get(&stmt_key).unwrap();
        require!(
            self.blockchain().get_caller() == statement.vvb_did,
            "VVB_CALLER_MISMATCH: adjustment must be submitted by statement vvb_did"
        );

        let current: u64 = self
            .verifier_adjustment_count(&pai_id)
            .get(&pk)
            .unwrap_or(0u64);
        require!(
            current < MAX_VERIFIER_ADJUSTMENTS_PER_PERIOD,
            "verifier adjustment cap exceeded"
        );
        let next_seq: u64 = current
            .checked_add(1)
            .unwrap_or_else(|| sc_panic!("verifier adjustment overflow"));
        self.verifier_adjustment_count(&pai_id)
            .insert(pk.clone(), next_seq);

        let sk = mrv_common::period_key(next_seq);
        let adjustment_key = (pai_id.clone(), pk, sk);

        let record = VerifierAdjustmentRecord {
            pai_id: pai_id.clone(),
            monitoring_period_n,
            adjustment_cid: adjustment_cid.clone(),
            sequence: next_seq,
            submitted_at: self
                .blockchain()
                .get_block_timestamp_seconds()
                .as_u64_seconds(),
        };

        self.verifier_adjustments().insert(adjustment_key, record);
        self.mrv_verifier_adjustment_submitted_event(&pai_id, &adjustment_cid);
    }

    #[view(getExecutionBundle)]
    fn get_execution_bundle(
        &self,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
    ) -> OptionalValue<ExecutionBundleRecord<Self::Api>> {
        let pk = mrv_common::period_key(monitoring_period_n);
        match self.execution_bundles().get(&(pai_id, pk)) {
            Some(record) => OptionalValue::Some(record),
            None => OptionalValue::None,
        }
    }

    #[view(getVerificationStatement)]
    fn get_verification_statement(
        &self,
        pai_id: ManagedBuffer,
        monitoring_period_n: u64,
    ) -> OptionalValue<VerificationStatementRecord<Self::Api>> {
        let pk = mrv_common::period_key(monitoring_period_n);
        match self.verification_statements().get(&(pai_id, pk)) {
            Some(record) => OptionalValue::Some(record),
            None => OptionalValue::None,
        }
    }

    /// Validates the verification case state-machine transition.
    ///
    /// Allowed transitions:
    /// `pending_assignment` → `assigned` | `rejected`
    /// `assigned` → `in_review` | `needs_more_information` | `approved` | `rejected` | `escalated`
    /// `in_review` → `needs_more_information` | `approved` | `rejected` | `escalated`
    /// `needs_more_information` → `assigned`
    /// `escalated` → `assigned` | `approved` | `rejected`
    fn is_valid_verification_transition(
        &self,
        current: &ManagedBuffer,
        next: &ManagedBuffer,
    ) -> bool {
        (current == &b"pending_assignment" && (next == &b"assigned" || next == &b"rejected"))
            || (current == &b"assigned"
                && (next == &b"in_review"
                    || next == &b"needs_more_information"
                    || next == &b"approved"
                    || next == &b"rejected"
                    || next == &b"escalated"))
            || (current == &b"in_review"
                && (next == &b"needs_more_information"
                    || next == &b"approved"
                    || next == &b"rejected"
                    || next == &b"escalated"))
            || (current == &b"needs_more_information" && next == &b"assigned")
            || (current == &b"escalated"
                && (next == &b"assigned" || next == &b"approved" || next == &b"rejected"))
    }

    fn is_valid_methodology_status(&self, status: &ManagedBuffer) -> bool {
        status == &b"ready_for_review"
            || status == &b"approved_internal"
            || status == &b"superseded"
    }

    fn is_valid_methodology_transition(
        &self,
        current: &ManagedBuffer,
        next: &ManagedBuffer,
    ) -> bool {
        current == next
            || (current == &b"ready_for_review" && next == &b"approved_internal")
            || (current == &b"approved_internal" && next == &b"superseded")
    }

    fn is_valid_project_status(&self, status: &ManagedBuffer) -> bool {
        status == &b"pending" || status == &b"active"
    }

    fn is_valid_project_transition(&self, current: &ManagedBuffer, next: &ManagedBuffer) -> bool {
        current == next || (current == &b"pending" && next == &b"active")
    }

    fn require_approved_methodology_record(
        &self,
        methodology_id: &ManagedBuffer,
        methodology_version_label: &ManagedBuffer,
    ) {
        let methodology_key = (methodology_id.clone(), methodology_version_label.clone());
        require!(
            self.methodology_records().contains_key(&methodology_key),
            "ENTITY_NOT_FOUND: methodology_record"
        );
        let methodology = self.methodology_records().get(&methodology_key).unwrap();
        require!(
            methodology.approval_status == ManagedBuffer::from(b"approved_internal"),
            "methodology must be approved_internal"
        );
    }

    fn derive_methodology_canonical_id(
        &self,
        methodology_id: &ManagedBuffer,
        version_label: &ManagedBuffer,
        pack_digest: &ManagedBuffer,
    ) -> ManagedBuffer {
        let mut preimage = ManagedBuffer::new();
        preimage.append_bytes(METHODOLOGY_CANONICAL_ID_DOMAIN);
        preimage.append_bytes(&[0x00]);
        self.append_len_prefixed_buffer(&mut preimage, methodology_id);
        self.append_len_prefixed_buffer(&mut preimage, version_label);
        self.append_len_prefixed_buffer(&mut preimage, pack_digest);
        self.crypto().sha256(&preimage).as_managed_buffer().clone()
    }

    fn derive_project_canonical_id(
        &self,
        project_id: &ManagedBuffer,
        tenant_id: &ManagedBuffer,
        asset_id: &ManagedBuffer,
        reporting_period_id: &ManagedBuffer,
        methodology_id: &ManagedBuffer,
        methodology_version_label: &ManagedBuffer,
    ) -> ManagedBuffer {
        let mut preimage = ManagedBuffer::new();
        preimage.append_bytes(PROJECT_CANONICAL_ID_DOMAIN);
        preimage.append_bytes(&[0x00]);
        self.append_len_prefixed_buffer(&mut preimage, project_id);
        self.append_len_prefixed_buffer(&mut preimage, tenant_id);
        self.append_len_prefixed_buffer(&mut preimage, asset_id);
        self.append_len_prefixed_buffer(&mut preimage, reporting_period_id);
        self.append_len_prefixed_buffer(&mut preimage, methodology_id);
        self.append_len_prefixed_buffer(&mut preimage, methodology_version_label);
        self.crypto().sha256(&preimage).as_managed_buffer().clone()
    }

    fn derive_evidence_canonical_id(
        &self,
        evidence_id: &ManagedBuffer,
        entity_type: &ManagedBuffer,
        entity_id: &ManagedBuffer,
        evidence_hash: &ManagedBuffer,
        manifest_hash: &ManagedBuffer,
    ) -> ManagedBuffer {
        let mut preimage = ManagedBuffer::new();
        preimage.append_bytes(EVIDENCE_CANONICAL_ID_DOMAIN);
        preimage.append_bytes(&[0x00]);
        self.append_len_prefixed_buffer(&mut preimage, evidence_id);
        self.append_len_prefixed_buffer(&mut preimage, entity_type);
        self.append_len_prefixed_buffer(&mut preimage, entity_id);
        self.append_len_prefixed_buffer(&mut preimage, evidence_hash);
        self.append_len_prefixed_buffer(&mut preimage, manifest_hash);
        self.crypto().sha256(&preimage).as_managed_buffer().clone()
    }

    fn derive_verification_case_canonical_id(
        &self,
        case_id: &ManagedBuffer,
        target_type: &ManagedBuffer,
        target_id: &ManagedBuffer,
    ) -> ManagedBuffer {
        let mut preimage = ManagedBuffer::new();
        preimage.append_bytes(VERIFICATION_CASE_CANONICAL_ID_DOMAIN);
        preimage.append_bytes(&[0x00]);
        self.append_len_prefixed_buffer(&mut preimage, case_id);
        self.append_len_prefixed_buffer(&mut preimage, target_type);
        self.append_len_prefixed_buffer(&mut preimage, target_id);
        self.crypto().sha256(&preimage).as_managed_buffer().clone()
    }

    fn derive_issuance_lot_canonical_id(
        &self,
        lot_id: &ManagedBuffer,
        project_id: &ManagedBuffer,
        verification_case_id: &ManagedBuffer,
        vintage: u64,
        quantity_scaled: &BigUint,
        replacement_for_lot_id: &ManagedBuffer,
    ) -> ManagedBuffer {
        let mut preimage = ManagedBuffer::new();
        preimage.append_bytes(ISSUANCE_LOT_CANONICAL_ID_DOMAIN);
        preimage.append_bytes(&[0x00]);
        self.append_len_prefixed_buffer(&mut preimage, lot_id);
        self.append_len_prefixed_buffer(&mut preimage, project_id);
        self.append_len_prefixed_buffer(&mut preimage, verification_case_id);
        preimage.append_bytes(&vintage.to_be_bytes());
        self.append_len_prefixed_buffer(&mut preimage, &quantity_scaled.to_bytes_be_buffer());
        self.append_len_prefixed_buffer(&mut preimage, replacement_for_lot_id);
        self.crypto().sha256(&preimage).as_managed_buffer().clone()
    }

    fn derive_report_canonical_id(
        &self,
        report_id: &ManagedBuffer,
        public_tenant_id: &ManagedBuffer,
        public_farm_id: &ManagedBuffer,
        public_season_id: &ManagedBuffer,
        public_project_id: &ManagedBuffer,
    ) -> ManagedBuffer {
        let mut preimage = ManagedBuffer::new();
        preimage.append_bytes(REPORT_CANONICAL_ID_DOMAIN);
        preimage.append_bytes(&[0x00]);
        self.append_len_prefixed_buffer(&mut preimage, report_id);
        self.append_len_prefixed_buffer(&mut preimage, public_tenant_id);
        self.append_len_prefixed_buffer(&mut preimage, public_farm_id);
        self.append_len_prefixed_buffer(&mut preimage, public_season_id);
        self.append_len_prefixed_buffer(&mut preimage, public_project_id);
        self.crypto().sha256(&preimage).as_managed_buffer().clone()
    }

    fn append_len_prefixed_buffer(&self, out: &mut ManagedBuffer, value: &ManagedBuffer) {
        let len = value.len();
        out.append_bytes(&len.to_be_bytes());
        out.append(value);
    }

    fn is_vvb_accredited_via_governance_or_local(&self, vvb_did: ManagedAddress) -> bool {
        use governance_proxy::GovernanceProxy;

        if self.governance_read_address().is_empty() {
            return self.accredited_vvbs().contains(&vvb_did);
        }

        let gas_for_query = self.blockchain().get_gas_left() / 16;
        self.tx()
            .to(self.governance_read_address().get())
            .gas(gas_for_query)
            .typed(GovernanceProxy)
            .is_accredited_vvb(vvb_did)
            .returns(ReturnsResult)
            .sync_call_readonly()
    }

    fn require_local_vvb_registry_mode(&self) {
        require!(
            self.governance_read_address().is_empty(),
            "VVB_REGISTRY_CANONICALIZED_TO_GOVERNANCE: local VVB registry mutations are disabled while governanceReadAddress is configured"
        );
    }

    fn require_terminal_lifecycle_authority(&self) {
        if self.carbon_credit_lifecycle_address().is_empty() {
            self.require_governance_or_owner();
            return;
        }

        let caller = self.blockchain().get_caller();
        require!(
            caller == self.carbon_credit_lifecycle_address().get(),
            "ISSUANCE_LOT_LIFECYCLE_CANONICALIZED_TO_CARBON_CREDIT"
        );
    }

    fn require_not_paused(&self) {
        use governance_proxy::GovernanceProxy;

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

        let gas_for_query = self.blockchain().get_gas_left() / 16;
        let paused: bool = self
            .tx()
            .to(self.governance_read_address().get())
            .gas(gas_for_query)
            .typed(GovernanceProxy)
            .get_paused()
            .returns(ReturnsResult)
            .sync_call_readonly();
        require!(!paused, "MRV_GOVERNANCE_PAUSED");
    }

    /// Monotonically increasing version counter per verification case,
    /// incremented on every status change. Consumers compare this value
    /// against their last-seen version to detect stale reads.
    #[view(getVerificationCaseVersion)]
    #[storage_mapper("verificationCaseVersion")]
    fn verification_case_version(&self, case_id: &ManagedBuffer) -> SingleValueMapper<u64>;

    #[view(getGovernanceReadAddress)]
    #[storage_mapper("governanceReadAddress")]
    fn governance_read_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getCarbonCreditLifecycleAddress)]
    #[storage_mapper("carbonCreditLifecycleAddress")]
    fn carbon_credit_lifecycle_address(&self) -> SingleValueMapper<ManagedAddress>;

    /// Storage layout version for forward-compatible upgrades.
    #[view(getStorageVersion)]
    #[storage_mapper("storageVersion")]
    fn storage_version(&self) -> SingleValueMapper<u32>;

    /// Upgrades storage layout version if needed and preserves existing state.
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
