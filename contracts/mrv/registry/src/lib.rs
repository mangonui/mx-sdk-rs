#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use mrv_common::MrvReportProof;

/// Versioned methodology record with approval lifecycle and supersession tracking.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
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
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
pub struct ProjectRecord<M: ManagedTypeApi> {
    pub project_id: ManagedBuffer<M>,
    pub tenant_id: ManagedBuffer<M>,
    pub asset_id: ManagedBuffer<M>,
    pub reporting_period_id: ManagedBuffer<M>,
    pub methodology_id: ManagedBuffer<M>,
    pub status: ManagedBuffer<M>,
}

/// Content-addressed evidence record anchored to an entity (project, farm, season).
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
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
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
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
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
pub struct IssuanceLotRecord<M: ManagedTypeApi> {
    pub lot_id: ManagedBuffer<M>,
    pub project_id: ManagedBuffer<M>,
    pub verification_case_id: ManagedBuffer<M>,
    pub vintage: u64,
    pub quantity: ManagedBuffer<M>,
    pub status: ManagedBuffer<M>,
    pub replacement_for_lot_id: ManagedBuffer<M>,
    pub reversed_amount: ManagedBuffer<M>,
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
    pub quantity: ManagedBuffer<M>,
    pub replacement_for_lot_id: ManagedBuffer<M>,
}

/// Event payload for `mrvIssuanceLotReversed`.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone)]
pub struct IssuanceLotReversedEventPayload<M: ManagedTypeApi> {
    pub reversed_amount: ManagedBuffer<M>,
    pub replacement_lot_id: ManagedBuffer<M>,
}

/// Execution bundle committed for a PAI monitoring period.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
pub struct ExecutionBundleRecord<M: ManagedTypeApi> {
    pub pai_id: ManagedBuffer<M>,
    pub monitoring_period_n: u64,
    pub bundle_cid: ManagedBuffer<M>,
    pub bundle_hash: ManagedBuffer<M>,
    pub committed_at: u64,
}

