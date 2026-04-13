use multiversx_sc_scenario::imports::*;

use drwa_asset_manager::drwa_asset_manager_proxy::DrwaAssetManagerProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const _NEW_GOVERNANCE: TestAddress = TestAddress::new("new_governance");
const HOLDER: TestAddress = TestAddress::new("holder");
const OTHER: TestAddress = TestAddress::new("other");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-asset-manager");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-asset-manager.mxsc.json");
const TOKEN_ID: &[u8] = b"HOTEL-ab12cd";
const _TOKEN_ID_2: &[u8] = b"HOTEL-bc23de";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/asset-manager");
    blockchain.register_contract(CODE_PATH, drwa_asset_manager::ContractBuilder);
    blockchain
}

/// Deploy, register an asset via typed proxy, query back and verify the
/// persisted AssetRecord fields.
#[test]
fn asset_manager_blackbox_register_and_query() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .typed(DrwaAssetManagerProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    // Verify governance
    let gov: ManagedAddress<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .governance()
        .returns(ReturnsResult)
        .run();
    assert_eq!(gov, GOVERNANCE.to_managed_address());

    // Register an asset from the owner
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"policy-hotel-1"),
        )
        .run();

    // Query back the asset record
    let asset: drwa_asset_manager::AssetRecord<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .asset(ManagedBuffer::<StaticApi>::from(TOKEN_ID))
        .returns(ReturnsResult)
        .run();

    assert_eq!(asset.token_id, ManagedBuffer::<StaticApi>::from(TOKEN_ID));
    assert_eq!(asset.carrier_type, ManagedBuffer::<StaticApi>::from(b"ESDT"));
    assert_eq!(
        asset.asset_class,
        ManagedBuffer::<StaticApi>::from(b"Hospitality")
    );
    assert_eq!(
        asset.policy_id,
        ManagedBuffer::<StaticApi>::from(b"policy-hotel-1")
    );
    assert!(asset.regulated);
}

/// Deploy, register an asset, then sync holder compliance for that token.
/// The holder_mirror storage mapper has no #[view], so we verify the
/// transaction succeeds without revert.
#[test]
fn asset_manager_blackbox_sync_holder_compliance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(HOLDER).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .typed(DrwaAssetManagerProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    // Register asset first
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"policy-hotel-1"),
        )
        .run();

    // Sync holder compliance — tx must succeed (exit 0)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .sync_holder_compliance(
            ManagedBuffer::from(TOKEN_ID),
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"approved"),
            ManagedBuffer::from(b"clear"),
            ManagedBuffer::from(b"accredited"),
            ManagedBuffer::from(b"SG"),
            500u64, // expiry_round (must be > current round, which is 0)
            false,  // transfer_locked
            false,  // receive_locked
            true,   // auditor_authorized
        )
        .run();

    // Second sync for the same holder — version should increment (verify tx succeeds)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .sync_holder_compliance(
            ManagedBuffer::from(TOKEN_ID),
            HOLDER.to_managed_address(),
            ManagedBuffer::from(b"approved"),
            ManagedBuffer::from(b"clear"),
            ManagedBuffer::from(b"accredited"),
            ManagedBuffer::from(b"SG"),
            1000u64,
            true, // transfer_locked changed
            false,
            true,
        )
        .run();
}

/// Deploy, then try to register an asset from an unauthorized address.
/// The call must revert with "caller not authorized".
#[test]
fn asset_manager_blackbox_non_owner_rejected() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(OTHER).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .typed(DrwaAssetManagerProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    // Attempt register_asset from OTHER (neither owner nor governance)
    world
        .tx()
        .from(OTHER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"policy-hotel-1"),
        )
        .with_result(ExpectError(4, "caller not authorized"))
        .run();
}

/// Deploy, register the same token twice. The second registration must revert
/// with the duplicate-guard error.
#[test]
fn asset_manager_blackbox_duplicate_registration_rejected() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .typed(DrwaAssetManagerProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    // First registration — should succeed
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"policy-hotel-1"),
        )
        .run();

    // Second registration — same token_id, must revert
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"policy-hotel-2"),
        )
        .with_result(ExpectError(
            4,
            "asset already registered - use an upgrade endpoint to modify",
        ))
        .run();
}
