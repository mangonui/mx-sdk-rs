use mrv_buffer_pool::BufferPool;
use mrv_carbon_credit::CarbonCreditModule;
use mrv_reserve_proof_registry::ReserveProofRegistry;
use multiversx_sc::types::{ManagedBuffer, TokenIdentifier};
use multiversx_sc_scenario::imports::*;
use std::cell::RefCell;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");

const CARBON_SC: TestSCAddress = TestSCAddress::new("mrv-carbon-credit");
const BUFFER_SC: TestSCAddress = TestSCAddress::new("mrv-buffer-pool");
const RESERVE_SC: TestSCAddress = TestSCAddress::new("mrv-reserve-proof");

const CARBON_CODE: MxscPath =
    MxscPath::new("mxsc:../../carbon-credit/output/mrv-carbon-credit.mxsc.json");
const BUFFER_CODE: MxscPath =
    MxscPath::new("mxsc:../../buffer-pool/output/mrv-buffer-pool.mxsc.json");
const RESERVE_CODE: MxscPath = MxscPath::new("mxsc:output/mrv-reserve-proof-registry.mxsc.json");

const DVCU_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DVCU-123456");
const DGSC_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DGSC-123456");
const BUFFER_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DVCUBUF-123456");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/reserve-proof-registry");
    world.register_contract(CARBON_CODE, mrv_carbon_credit::ContractBuilder);
    world.register_contract(BUFFER_CODE, mrv_buffer_pool::ContractBuilder);
    world.register_contract(RESERVE_CODE, mrv_reserve_proof_registry::ContractBuilder);
    world
}

fn deploy_runtime(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CARBON_CODE)
        .new_address(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                BUFFER_SC.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(BUFFER_CODE)
        .new_address(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.init(
                GOVERNANCE.to_managed_address(),
                CARBON_SC.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(RESERVE_CODE)
        .new_address(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world.set_esdt_local_roles(
        CARBON_SC.to_address(),
        DVCU_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );
    world.set_esdt_local_roles(
        CARBON_SC.to_address(),
        DGSC_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );
    world.set_esdt_local_roles(
        BUFFER_SC.to_address(),
        BUFFER_TOKEN.as_bytes(),
        &[EsdtLocalRole::Mint, EsdtLocalRole::Burn],
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.set_dvcu_token_id(TokenIdentifier::from(DVCU_TOKEN.as_bytes()));
            sc.set_dgsc_token_id(TokenIdentifier::from(DGSC_TOKEN.as_bytes()));
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.set_buffer_token_id(TokenIdentifier::from(BUFFER_TOKEN.as_bytes()));
        });

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.set_carbon_credit_addr(CARBON_SC.to_managed_address());
            sc.set_buffer_pool_addr(BUFFER_SC.to_managed_address());
        },
    );
}

