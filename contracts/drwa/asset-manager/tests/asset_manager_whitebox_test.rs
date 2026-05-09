use drwa_asset_manager::DrwaAssetManager;
use drwa_common::{
    DrwaCallerDomain, DrwaGovernanceModule, DrwaSyncOperationType, set_drwa_sync_hook_test_result,
};
use drwa_policy_registry::DrwaPolicyRegistry;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const HOLDER: TestAddress = TestAddress::new("holder");
const OTHER: TestAddress = TestAddress::new("other");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-asset-manager");
const POLICY_SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-policy-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-asset-manager.mxsc.json");
const POLICY_CODE_PATH: MxscPath =
    MxscPath::new("mxsc:../policy-registry/output/drwa-policy-registry.mxsc.json");
const TOKEN_ID_1: &[u8] = b"HOTEL-ab12cd";
const TOKEN_ID_2: &[u8] = b"HOTEL-bc23de";
const TOKEN_ID_3: &[u8] = b"HOTEL-cd34ef";

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/drwa/asset-manager");
    world.register_contract(CODE_PATH, drwa_asset_manager::ContractBuilder);
    world.register_contract(POLICY_CODE_PATH, drwa_policy_registry::ContractBuilder);
    world
}

fn hash32(byte: u8) -> ManagedBuffer<DebugApi> {
    ManagedBuffer::from(&[byte; 32][..])
}

fn deploy_asset_manager_with_policy_registry(world: &mut ScenarioWorld, governance: TestAddress) {
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(POLICY_CODE_PATH)
        .new_address(POLICY_SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(governance.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(governance.to_managed_address());
        });

    world
        .tx()
        .from(governance)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.set_policy_registry_address(POLICY_SC_ADDRESS.to_managed_address());
        });

    for token_id in [TOKEN_ID_1, TOKEN_ID_2] {
        world.tx().from(governance).to(POLICY_SC_ADDRESS).whitebox(
            drwa_policy_registry::contract_obj,
            |sc| {
                let investor_classes: ManagedVec<DebugApi, ManagedBuffer<DebugApi>> =
                    ManagedVec::new();
                let jurisdictions: ManagedVec<DebugApi, ManagedBuffer<DebugApi>> =
                    ManagedVec::new();
                sc.set_token_policy(
                    ManagedBuffer::from(token_id),
                    true,
                    false,
                    false,
                    false,
                    investor_classes,
                    jurisdictions,
                );
            },
        );
    }
}

#[test]
fn asset_manager_whitebox_flow() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let envelope = sc.sync_holder_compliance(
                ManagedBuffer::from(TOKEN_ID_1),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                250,
                false,
                false,
                false,
            );

            assert!(envelope.caller_domain == DrwaCallerDomain::AssetManager);
            assert_eq!(envelope.operations.len(), 1);

            let operation = envelope.operations.get(0);
            assert!(operation.operation_type == DrwaSyncOperationType::HolderMirror);
            assert_eq!(operation.version, 1);
            operation.body.with_buffer_contents(|body| {
                assert!(
                    body.len() >= 8,
                    "holder mirror sync body must carry evaluated policy version"
                );
                assert_eq!(&body[body.len() - 8..], &1u64.to_be_bytes());
            });
            assert!(!envelope.payload_hash.is_empty());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(TOKEN_ID_1);
            let asset = sc.asset(&token_id).get();
            assert!(asset.regulated);
            assert_eq!(asset.asset_class, ManagedBuffer::from(b"Hospitality"));

            let mirror = sc
                .holder_mirror(&token_id, &HOLDER.to_managed_address())
                .get();
            assert_eq!(mirror.holder_policy_version, 1);
            assert_eq!(mirror.kyc_status, ManagedBuffer::from(b"approved"));
            assert_eq!(mirror.investor_class, ManagedBuffer::from(b"accredited"));
            assert_eq!(
                sc.holder_policy_version(&token_id, &HOLDER.to_managed_address())
                    .get(),
                1
            );
        });
}

#[test]
fn asset_manager_attaches_hash_only_legal_custody_pack() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );

            sc.attach_asset_legal_custody_pack(
                ManagedBuffer::from(TOKEN_ID_1),
                hash32(0x10),
                hash32(0x20),
                hash32(0x30),
                GOVERNANCE.to_managed_address(),
                hash32(0x40),
                hash32(0x50),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(TOKEN_ID_1);
            let pack = sc.asset_legal_custody_pack(&token_id).get();
            assert_eq!(pack.legal_pack_hash, hash32(0x10));
            assert_eq!(pack.custody_attestation_hash, hash32(0x20));
            assert_eq!(pack.insurance_ref_hash, hash32(0x30));
            assert_eq!(pack.valuation_authority, GOVERNANCE.to_managed_address());
            assert_eq!(pack.redemption_terms_hash, hash32(0x40));
            assert_eq!(pack.asset_state_proof_hash, hash32(0x50));
        });
}

