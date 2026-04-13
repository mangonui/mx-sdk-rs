use mrv_reserve_proof_registry::ReserveProofRegistry;
use multiversx_sc::types::{BigUint, ManagedBuffer};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("reserve-proof-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-reserve-proof-registry.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/reserve-proof-registry");
    world.register_contract(CODE_PATH, mrv_reserve_proof_registry::ContractBuilder);
    world
}

#[test]
fn reserve_proof_init_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });
}

#[test]
fn reserve_proof_anchor_and_retrieve_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // Anchor a VM0042 reserve proof
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(b"CARBON-abc123"),
                BigUint::from(100_000u64),  // total supply
                BigUint::from(10_000u64),   // buffer
                BigUint::from(5_000u64),    // retired
                ManagedBuffer::from(b"merkle-root-001"),
                1000u64,  // snapshot_block
            );
        });

    // Retrieve and verify
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let proof = sc
                .get_latest_reserve_proof(ManagedBuffer::from(b"CARBON-abc123"))
                .into_option()
                .expect("proof should exist");
            assert_eq!(proof.snapshot_block, 1000u64);
            assert_eq!(proof.net_circulating_scaled, BigUint::from(85_000u64));
        });
}

#[test]
fn reserve_proof_monotonic_block_guard_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // Anchor at block 1000
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(b"TOK-001"),
                BigUint::from(50_000u64),
                BigUint::from(5_000u64),
                BigUint::from(0u64),
                ManagedBuffer::from(b"root-1"),
                1000u64,
            );
        });

    // Attempt to anchor at block 999 (backward) — must fail
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "SNAPSHOT_BLOCK_NOT_MONOTONIC: new block must be greater than current latest"))
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(b"TOK-001"),
                BigUint::from(50_000u64),
                BigUint::from(5_000u64),
                BigUint::from(0u64),
                ManagedBuffer::from(b"root-old"),
                999u64,
            );
        });

    // Anchor at block 2000 (forward) — must succeed
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(b"TOK-001"),
                BigUint::from(60_000u64),
                BigUint::from(6_000u64),
                BigUint::from(1_000u64),
                ManagedBuffer::from(b"root-2"),
                2000u64,
            );
        });
}

#[test]
fn reserve_proof_arithmetic_guard_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    // supply < buffer + retired — must fail
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "INVALID_RESERVE_ARITHMETIC: supply < buffer + retired"))
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(b"TOK-002"),
                BigUint::from(100u64),    // supply = 100
                BigUint::from(60u64),     // buffer = 60
                BigUint::from(50u64),     // retired = 50 → total 110 > 100
                ManagedBuffer::from(b"root-bad"),
                1000u64,
            );
        });
}

#[test]
fn gsoc_reserve_proof_anchor_rs() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.init(OWNER.to_managed_address());
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.anchor_gsoc_reserve_proof(
                ManagedBuffer::from(b"proj-001"),
                10000u64,  // total issued
                2000u64,   // total retired
                100u64,    // serial count
                ManagedBuffer::from(b"itmo-hash-001"),
                500u64,    // snapshot_block
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let proof = sc
                .get_latest_gsoc_reserve_proof(ManagedBuffer::from(b"proj-001"))
                .into_option()
                .expect("gsoc proof should exist");
            assert_eq!(proof.net_active, 8000u64);
            assert_eq!(proof.serial_count, 100u64);
        });
}