/// Verification statement submitted for a PAI monitoring period.
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
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
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, ManagedVecItem, Clone, PartialEq, Eq)]
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

        require!(!methodology_id.is_empty(), "empty methodology id");
        require!(!version_label.is_empty(), "empty version label");
        require!(!pack_digest.is_empty(), "empty pack digest");
        require!(!approval_status.is_empty(), "empty approval status");
        require!(effective_from > 0, "invalid effective from");
        require!(
            effective_to == 0 || effective_to >= effective_from,
            "invalid effective window"
        );

        let key = (methodology_id.clone(), version_label.clone());
        let existing = self.methodology_records().get(&key);
        if let Some(record) = existing {
            require!(record.pack_digest == pack_digest, "conflicting methodology record");
            require!(
                record.approval_status == approval_status,
                "conflicting methodology status"
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
                pack_digest,
                approval_status,
                effective_from,
                effective_to,
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
        require!(!methodology_id.is_empty(), "empty methodology id");
        require!(!version_label.is_empty(), "empty version label");
        require!(!approval_status.is_empty(), "empty approval status");

        let key = (methodology_id.clone(), version_label.clone());
        require!(
            self.methodology_records().contains_key(&key),
            "ENTITY_NOT_FOUND: methodology_record"
        );
        let mut record = self.methodology_records().get(&key).unwrap();
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
        require!(!methodology_id.is_empty(), "empty methodology id");
        require!(!version_label.is_empty(), "empty version label");
        require!(
            !replacement_version_label.is_empty(),
            "empty replacement version label"
        );
        require!(effective_to > 0, "invalid supersession effective to");

        let key = (methodology_id.clone(), version_label.clone());
        require!(
            self.methodology_records().contains_key(&key),
            "ENTITY_NOT_FOUND: methodology_record"
        );
        let mut record = self.methodology_records().get(&key).unwrap();
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
        status: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        require!(!project_id.is_empty(), "empty project id");
        require!(!tenant_id.is_empty(), "empty tenant id");
        require!(!asset_id.is_empty(), "empty asset id");
        require!(!reporting_period_id.is_empty(), "empty reporting period id");
        require!(!methodology_id.is_empty(), "empty methodology id");
        require!(!status.is_empty(), "empty project status");

        let record = ProjectRecord {
            project_id: project_id.clone(),
            tenant_id: tenant_id.clone(),
            asset_id: asset_id.clone(),
            reporting_period_id: reporting_period_id.clone(),
            methodology_id: methodology_id.clone(),
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
            return;
        }

        self.project_records().insert(project_id.clone(), record);
        self.mrv_project_registered_event(
            &project_id,
            &tenant_id,
            &ProjectRegisteredEventPayload {
                asset_id,
                reporting_period_id,
                methodology_id,
                status,
            },
        );
    }

    /// Updates the status of an existing project record.
    #[endpoint(setProjectStatus)]
    fn set_project_status(&self, project_id: ManagedBuffer, status: ManagedBuffer) {
        self.require_governance_or_owner();
        require!(!project_id.is_empty(), "empty project id");
        require!(!status.is_empty(), "empty project status");

        require!(
            self.project_records().contains_key(&project_id),
            "ENTITY_NOT_FOUND: project_record"
        );
        let mut record = self.project_records().get(&project_id).unwrap();
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
        require!(!evidence_id.is_empty(), "empty evidence id");
        require!(!entity_type.is_empty(), "empty entity type");
        require!(!entity_id.is_empty(), "empty entity id");
        require!(!evidence_hash.is_empty(), "empty evidence hash");
        require!(!manifest_hash.is_empty(), "empty manifest hash");
        require!(submitted_at > 0, "invalid submitted at");

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
                evidence_hash,
                manifest_hash,
                submitted_at,
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
        updated_at: u64,
    ) {
        self.require_governance_or_owner();
        require!(!case_id.is_empty(), "empty case id");
        require!(!target_type.is_empty(), "empty target type");
        require!(!target_id.is_empty(), "empty target id");
        require!(!assignee.is_zero(), "empty assignee");
        require!(updated_at > 0, "invalid updated at");
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
        updated_at: u64,
    ) {
        self.require_governance_or_owner();
        require!(!case_id.is_empty(), "empty case id");
        require!(!status.is_empty(), "empty verification status");
        require!(!assignee.is_zero(), "empty assignee");
        require!(updated_at > 0, "invalid updated at");

        require!(
            self.verification_cases().contains_key(&case_id),
            "ENTITY_NOT_FOUND: verification_case"
        );
        let mut record = self.verification_cases().get(&case_id).unwrap();
        require!(
            self.is_valid_verification_transition(&record.status, &status),
            "invalid verification transition"
        );

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
    #[endpoint(createIssuanceLot)]
    fn create_issuance_lot(
        &self,
        lot_id: ManagedBuffer,
        project_id: ManagedBuffer,
        verification_case_id: ManagedBuffer,
        vintage: u64,
        quantity: ManagedBuffer,
        replacement_for_lot_id: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        require!(!lot_id.is_empty(), "empty lot id");
        require!(!project_id.is_empty(), "empty project id");
        require!(!verification_case_id.is_empty(), "empty verification case id");
        require!(vintage > 0, "invalid vintage");
        require!(!quantity.is_empty(), "empty quantity");

        let record = IssuanceLotRecord {
            lot_id: lot_id.clone(),
            project_id: project_id.clone(),
            verification_case_id: verification_case_id.clone(),
            vintage,
            quantity: quantity.clone(),
            status: ManagedBuffer::from(b"minted"),
            replacement_for_lot_id: replacement_for_lot_id.clone(),
            reversed_amount: ManagedBuffer::new(),
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
                quantity,
                replacement_for_lot_id,
            },
        );
    }

    /// Transitions a minted issuance lot to `retired` status.
    #[endpoint(retireIssuanceLot)]
    fn retire_issuance_lot(&self, lot_id: ManagedBuffer) {
        self.require_governance_or_owner();
        require!(!lot_id.is_empty(), "empty lot id");

        require!(
            self.issuance_lots().contains_key(&lot_id),
            "ENTITY_NOT_FOUND: issuance_lot"
        );
        let mut record = self.issuance_lots().get(&lot_id).unwrap();
        require!(record.status == b"minted", "lot not eligible for retirement");
        record.status = ManagedBuffer::from(b"retired");
        self.issuance_lots().insert(lot_id.clone(), record);
        self.mrv_issuance_lot_retired_event(&lot_id);
    }

    /// Reverses a minted or retired issuance lot and records the reversed amount.
    #[endpoint(reverseIssuanceLot)]
    fn reverse_issuance_lot(
        &self,
        lot_id: ManagedBuffer,
        reversed_amount: ManagedBuffer,
        replacement_lot_id: ManagedBuffer,
    ) {
        self.require_governance_or_owner();
        require!(!lot_id.is_empty(), "empty lot id");
        require!(!reversed_amount.is_empty(), "empty reversed amount");

        require!(
            self.issuance_lots().contains_key(&lot_id),
            "ENTITY_NOT_FOUND: issuance_lot"
        );
        let mut record = self.issuance_lots().get(&lot_id).unwrap();
        require!(
            record.status == b"minted" || record.status == b"retired",
            "lot not eligible for reversal"
        );
        record.status = ManagedBuffer::from(b"reversed");
        record.reversed_amount = reversed_amount.clone();
        self.issuance_lots().insert(lot_id.clone(), record);
        self.mrv_issuance_lot_reversed_event(
            &lot_id,
            &IssuanceLotReversedEventPayload {
                reversed_amount,
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
                public_project_id,
                evidence_manifest_hash,
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
    fn verifier_adjustment_count(
        &self,
        pai_id: &ManagedBuffer,
    ) -> MapMapper<ManagedBuffer, u64>;

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

    #[storage_mapper("reportProofs")]
    fn report_proofs(&self) -> MapMapper<ManagedBuffer, MrvReportProof<Self::Api>>;

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
    fn verification_cases(
        &self,
    ) -> MapMapper<ManagedBuffer, VerificationCaseRecord<Self::Api>>;

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
        require!(!vvb_did.is_zero(), "vvb_did must not be zero");
        self.accredited_vvbs().insert(vvb_did);
    }

    /// Removes an accredited VVB address.
    #[endpoint(deregisterAccreditedVvb)]
    fn deregister_accredited_vvb(&self, vvb_did: ManagedAddress) {
        self.require_governance_or_owner();
        self.accredited_vvbs().swap_remove(&vvb_did);
    }

    #[view(isVvbAccredited)]
    fn is_vvb_accredited(&self, vvb_did: ManagedAddress) -> bool {
        self.accredited_vvbs().contains(&vvb_did)
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
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(!bundle_cid.is_empty(), "empty bundle_cid");
        require!(bundle_hash.len() == 32, "bundle_hash must be 32 bytes (SHA-256)");

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
            committed_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
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
        self.require_governance_or_owner();
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
            self.accredited_vvbs().contains(&vvb_did),
            "VVB_NOT_ACCREDITED: vvb_did must be registered via registerAccreditedVvb"
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
            submitted_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
        };

        self.verification_statements().insert(key, record);
        self.mrv_verification_statement_submitted_event(
            &pai_id, &vvb_did, &statement_cid, &outcome,
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
        self.require_governance_or_owner();
        require!(!pai_id.is_empty(), "empty pai_id");
        require!(monitoring_period_n > 0, "invalid monitoring_period_n");
        require!(!adjustment_cid.is_empty(), "empty adjustment_cid");

        let pk = mrv_common::period_key(monitoring_period_n);
        let stmt_key = (pai_id.clone(), pk.clone());
        require!(
            self.verification_statements().contains_key(&stmt_key),
            "verification statement not submitted for this PAI/period"
        );

        let current: u64 = self.verifier_adjustment_count(&pai_id).get(&pk).unwrap_or(0u64);
        let next_seq: u64 = current + 1;
        self.verifier_adjustment_count(&pai_id).insert(pk.clone(), next_seq);

        let sk = mrv_common::period_key(next_seq);
        let adjustment_key = (pai_id.clone(), pk, sk);

        let record = VerifierAdjustmentRecord {
            pai_id: pai_id.clone(),
            monitoring_period_n,
            adjustment_cid: adjustment_cid.clone(),
            sequence: next_seq,
            submitted_at: self.blockchain().get_block_timestamp_seconds().as_u64_seconds(),
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
                && (next == &b"assigned"
                    || next == &b"approved"
                    || next == &b"rejected"))
    }

    /// Monotonically increasing version counter per verification case,
    /// incremented on every status change. Consumers compare this value
    /// against their last-seen version to detect stale reads.
    #[view(getVerificationCaseVersion)]
    #[storage_mapper("verificationCaseVersion")]
    fn verification_case_version(&self, case_id: &ManagedBuffer) -> SingleValueMapper<u64>;

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