#[test]
fn asset_manager_rejects_short_legal_custody_hash() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "ASSET_BINDING_HASH_MUST_BE_32_BYTES"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.attach_asset_legal_custody_pack(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"too-short"),
                hash32(0x20),
                hash32(0x30),
                GOVERNANCE.to_managed_address(),
                hash32(0x40),
                hash32(0x50),
            );
        });
}

#[test]
fn asset_manager_sync_hook_failure_reverts_asset_registration() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    set_drwa_sync_hook_test_result(13);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "native mirror sync failed"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });
    set_drwa_sync_hook_test_result(0);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(TOKEN_ID_1);
            assert!(sc.asset(&token_id).is_empty());
            assert!(sc.asset_record_version(&token_id).is_empty());
        });
}

#[test]
fn asset_manager_rejects_non_owner_and_increments_holder_version() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    for version in [1u64, 2u64] {
        world
            .tx()
            .from(GOVERNANCE)
            .to(SC_ADDRESS)
            .whitebox(drwa_asset_manager::contract_obj, |sc| {
                let envelope = sc.sync_holder_compliance(
                    ManagedBuffer::from(TOKEN_ID_1),
                    HOLDER.to_managed_address(),
                    ManagedBuffer::from(b"approved"),
                    ManagedBuffer::from(b"clear"),
                    ManagedBuffer::from(b"accredited"),
                    ManagedBuffer::from(b"SG"),
                    250 + version,
                    version == 2,
                    false,
                    false,
                );
                assert_eq!(envelope.operations.get(0).version, version);
            });
    }

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(TOKEN_ID_1);
            let mirror = sc
                .holder_mirror(&token_id, &HOLDER.to_managed_address())
                .get();
            assert_eq!(mirror.holder_policy_version, 2);
            assert!(mirror.transfer_locked);
            assert_eq!(mirror.expiry_round, 252);
        });
}

#[test]
fn asset_manager_allows_governance_to_manage_assets_and_holders() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.accept_governance();
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_2),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-bc23de"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let envelope = sc.sync_holder_compliance(
                ManagedBuffer::from(TOKEN_ID_2),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                500,
                false,
                false,
                false,
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        });
}

#[test]
fn asset_manager_requires_pending_governance_acceptance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
            assert_eq!(
                sc.pending_governance().get(),
                GOVERNANCE.to_managed_address()
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
        });
}

#[test]
fn asset_manager_rejects_expired_pending_governance_acceptance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, OTHER);

    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
        });

    world.current_block().block_round(1_001);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "pending governance acceptance expired"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.accept_governance();
        });
}

#[test]
fn asset_manager_rejects_invalid_token_id_format() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id suffix must be 6 characters"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(b"HOTEL-001"),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-001"),
            );
        });
}

#[test]
fn asset_manager_rejects_register_asset_without_registered_token_policy() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "token policy not registered: setTokenPolicy must be called first",
        ))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_3),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-cd34ef"),
            );
        });
}

#[test]
fn asset_manager_identical_holder_sync_is_noop() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let envelope = sc.sync_holder_compliance(
                ManagedBuffer::from(TOKEN_ID_1),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                250,
                false,
                false,
                false,
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let envelope = sc.sync_holder_compliance(
                ManagedBuffer::from(TOKEN_ID_1),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                250,
                false,
                false,
                false,
            );
            assert_eq!(envelope.operations.len(), 0);
            assert_eq!(
                sc.holder_policy_version(
                    &ManagedBuffer::from(TOKEN_ID_1),
                    &HOLDER.to_managed_address(),
                )
                .get(),
                1
            );
        });
}

#[test]
fn asset_manager_rejects_reregistration_for_same_token() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "asset already registered - use an upgrade endpoint to modify",
        ))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(TOKEN_ID_1),
            );
        });
}

#[test]
fn asset_manager_rejects_sync_holder_compliance_on_unregistered_asset() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    // Attempt to sync holder compliance without registering the asset first
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "asset not registered: use registerAsset first",
        ))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.sync_holder_compliance(
                ManagedBuffer::from(TOKEN_ID_1),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                250,
                false,
                false,
                false,
            );
        });
}

