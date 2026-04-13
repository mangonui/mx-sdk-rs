use drwa_asset_manager::DrwaAssetManager;
use drwa_common::{DrwaCallerDomain, DrwaGovernanceModule, DrwaSyncOperationType};
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const HOLDER: TestAddress = TestAddress::new("holder");
const OTHER: TestAddress = TestAddress::new("other");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-asset-manager");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-asset-manager.mxsc.json");
const TOKEN_ID_1: &[u8] = b"HOTEL-ab12cd";
const TOKEN_ID_2: &[u8] = b"HOTEL-bc23de";

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/drwa/asset-manager");
    world.register_contract(CODE_PATH, drwa_asset_manager::ContractBuilder);
    world
}

#[test]
fn asset_manager_whitebox_flow() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"policy-hotel-1"),
            );
        });

    world
        .tx()
        .from(OWNER)
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
                true,
            );

            assert!(envelope.caller_domain == DrwaCallerDomain::AssetManager);
            assert_eq!(envelope.operations.len(), 1);

            let operation = envelope.operations.get(0);
            assert!(operation.operation_type == DrwaSyncOperationType::HolderMirror);
            assert_eq!(operation.version, 1);
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
fn asset_manager_rejects_non_owner_and_increments_holder_version() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"policy-hotel-1"),
            );
        });

    for version in [1u64, 2u64] {
        world
            .tx()
            .from(OWNER)
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
                    true,
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
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
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
                ManagedBuffer::from(b"policy-hotel-2"),
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
                true,
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        });
}

#[test]
fn asset_manager_requires_pending_governance_acceptance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
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

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(OTHER.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
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
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

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
                ManagedBuffer::from(b"policy-hotel-invalid"),
            );
        });
}

#[test]
fn asset_manager_rejects_reregistration_for_same_token() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"policy-hotel-1"),
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
                ManagedBuffer::from(b"policy-hotel-2"),
            );
        });
}

#[test]
fn asset_manager_rejects_sync_holder_compliance_on_unregistered_asset() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Attempt to sync holder compliance without registering the asset first
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "asset not registered: use registerAsset first"))
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

#[test]
fn asset_manager_rejects_zero_address_holder() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"policy-hotel-1"),
            );
        });

    world
        .tx()
        .from(OWNER)
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
                true,
            );
        });
}

#[test]
fn asset_manager_update_asset_works() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"policy-hotel-1"),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.update_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"SFT"),
                ManagedBuffer::from(b"RealEstate"),
                ManagedBuffer::from(b"policy-hotel-2"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let asset = sc.asset(&ManagedBuffer::from(TOKEN_ID_1)).get();
            assert_eq!(asset.carrier_type, ManagedBuffer::from(b"SFT"));
            assert_eq!(asset.asset_class, ManagedBuffer::from(b"RealEstate"));
            assert_eq!(asset.policy_id, ManagedBuffer::from(b"policy-hotel-2"));
            assert!(asset.regulated);
        });
}

#[test]
fn asset_manager_update_asset_rejects_unregistered() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "asset not registered: use registerAsset first"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.update_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"SFT"),
                ManagedBuffer::from(b"RealEstate"),
                ManagedBuffer::from(b"policy-new"),
            );
        });
}

// ── MiCA Wind-Down Tests ──────────────────────────────────────────────

fn wind_down_setup() -> ScenarioWorld {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"policy-hotel-1"),
            );
        });

    world
}

#[test]
fn wind_down_initiate_success() {
    let mut world = wind_down_setup();

    world
        .tx()
        .from(OWNER)
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
        });
}

#[test]
fn wind_down_rejects_double_initiation() {
    let mut world = wind_down_setup();

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "WIND_DOWN_ALREADY_INITIATED"))
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.initiate_wind_down(ManagedBuffer::from(TOKEN_ID_1));
        });
}

#[test]
fn wind_down_rejects_unregistered_asset() {
    let mut world = wind_down_setup();

    world
        .tx()
        .from(OWNER)
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
        .from(OWNER)
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
            assert_eq!(asset.wind_down_round, 42, "wind_down_round should match the block round at initiation");
        });

    // Verify registration of a different token still works (wind-down is per-token)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_2),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"policy-hotel-2"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let asset2 = sc.asset(&ManagedBuffer::from(TOKEN_ID_2)).get();
            assert!(!asset2.wind_down_initiated, "new token should not be in wind-down");
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
        .from(OWNER)
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
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            sc.register_asset(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ESDT"),
                ManagedBuffer::from(b"Hospitality"),
                ManagedBuffer::from(b"policy-hotel-1"),
            );
        });

    world
        .tx()
        .from(OWNER)
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
                true,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_asset_manager::contract_obj, |sc| {
            let mirror = sc.get_holder_mirror(
                ManagedBuffer::from(TOKEN_ID_1),
                HOLDER.to_managed_address(),
            );
            assert_eq!(mirror.holder_policy_version, 1);
            assert_eq!(mirror.kyc_status, ManagedBuffer::from(b"approved"));
            assert_eq!(mirror.aml_status, ManagedBuffer::from(b"clear"));
            assert!(mirror.auditor_authorized);
        });
}
