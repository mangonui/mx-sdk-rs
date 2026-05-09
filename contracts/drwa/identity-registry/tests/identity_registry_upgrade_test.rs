use multiversx_sc_scenario::imports::*;

use drwa_identity_registry::DrwaIdentityRegistry;
use drwa_identity_registry::drwa_identity_registry_proxy::DrwaIdentityRegistryProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const ISSUER: TestAddress = TestAddress::new("issuer");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-identity-registry-upgrade");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-identity-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/identity-registry");
    blockchain.register_contract(CODE_PATH, drwa_identity_registry::ContractBuilder);
    blockchain
}

#[test]
fn identity_registry_upgrade_preserves_identity_and_storage_version() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(ISSUER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .typed(DrwaIdentityRegistryProxy)
        .init(GOVERNANCE)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaIdentityRegistryProxy)
        .register_identity(
            ISSUER.to_managed_address(),
            "Upgrade Corp",
            "SG",
            "REG-UPGRADE",
            "SPV",
        )
        .run();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .typed(DrwaIdentityRegistryProxy)
        .update_compliance_status(
            ISSUER.to_managed_address(),
            "approved",
            "clear",
            "issuer",
            1_000u64,
        )
        .run();

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaIdentityRegistryProxy)
        .upgrade()
        .code(CODE_PATH)
        .run();

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_identity_registry::contract_obj, |sc| {
            assert_eq!(sc.storage_version().get(), 1);

            let record = sc.identity(&ISSUER.to_managed_address()).get();
            assert_eq!(record.subject, ISSUER.to_managed_address());
            assert_eq!(record.legal_name, ManagedBuffer::from("Upgrade Corp"));
            assert_eq!(record.jurisdiction_code, ManagedBuffer::from("SG"));
            assert_eq!(
                record.registration_number,
                ManagedBuffer::from("REG-UPGRADE")
            );
            assert_eq!(record.entity_type, ManagedBuffer::from("SPV"));
            assert_eq!(record.kyc_status, ManagedBuffer::from("approved"));
            assert_eq!(record.aml_status, ManagedBuffer::from("clear"));
            assert_eq!(record.investor_class, ManagedBuffer::from("issuer"));
            assert_eq!(record.expiry_round, 1_000u64);
        });
}