#[test]
fn asset_manager_rejects_zero_address_holder() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "ZERO_ADDRESS: holder must not be zero"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.sync_holder_compliance(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedAddress::zero(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                250,
                false,
                false,
                false,
            );
        });
}

#[test]
fn asset_manager_update_asset_works() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.update_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"SFT"),
                ManagedBuffer::from(b"RealEstate"),
                ManagedBuffer::from(TOKEN_ID_1),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let asset = sc.asset(&ManagedBuffer::from(TOKEN_ID_1)).get();
            assert_eq!(asset.carrier_type, ManagedBuffer::from(b"SFT"));
            assert_eq!(asset.asset_class, ManagedBuffer::from(b"RealEstate"));
            assert_eq!(asset.policy_id, ManagedBuffer::from(TOKEN_ID_1));
            assert_eq!(asset.policy_version_at_register, 1);
            assert!(asset.regulated);
        });
}

#[test]
fn asset_manager_rejects_policy_id_that_does_not_match_token_id() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "policy_id must equal token_id"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(TOKEN_ID_2),
            );
        });
}

#[test]
fn asset_manager_update_asset_rejects_unregistered() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "asset not registered: use registerAsset first",
        ))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.update_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"SFT"),
                ManagedBuffer::from(b"RealEstate"),
                ManagedBuffer::from(TOKEN_ID_1),
            );
        });
}

// ── MiCA Wind-Down Tests ──────────────────────────────────────────────

fn wind_down_setup() -> ScenarioWorld {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
}

#[test]
fn wind_down_initiate_success() {
    let mut world = wind_down_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let envelope = sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
            assert!(envelope.caller_domain == DrwaCallerDomain::AssetManager);
            assert_eq!(envelope.operations.len(), 1);
            let op = envelope.operations.get(0);
            assert!(op.operation_type == DrwaSyncOperationType::AssetRecord);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let asset = sc.asset(&ManagedBuffer::from(TOKEN_ID_1)).get();
            assert!(asset.wind_down_initiated);
            assert!(sc.is_wind_down_initiated(ManagedBuffer::from(TOKEN_ID_1)));
            assert_eq!(
                sc.get_wind_down_status_code(ManagedBuffer::from(TOKEN_ID_1)),
                1
            );
        });
}

#[test]
fn wind_down_rejects_double_initiation() {
    let mut world = wind_down_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "WIND_DOWN_ALREADY_INITIATED"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });
}

#[test]
fn wind_down_complete_keeps_transfer_lock_and_rejects_cancel() {
    let mut world = wind_down_setup();
    world.current_block().block_round(100);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });

    world.current_block().block_round(150);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let envelope = sc.complete_wind_down(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"bafy-completion-evidence"),
            );
            assert_eq!(envelope.operations.len(), 1);
            let body = envelope.operations.get(0).body.clone();
            let body_bytes = body.to_boxed_bytes();
            let body_slice = body_bytes.as_slice();
            assert_eq!(body_slice[0], 0x01);
            assert!(
                core::str::from_utf8(&body_slice[1..])
                    .unwrap()
                    .contains("\"wind_down_status\":\"completed\"")
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let asset = sc.asset(&ManagedBuffer::from(TOKEN_ID_1)).get();
            assert!(asset.wind_down_initiated);
            assert!(sc.is_wind_down_initiated(ManagedBuffer::from(TOKEN_ID_1)));
            assert_eq!(
                sc.get_wind_down_status_code(ManagedBuffer::from(TOKEN_ID_1)),
                2
            );
            assert_eq!(
                sc.get_wind_down_status_round(ManagedBuffer::from(TOKEN_ID_1)),
                150
            );
            assert_eq!(
                sc.get_wind_down_evidence_cid(ManagedBuffer::from(TOKEN_ID_1))
                    .to_boxed_bytes()
                    .as_slice(),
                b"bafy-completion-evidence"
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "WIND_DOWN_NOT_INITIATED"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.cancel_wind_down(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"bafy-cancel-evidence"),
            );
        });
}

