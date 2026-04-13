use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const NEW_GOVERNANCE: TestAddress = TestAddress::new("new_governance");
const ISSUER: TestAddress = TestAddress::new("issuer");
const INTRUDER: TestAddress = TestAddress::new("intruder");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-identity-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-identity-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/identity-registry");
    blockchain.register_contract(CODE_PATH, drwa_identity_registry::ContractBuilder);
    blockchain
}

/// Deploy the contract and set up standard accounts.
/// Returns the world instance ready for endpoint calls.
fn deploy() -> ScenarioWorld {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(NEW_GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(ISSUER).nonce(1).balance(1_000_000u64);
    world.account(INTRUDER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .argument(&GOVERNANCE)
        .run();

    world
}

#[test]
fn identity_registry_blackbox_deploy_and_query_governance() {
    let mut world = deploy();

    world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .governance()
        .returns(ExpectValue(GOVERNANCE.to_managed_address()))
        .run();
}

#[test]
fn identity_registry_blackbox_register_and_query() {
    let mut world = deploy();

    // Register identity from governance address
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .register_identity(
            ISSUER.to_managed_address(),
            "Carbon Ventures",
            "SG",
            "REG-001",
            "SPV",
        )
        .run();

    // Query the identity and verify all fields
    let record = world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .identity(ISSUER.to_managed_address())
        .returns(ReturnsResult)
        .run();

    assert_eq!(
        record.subject,
        ISSUER.to_managed_address(),
        "subject mismatch"
    );
    assert_eq!(
        record.legal_name,
        ManagedBuffer::<StaticApi>::from("Carbon Ventures"),
        "legal_name mismatch"
    );
    assert_eq!(
        record.jurisdiction_code,
        ManagedBuffer::<StaticApi>::from("SG"),
        "jurisdiction_code mismatch"
    );
    assert_eq!(
        record.registration_number,
        ManagedBuffer::<StaticApi>::from("REG-001"),
        "registration_number mismatch"
    );
    assert_eq!(
        record.entity_type,
        ManagedBuffer::<StaticApi>::from("SPV"),
        "entity_type mismatch"
    );
    assert_eq!(
        record.kyc_status,
        ManagedBuffer::<StaticApi>::from("pending"),
        "kyc_status should be pending after registration"
    );
    assert_eq!(
        record.aml_status,
        ManagedBuffer::<StaticApi>::from("pending"),
        "aml_status should be pending after registration"
    );
}

#[test]
fn identity_registry_blackbox_unauthorized_register_rejected() {
    let mut world = deploy();

    // Attempt to register from an unauthorized address
    world
        .tx()
        .from(INTRUDER)
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .register_identity(
            ISSUER.to_managed_address(),
            "Blocked",
            "US",
            "REG-X",
            "SPV",
        )
        .with_result(ExpectError(4u64, "caller not authorized"))
        .run();
}

#[test]
fn identity_registry_blackbox_update_compliance() {
    let mut world = deploy();

    // Register identity first
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .register_identity(
            ISSUER.to_managed_address(),
            "Carbon Ventures",
            "SG",
            "REG-001",
            "SPV",
        )
        .run();

    // Update compliance status
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .update_compliance_status(
            ISSUER.to_managed_address(),
            "approved",
            "clear",
            "issuer",
            100u64,
        )
        .run();

    // Query and verify updated fields
    let record = world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .identity(ISSUER.to_managed_address())
        .returns(ReturnsResult)
        .run();

    assert_eq!(
        record.kyc_status,
        ManagedBuffer::<StaticApi>::from("approved"),
        "kyc_status should be approved after update"
    );
    assert_eq!(
        record.aml_status,
        ManagedBuffer::<StaticApi>::from("clear"),
        "aml_status should be clear after update"
    );
    assert_eq!(
        record.investor_class,
        ManagedBuffer::<StaticApi>::from("issuer"),
        "investor_class should be issuer after update"
    );
    assert_eq!(record.expiry_round, 100u64, "expiry_round mismatch");
}

#[test]
fn identity_registry_blackbox_governance_handoff() {
    let mut world = deploy();

    // Verify initial governance
    world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .governance()
        .returns(ExpectValue(GOVERNANCE.to_managed_address()))
        .run();

    // Owner proposes new governance
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .set_governance(NEW_GOVERNANCE.to_managed_address())
        .run();

    // Governance should still be the original until acceptance
    world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .governance()
        .returns(ExpectValue(GOVERNANCE.to_managed_address()))
        .run();

    // Verify pending governance is set
    world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .pending_governance()
        .returns(ExpectValue(NEW_GOVERNANCE.to_managed_address()))
        .run();

    // New governance accepts
    world
        .tx()
        .from(NEW_GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .accept_governance()
        .run();

    // Verify governance is now the new address
    world
        .query()
        .to(SC_ADDRESS)
        .typed(drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy)
        .governance()
        .returns(ExpectValue(NEW_GOVERNANCE.to_managed_address()))
        .run();
}