fn seed_vm0042_runtime(world: &mut ScenarioWorld) {
    deploy_runtime(world);

    let bundle_hash: [u8; 32] = [0x21u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            domain_codes.push(ManagedBuffer::from(b"KE"));

            sc.register_ime_record(
                ManagedBuffer::from(b"project-int-001"),
                ManagedBuffer::from(b"sha256:image-int-001"),
                ManagedBuffer::from(b"sha256:param-int-001"),
                ManagedBuffer::from(b"sha256:cal-int-001"),
                ManagedBuffer::from(b"sha256:strata-int-001"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes,
            );
            sc.register_committed_bundle(
                ManagedBuffer::from(b"pai-int-001"),
                1u64,
                ManagedBuffer::from(&bundle_hash[..]),
            );
            sc.issue_credits(
                ManagedBuffer::from(b"project-int-001"),
                ManagedBuffer::from(b"lot-int-001"),
                ManagedBuffer::from(b"pai-int-001"),
                1u64,
                ManagedBuffer::from(b"KE"),
                BigUint::from(1_000u64),
                100u64,
                mrv_carbon_credit::ExecutionBundleRef {
                    science_service_image_digest: ManagedBuffer::from(b"sha256:image-int-001"),
                    parameter_pack_hash: ManagedBuffer::from(b"sha256:param-int-001"),
                    calibration_dataset_hash: ManagedBuffer::from(b"sha256:cal-int-001"),
                    strata_protocol_hash: ManagedBuffer::from(b"sha256:strata-int-001"),
                    methodology_version: ManagedBuffer::from(b"1.0.0"),
                },
                ManagedBuffer::from(&bundle_hash[..]),
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(CARBON_SC)
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.deposit_buffer_credits(
                ManagedBuffer::from(b"project-int-001"),
                BigUint::from(10u64),
                1u64,
            );
        });
}

#[test]
fn reserve_proof_dvcu_and_gsoc_dual_track_rs() {
    let mut world = world();
    seed_vm0042_runtime(&mut world);
    let canonical_hash_cell: RefCell<[u8; 32]> = RefCell::new([0u8; 32]);

    let merkle_root = [0x11u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_reserve_proof(
            ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
            BigUint::<StaticApi>::from(990u64),
            BigUint::<StaticApi>::from(10u64),
            BigUint::<StaticApi>::zero(),
            ManagedBuffer::from(&merkle_root[..]),
            100u64,
        )
        .run();

    let itmo_hash = [0x22u8; 32];
    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.register_gsoc_bundle(
                ManagedBuffer::from(b"pai-gsoc-int"),
                1u64,
                ManagedBuffer::from(&itmo_hash[..]),
            );
            sc.add_approved_gsoc_verifier(GOVERNANCE.to_managed_address());
            sc.issue_gsoc_credits(
                ManagedBuffer::from(b"GSOC-KE-001"),
                ManagedBuffer::from(b"pai-gsoc-int"),
                1u64,
                ManagedBuffer::from(&itmo_hash[..]),
                GOVERNANCE.to_managed_address(),
                ManagedBuffer::from(b"dna-gsoc-int"),
                ManagedBuffer::from(b"ITMO-GSOC-INT-001"),
                BigUint::from(500u64),
                100u64,
                GOVERNANCE.to_managed_address(),
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .payment(Payment::try_new(DGSC_TOKEN, 0, 50u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.burn_and_retire_gsoc(
                ManagedBuffer::from(b"ITMO-GSOC-INT-001"),
                BigUint::from(50u64),
                ManagedBuffer::from(b"Beneficiary"),
                GOVERNANCE.to_managed_address(),
            );

            let canonical_hash =
                sc.get_canonical_gsoc_serial_inventory_hash(ManagedBuffer::from(b"GSOC-KE-001"));
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(canonical_hash.to_boxed_bytes().as_slice());
            *canonical_hash_cell.borrow_mut() = bytes;
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_gsoc_reserve_proof(
            ManagedBuffer::from(b"GSOC-KE-001"),
            495u64,
            50u64,
            1u64,
            ManagedBuffer::from(&canonical_hash_cell.borrow()[..]),
            100u64,
        )
        .run();
}

#[test]
fn reserve_proof_monotonic_block_guard_rs() {
    let mut world = world();
    seed_vm0042_runtime(&mut world);

    let merkle_root = [0x33u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_reserve_proof(
            ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
            BigUint::<StaticApi>::from(990u64),
            BigUint::<StaticApi>::from(10u64),
            BigUint::<StaticApi>::zero(),
            ManagedBuffer::from(&merkle_root[..]),
            100u64,
        )
        .run();

    world
        .tx()
        .from(GOVERNANCE)
        .to(RESERVE_SC)
        .typed(mrv_reserve_proof_registry::reserve_proof_registry_proxy::ReserveProofRegistryProxy)
        .anchor_reserve_proof(
            ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
            BigUint::<StaticApi>::from(990u64),
            BigUint::<StaticApi>::from(10u64),
            BigUint::<StaticApi>::zero(),
            ManagedBuffer::from(&merkle_root[..]),
            50u64,
        )
        .with_result(ExpectError(
            4u64,
            "SNAPSHOT_BLOCK_NOT_MONOTONIC: new block must be greater than current latest",
        ))
        .run();
}
