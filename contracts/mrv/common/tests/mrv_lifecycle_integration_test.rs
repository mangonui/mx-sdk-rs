// MRV multi-contract lifecycle integration test.
//
// Validates the tokenized MRV lifecycle across multiple MRV contracts:
//   1. Carbon-credit: issue and burn canonical dVCU supply
//   2. Buffer-pool: mint canonical dVCU-BUF reserve supply
//   3. Reserve-proof-registry: anchor VM0042 proofs only when supplied
//      totals reconcile against the lifecycle-controlled counters

use mrv_buffer_pool::BufferPool;
use mrv_carbon_credit::CarbonCreditModule;
use mrv_reserve_proof_registry::ReserveProofRegistry;
use multiversx_sc::types::{ManagedBuffer, TokenIdentifier};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const UNAUTHORIZED: TestAddress = TestAddress::new("unauthorized");

const CARBON_SC: TestSCAddress = TestSCAddress::new("mrv-carbon-credit");
const BUFFER_SC: TestSCAddress = TestSCAddress::new("mrv-buffer-pool");
const RESERVE_SC: TestSCAddress = TestSCAddress::new("mrv-reserve-proof");

const CARBON_CODE: MxscPath =
    MxscPath::new("mxsc:../../carbon-credit/output/mrv-carbon-credit.mxsc.json");
const BUFFER_CODE: MxscPath =
    MxscPath::new("mxsc:../../buffer-pool/output/mrv-buffer-pool.mxsc.json");
const RESERVE_CODE: MxscPath =
    MxscPath::new("mxsc:../../reserve-proof-registry/output/mrv-reserve-proof-registry.mxsc.json");

const DVCU_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DVCU-123456");
const DGSC_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DGSC-123456");
const BUFFER_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("DVCUBUF-123456");

fn world() -> ScenarioWorld {
    let mut w = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    w.set_current_dir_from_workspace("contracts/mrv/common");
    w.register_contract(CARBON_CODE, mrv_carbon_credit::ContractBuilder);
    w.register_contract(BUFFER_CODE, mrv_buffer_pool::ContractBuilder);
    w.register_contract(RESERVE_CODE, mrv_reserve_proof_registry::ContractBuilder);
    w
}

fn deploy_all() -> ScenarioWorld {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(10_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(10_000_000u64);
    world.account(UNAUTHORIZED).nonce(1).balance(1_000_000u64);

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

    world
}

fn register_ime_and_bundle(
    world: &mut ScenarioWorld,
    project_id: &[u8],
    pai_id: &[u8],
) -> [u8; 32] {
    let bundle_hash: [u8; 32] = [0x44u8; 32];

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let mut domain_codes = MultiValueEncoded::new();
            domain_codes.push(ManagedBuffer::from(b"SG"));
            sc.register_ime_record(
                ManagedBuffer::from(project_id),
                ManagedBuffer::from(b"sha256:image"),
                ManagedBuffer::from(b"sha256:param"),
                ManagedBuffer::from(b"sha256:cal"),
                ManagedBuffer::from(b"sha256:strata"),
                ManagedBuffer::from(b"1.0.0"),
                9_999_999_999u64,
                domain_codes,
            );
            sc.register_committed_bundle(
                ManagedBuffer::from(pai_id),
                1u64,
                ManagedBuffer::from(&bundle_hash[..]),
            );
        });

    bundle_hash
}

