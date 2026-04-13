use mrv_gsoc_registry::GsocRegistry;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("gsoc-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-gsoc-registry.mxsc.json");
const GOVERNANCE: TestAddress = TestAddress::new("governance");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/gsoc-registry");
    world.register_contract(CODE_PATH, mrv_gsoc_registry::ContractBuilder);
    world
}

#[test]
fn gsoc_registry_init_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            assert_eq!(sc.total_supply().get(), 0u64);
            assert_eq!(sc.total_retired().get(), 0u64);
        });
}

#[test]
fn gsoc_registry_reserve_and_register_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // Reserve serial
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            let result = sc.reserve_serial(ManagedBuffer::from(b"KE-DH-00001"));
            assert!(result); // first reservation succeeds
        });

    // Duplicate reservation fails
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            let result = sc.reserve_serial(ManagedBuffer::from(b"KE-DH-00001"));
            assert!(!result); // already reserved
        });

    // Register the serial batch
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-00001"),
                ManagedBuffer::from(b"proj-001"),
                2026u32,
                ManagedBuffer::from(b"KE-DH-00001"),
                ManagedBuffer::from(b"KE-DH-00001"),
                1u64,
            );
        });

    // Verify total supply increased
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            assert_eq!(sc.total_supply().get(), 1u64);
        });
}

#[test]
fn gsoc_registry_cancel_reservation_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // Reserve serial
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.reserve_serial(ManagedBuffer::from(b"KE-DH-00002"));
        });

    // Cancel reservation
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.cancel_reservation(ManagedBuffer::from(b"KE-DH-00002"));
        });

    // Serial should be available for re-reservation
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            let result = sc.reserve_serial(ManagedBuffer::from(b"KE-DH-00002"));
            assert!(result); // re-reservation succeeds after cancel
        });
}

#[test]
fn gsoc_registry_retire_serial_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // Reserve + register
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.reserve_serial(ManagedBuffer::from(b"KE-DH-00003"));
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-00003"),
                ManagedBuffer::from(b"proj-001"),
                2026u32,
                ManagedBuffer::from(b"KE-DH-00003"),
                ManagedBuffer::from(b"KE-DH-00003"),
                100u64,
            );
        });

    // Retire
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.record_retirement(
                ManagedBuffer::from(b"KE-DH-00003"),
                ManagedBuffer::from(b"Acme Corp"),
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"0xburn123"),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            assert_eq!(sc.total_retired().get(), 100u64);
        });
}

#[test]
fn gsoc_registry_double_retire_fails_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.reserve_serial(ManagedBuffer::from(b"KE-DH-00004"));
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-00004"),
                ManagedBuffer::from(b"proj-001"),
                2026u32,
                ManagedBuffer::from(b"KE-DH-00004"),
                ManagedBuffer::from(b"KE-DH-00004"),
                50u64,
            );
            sc.record_retirement(
                ManagedBuffer::from(b"KE-DH-00004"),
                ManagedBuffer::from(b"Acme"),
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"0xburn"),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "SERIAL_ALREADY_RETIRED"))
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.record_retirement(
                ManagedBuffer::from(b"KE-DH-00004"),
                ManagedBuffer::from(b"Other"),
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"0xburn2"),
            );
        });
}

#[test]
fn gsoc_registry_register_without_reservation_fails_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // Try to register without reservation
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "SERIAL_NOT_RESERVED: call reserveSerial() first"))
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-UNRESERVED"),
                ManagedBuffer::from(b"proj-001"),
                2026u32,
                ManagedBuffer::from(b"KE-DH-UNRESERVED"),
                ManagedBuffer::from(b"KE-DH-UNRESERVED"),
                10u64,
            );
        });
}

#[test]
fn gsoc_registry_duplicate_registration_fails_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // Reserve and register
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.reserve_serial(ManagedBuffer::from(b"KE-DH-DUP01"));
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-DUP01"),
                ManagedBuffer::from(b"proj-001"),
                2026u32,
                ManagedBuffer::from(b"KE-DH-DUP01"),
                ManagedBuffer::from(b"KE-DH-DUP01"),
                5u64,
            );
        });

    // Attempt to re-reserve the same serial (already registered, not reserved)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            let result = sc.reserve_serial(ManagedBuffer::from(b"KE-DH-DUP01"));
            assert!(!result); // serial already registered, reservation returns false
        });
}

#[test]
fn gsoc_registry_cancel_registered_fails_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.reserve_serial(ManagedBuffer::from(b"KE-DH-CANC01"));
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-CANC01"),
                ManagedBuffer::from(b"proj-001"),
                2026u32,
                ManagedBuffer::from(b"KE-DH-CANC01"),
                ManagedBuffer::from(b"KE-DH-CANC01"),
                10u64,
            );
        });

    // Cancel should fail — serial is registered, not just reserved
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "SERIAL_NOT_RESERVED: cannot cancel a non-reserved serial"))
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.cancel_reservation(ManagedBuffer::from(b"KE-DH-CANC01"));
        });
}

#[test]
fn gsoc_registry_vintage_out_of_range_fails_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.reserve_serial(ManagedBuffer::from(b"KE-DH-VIN01"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "vintage_year out of range"))
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-VIN01"),
                ManagedBuffer::from(b"proj-001"),
                2019u32, // below 2020 range
                ManagedBuffer::from(b"KE-DH-VIN01"),
                ManagedBuffer::from(b"KE-DH-VIN01"),
                1u64,
            );
        });
}

#[test]
fn gsoc_registry_project_serial_count_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // Register two serials for same project
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.reserve_serial(ManagedBuffer::from(b"KE-DH-CNT01"));
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-CNT01"),
                ManagedBuffer::from(b"proj-count"),
                2026u32,
                ManagedBuffer::from(b"KE-DH-CNT01"),
                ManagedBuffer::from(b"KE-DH-CNT01"),
                100u64,
            );
            sc.reserve_serial(ManagedBuffer::from(b"KE-DH-CNT02"));
            sc.register_serial_batch(
                ManagedBuffer::from(b"KE-DH-CNT02"),
                ManagedBuffer::from(b"proj-count"),
                2026u32,
                ManagedBuffer::from(b"KE-DH-CNT02"),
                ManagedBuffer::from(b"KE-DH-CNT02"),
                200u64,
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            assert_eq!(sc.get_project_serials(ManagedBuffer::from(b"proj-count")), 2u64);
            assert_eq!(sc.total_supply().get(), 300u64);
        });
}

#[test]
fn gsoc_registry_retire_unregistered_fails_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "serial not registered"))
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.record_retirement(
                ManagedBuffer::from(b"NONEXISTENT"),
                ManagedBuffer::from(b"Acme"),
                OWNER.to_managed_address(),
                ManagedBuffer::from(b"0xburn"),
            );
        });
}

const VERIFIER: TestAddress = TestAddress::new("verifier");

#[test]
fn gsoc_registry_verifier_lifecycle_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(VERIFIER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Add verifier
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.add_verifier(VERIFIER.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            assert!(sc.is_verifier(VERIFIER.to_managed_address()));
        });

    // Remove verifier (requires governance)
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            sc.remove_verifier(VERIFIER.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_gsoc_registry::contract_obj, |sc| {
            assert!(!sc.is_verifier(VERIFIER.to_managed_address()));
        });
}