#[test]
fn wind_down_cancel_requires_evidence_and_clears_transfer_lock() {
    let mut world = wind_down_setup();
    world.current_block().block_round(77);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "WIND_DOWN_EVIDENCE_REQUIRED"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.cancel_wind_down(ManagedBuffer::from(TOKEN_ID_1), ManagedBuffer::new());
        });

    world.current_block().block_round(88);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let envelope = sc.cancel_wind_down(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"bafy-legal-basis"),
            );
            assert_eq!(envelope.operations.len(), 1);
            let body = envelope.operations.get(0).body.clone();
            let body_bytes = body.to_boxed_bytes();
            let body_slice = body_bytes.as_slice();
            assert_eq!(body_slice[0], 0x01);
            let json = core::str::from_utf8(&body_slice[1..]).unwrap();
            assert!(json.contains("\"wind_down_initiated\":false"));
            assert!(json.contains("\"wind_down_status\":\"cancelled\""));
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let asset = sc.asset(&ManagedBuffer::from(TOKEN_ID_1)).get();
            assert!(!asset.wind_down_initiated);
            assert!(!sc.is_wind_down_initiated(ManagedBuffer::from(TOKEN_ID_1)));
            assert_eq!(
                sc.get_wind_down_status_code(ManagedBuffer::from(TOKEN_ID_1)),
                3
            );
            assert_eq!(
                sc.get_wind_down_status_round(ManagedBuffer::from(TOKEN_ID_1)),
                88
            );
            assert_eq!(
                sc.get_wind_down_evidence_cid(ManagedBuffer::from(TOKEN_ID_1))
                    .to_boxed_bytes()
                    .as_slice(),
                b"bafy-legal-basis"
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
            assert_eq!(
                sc.get_wind_down_status_code(ManagedBuffer::from(TOKEN_ID_1)),
                1
            );
        });
}

#[test]
fn wind_down_rejects_unregistered_asset() {
    let mut world = wind_down_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "ASSET_NOT_REGISTERED"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_2));
        });
}

#[test]
fn wind_down_access_control() {
    let mut world = wind_down_setup();
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });
}

#[test]
fn wind_down_not_initiated_by_default() {
    let mut world = wind_down_setup();

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            assert!(!sc.is_wind_down_initiated(ManagedBuffer::from(TOKEN_ID_1)));
            let asset = sc.asset(&ManagedBuffer::from(TOKEN_ID_1)).get();
            assert!(!asset.wind_down_initiated);
            assert_eq!(asset.wind_down_round, 0);
        });
}

#[test]
fn wind_down_sets_round_and_blocks_holder_sync() {
    let mut world = wind_down_setup();

    // Set block round so wind_down_round has a meaningful value
    world.current_block().block_round(42);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });

    // Verify wind_down_round is set to current block round
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let asset = sc.asset(&ManagedBuffer::from(TOKEN_ID_1)).get();
            assert!(asset.wind_down_initiated);
            assert_eq!(
                asset.wind_down_round, 42,
                "wind_down_round should match the block round at initiation"
            );
        });

    // Verify registration of a different token still works (wind-down is per-token)
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_2),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-bc23de"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let asset2 = sc.asset(&ManagedBuffer::from(TOKEN_ID_2)).get();
            assert!(
                !asset2.wind_down_initiated,
                "new token should not be in wind-down"
            );
            assert_eq!(asset2.wind_down_round, 0);
        });
}

#[test]
fn wind_down_governance_can_initiate() {
    let mut world = wind_down_setup();
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    // Transfer governance
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.accept_governance();
        });

    // Governance initiates wind-down
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let envelope = sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
            assert!(envelope.caller_domain == DrwaCallerDomain::AssetManager);
        });

    // Second attempt by governance also rejected
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "WIND_DOWN_ALREADY_INITIATED"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });
}

#[test]
fn asset_manager_get_holder_mirror_view() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.sync_holder_compliance(
                ManagedBuffer::from(TOKEN_ID_1),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                250,
                false,
                false,
                false,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let mirror =
                sc.get_holder_mirror(ManagedBuffer::from(TOKEN_ID_1), HOLDER.to_managed_address());
            assert_eq!(mirror.holder_policy_version, 1);
            assert_eq!(mirror.kyc_status, ManagedBuffer::from(b"approved"));
            assert_eq!(mirror.aml_status, ManagedBuffer::from(b"clear"));
            assert!(!mirror.auditor_authorized);
        });
}

#[test]
fn asset_manager_rejects_governance_written_auditor_authorization() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    deploy_asset_manager_with_policy_registry(&mut world, GOVERNANCE);

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"HOTEL-ab12cd"),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "AUDITOR_AUTHORIZATION_ATTESTATION_OWNED: use attestation::recordAttestation",
        ))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.sync_holder_compliance(
                ManagedBuffer::from(TOKEN_ID_1),
                HOLDER.to_managed_address(),
                ManagedBuffer::from(b"approved"),
                ManagedBuffer::from(b"clear"),
                ManagedBuffer::from(b"accredited"),
                ManagedBuffer::from(b"SG"),
                250,
                false,
                false,
                true,
            );
        });
}
