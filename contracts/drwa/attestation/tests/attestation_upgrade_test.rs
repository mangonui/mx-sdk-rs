use multiversx_sc_scenario::imports::*;

use drwa_attestation::DrwaAttestation;
use drwa_attestation::drwa_attestation_proxy::DrwaAttestationProxy;

const OWNER: TestAddress = TestAddress::new("owner");
const AUDITOR: TestAddress = TestAddress::new("auditor");
const SUBJECT: TestAddress = TestAddress::new("subject");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-attestation-upgrade");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-attestation.mxsc.json");
const TOKEN_ID: &str = "CARBON-ab12cd";

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    blockchain.set_current_dir_from_workspace("contracts/drwa/attestation");
    blockchain.register_contract(CODE_PATH, drwa_attestation::ContractBuilder);
    blockchain
}

#[test]
fn attestation_upgrade_preserves_auditor_record_and_storage_version() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(AUDITOR).nonce(1).balance(1_000_000u64);
    world.account(SUBJECT).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .typed(DrwaAttestationProxy)
        .init(AUDITOR)
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .run();

    world
        .tx()
        .from(AUDITOR)
        .to(SC_ADDRESS)
        .typed(DrwaAttestationProxy)
        .record_attestation(
            TOKEN_ID,
            SUBJECT.to_managed_address(),
            "MRV",
            "hash-upgrade",
            true,
        )
        .run();

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .typed(DrwaAttestationProxy)
        .upgrade()
        .code(CODE_PATH)
        .run();

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_attestation::contract_obj, |sc| {
            assert_eq!(sc.storage_version().get(), 1);

            let record = sc
                .attestation(
                    &ManagedBuffer::from(TOKEN_ID),
                    &SUBJECT.to_managed_address(),
                )
                .get();
            assert_eq!(record.subject, SUBJECT.to_managed_address());
            assert_eq!(record.evidence_hash, ManagedBuffer::from("hash-upgrade"));
            assert!(record.approved);
        });
}
