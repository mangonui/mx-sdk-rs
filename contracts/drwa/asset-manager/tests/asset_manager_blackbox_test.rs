use multiversx_sc_scenario::imports::*;

use drwa_asset_manager::drwa_asset_manager_proxy::DrwaAssetManagerProxy;
use drwa_policy_registry::drwa_policy_registry_proxy::DrwaPolicyRegistryProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const _NEW_GOVERNANCE: TestAddress = TestAddress::new("new_governance");
const HOLDER: TestAddress = TestAddress::new("holder");
const OTHER: TestAddress = TestAddress::new("other");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-asset-manager");
const POLICY_SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-policy-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-asset-manager.mxsc.json");
const POLICY_CODE_PATH: MxscPath =
    MxscPath::new("mxsc:../policy-registry/output/drwa-policy-registry.mxsc.json");
const TOKEN_ID: &[u8] = b"HOTEL-ab12cd";
const _TOKEN_ID_2: &[u8] = b"HOTEL-bc23de";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/asset-manager");
    blockchain.register_contract(CODE_PATH, drwa_asset_manager::ContractBuilder);
    blockchain.register_contract(POLICY_CODE_PATH, drwa_policy_registry::ContractBuilder);
    blockchain
}

fn deploy_with_policy_registry(world: &mut ScenarioWorld) {
    world
        .tx()
        .from(OWNER)
        .typed(DrwaPolicyRegistryProxy)
        .init(GOVERNANCE)
        .code(POLICY_CODE_PATH)
        .new_address(POLICY_SC_ADDRESS)
        .run();

    world
        .tx()
        .from(OWNER)
        .typed(DrwaAssetManagerProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .set_policy_registry_address(POLICY_SC_ADDRESS)
        .run();

    for token_id in [TOKEN_ID] {
        world
            .tx()
            .from(GOVERNANCE)
            .to(POLICY_SC_ADDRESS)
            .typed(DrwaPolicyRegistryProxy)
            .set_token_policy(
                ManagedBuffer::from(token_id),
                true,
                false,
                false,
                false,
                ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
                ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new(),
            )
            .run();
    }
}

/// Deploy, register an asset via typed proxy, query back and verify the
/// persisted AssetRecord fields.
#[test]
fn asset_manager_blackbox_register_and_query() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    deploy_with_policy_registry(&mut world);

    // Verify governance
    let gov: ManagedAddress<StaticApi> = world
        .query()
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .governance()
        .returns(ReturnsResult)
        .run();
    assert_eq!(gov, GOVERNANCE.to_managed_address());

    // Register an asset from the configured governance address.
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"HOTEL-ab12cd"),
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
    assert_eq!(
        asset.carrier_type,
        ManagedBuffer::<StaticApi>::from(b"ESDT")
    );
    assert_eq!(
        asset.asset_class,
        ManagedBuffer::<StaticApi>::from(b"Hospitality")
    );
    assert_eq!(
        asset.policy_id,
        ManagedBuffer::<StaticApi>::from(b"HOTEL-ab12cd")
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

    deploy_with_policy_registry(&mut world);

    // Register asset first
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"HOTEL-ab12cd"),
        )
        .run();

    // Sync holder compliance — tx must succeed (exit 0)
    world
        .tx()
        .from(GOVERNANCE)
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
            false,  // auditor_authorized is attestation-owned
        )
        .run();

    // Second sync for the same holder — version should increment (verify tx succeeds)
    world
        .tx()
        .from(GOVERNANCE)
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
            false,
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

    deploy_with_policy_registry(&mut world);

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
            ManagedBuffer::from(b"HOTEL-ab12cd"),
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

    deploy_with_policy_registry(&mut world);

    // First registration — should succeed
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(b"HOTEL-ab12cd"),
        )
        .run();

    // Second registration — same token_id, must revert
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaAssetManagerProxy)
        .register_asset(
            ManagedBuffer::from(TOKEN_ID),
            ManagedBuffer::from(b"ESDT"),
            ManagedBuffer::from(b"Hospitality"),
            ManagedBuffer::from(TOKEN_ID),
        )
        .with_result(ExpectError(
            4,
            "asset already registered - use an upgrade endpoint to modify",
        ))
        .run();
}