fn issue_vm0042(
    world: &mut ScenarioWorld,
    project_id: &[u8],
    lot_id: &[u8],
    pai_id: &[u8],
    gross: u64,
    buffer_bps: u64,
) {
    let bundle_hash = register_ime_and_bundle(world, project_id, pai_id);

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.issue_credits(
                ManagedBuffer::from(project_id),
                ManagedBuffer::from(lot_id),
                ManagedBuffer::from(pai_id),
                1u64,
                ManagedBuffer::from(b"SG"),
                BigUint::from(gross),
                buffer_bps,
                mrv_carbon_credit::ExecutionBundleRef {
                    science_service_image_digest: ManagedBuffer::from(b"sha256:image"),
                    parameter_pack_hash: ManagedBuffer::from(b"sha256:param"),
                    calibration_dataset_hash: ManagedBuffer::from(b"sha256:cal"),
                    strata_protocol_hash: ManagedBuffer::from(b"sha256:strata"),
                    methodology_version: ManagedBuffer::from(b"1.0.0"),
                },
                ManagedBuffer::from(&bundle_hash[..]),
                OWNER.to_managed_address(),
            );
        });
}

#[test]
fn mrv_carbon_credit_lifecycle() {
    let mut world = deploy_all();
    let merkle_root: [u8; 32] = [0xA1u8; 32];

    issue_vm0042(
        &mut world,
        b"project-001",
        b"lot-001",
        b"pai-001",
        100_000u64,
        500u64,
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.confirm_buffer_deposit(
                ManagedBuffer::from(b"project-001"),
                ManagedBuffer::from(b"pai-001"),
                1u64,
            );
        });

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                BigUint::from(95_000u64),
                BigUint::from(5_000u64),
                BigUint::from(0u64),
                ManagedBuffer::from(&merkle_root[..]),
                777u64,
            );
        },
    );

    world
        .query()
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            assert_eq!(sc.total_dvcu_minted().get(), BigUint::from(95_000u64));
            assert_eq!(sc.total_dvcu_burned().get(), BigUint::zero());
        });

    world
        .query()
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            assert_eq!(sc.total_buffer_minted().get(), BigUint::from(5_000u64));
            assert_eq!(sc.total_buffer_burned().get(), BigUint::zero());
            assert_eq!(sc.total_pool_balance().get(), BigUint::from(5_000u64));
        });

    world
        .query()
        .to(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let proof = sc
                .get_latest_reserve_proof(ManagedBuffer::from(DVCU_TOKEN.as_bytes()))
                .into_option()
                .expect("latest proof should exist");
            assert_eq!(proof.total_supply_scaled, BigUint::from(95_000u64));
            assert_eq!(proof.total_buffer_scaled, BigUint::from(5_000u64));
            assert_eq!(proof.total_retired_scaled, BigUint::zero());
            assert_eq!(proof.net_circulating_scaled, BigUint::from(90_000u64));
        });
}

#[test]
fn mrv_confirm_buffer_deposit_atomically_deposits_and_clears_pending() {
    let mut world = deploy_all();

    issue_vm0042(
        &mut world,
        b"project-c13",
        b"lot-c13",
        b"pai-c13",
        100_000u64,
        500u64,
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.confirm_buffer_deposit(
                ManagedBuffer::from(b"project-c13"),
                ManagedBuffer::from(b"pai-c13"),
                1u64,
            );
        });

    world
        .query()
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            let key = (
                ManagedBuffer::from(b"project-c13"),
                ManagedBuffer::from(b"pai-c13"),
                mrv_common::period_key(1u64),
            );
            assert!(
                !sc.pending_buffer_deposits().contains_key(&key),
                "pending buffer deposit must only clear after buffer-pool deposit succeeds"
            );
        });

    world
        .query()
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            let record = sc
                .get_buffer_record(ManagedBuffer::from(b"project-c13"))
                .into_option()
                .expect("buffer deposit must create a project record");
            assert_eq!(record.total_deposited, BigUint::from(5_000u64));
            assert_eq!(record.total_cancelled, BigUint::zero());
            assert_eq!(record.total_replenished, BigUint::zero());
            assert_eq!(sc.total_buffer_minted().get(), BigUint::from(5_000u64));
            assert_eq!(sc.total_pool_balance().get(), BigUint::from(5_000u64));
        });
}

#[test]
fn mrv_carbon_credit_retirement_and_reserve_proof_flow() {
    let mut world = deploy_all();
    let merkle_root: [u8; 32] = [0xB2u8; 32];

    issue_vm0042(
        &mut world,
        b"project-002",
        b"lot-002",
        b"pai-002",
        100_000u64,
        500u64,
    );

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.confirm_buffer_deposit(
                ManagedBuffer::from(b"project-002"),
                ManagedBuffer::from(b"pai-002"),
                1u64,
            );
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(CARBON_SC)
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.initiate_retirement(
                ManagedBuffer::from(b"ret-002"),
                ManagedBuffer::from(b"lot-002"),
                ManagedBuffer::from(b"project-002"),
                BigUint::from(20_000u64),
                OWNER.to_managed_address(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(CARBON_SC)
        .payment(Payment::try_new(DVCU_TOKEN, 0, 20_000u64).unwrap())
        .whitebox(mrv_carbon_credit::contract_obj, |sc| {
            sc.confirm_retirement_burn(
                ManagedBuffer::from(b"ret-002"),
                ManagedBuffer::from(b"burn-hash-002"),
            );
        });

    world.tx().from(GOVERNANCE).to(RESERVE_SC).whitebox(
        mrv_reserve_proof_registry::contract_obj,
        |sc| {
            sc.anchor_reserve_proof(
                ManagedBuffer::from(DVCU_TOKEN.as_bytes()),
                BigUint::from(75_000u64),
                BigUint::from(5_000u64),
                BigUint::from(20_000u64),
                ManagedBuffer::from(&merkle_root[..]),
                888u64,
            );
        },
    );

    world
        .query()
        .to(RESERVE_SC)
        .whitebox(mrv_reserve_proof_registry::contract_obj, |sc| {
            let proof = sc
                .get_latest_reserve_proof(ManagedBuffer::from(DVCU_TOKEN.as_bytes()))
                .into_option()
                .expect("latest proof should exist");
            assert_eq!(proof.total_supply_scaled, BigUint::from(75_000u64));
            assert_eq!(proof.total_buffer_scaled, BigUint::from(5_000u64));
            assert_eq!(proof.total_retired_scaled, BigUint::from(20_000u64));
            assert_eq!(proof.net_circulating_scaled, BigUint::from(50_000u64));
            assert_eq!(proof.snapshot_block, 888u64);
        });
}

#[test]
fn mrv_buffer_pool_deposit_and_replenishment_flow() {
    let mut world = deploy_all();

    world
        .tx()
        .from(CARBON_SC)
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.deposit_buffer_credits(
                ManagedBuffer::from(b"PROJECT-ALPHA"),
                BigUint::from(100_000u64),
                1u64,
            );
        });

    world.current_block().block_epoch(1_500u64);

    world
        .tx()
        .from(CARBON_SC)
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.replenish_buffer_credits(
                ManagedBuffer::from(b"PROJECT-ALPHA"),
                BigUint::from(10_000u64),
                ManagedBuffer::from(b"justification-cid-001"),
            );
        });

    world
        .query()
        .to(BUFFER_SC)
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            let record = sc
                .buffer_records()
                .get(&ManagedBuffer::from(b"PROJECT-ALPHA"))
                .expect("buffer record should exist");
            assert_eq!(record.total_deposited, BigUint::from(100_000u64));
            assert_eq!(record.total_replenished, BigUint::from(10_000u64));
            assert_eq!(sc.total_buffer_minted().get(), BigUint::from(110_000u64));
            assert_eq!(sc.total_pool_balance().get(), BigUint::from(110_000u64));
        });
}

#[test]
fn mrv_lifecycle_auth_boundaries() {
    let mut world = deploy_all();

    world
        .tx()
        .from(UNAUTHORIZED)
        .to(BUFFER_SC)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(mrv_buffer_pool::contract_obj, |sc| {
            sc.add_authorized_caller(UNAUTHORIZED.to_managed_address());
        });
}
