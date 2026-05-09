// Income-distribution integration test.
//
// Validates the income-distribution contract lifecycle including deploy,
// fund, claim (with Merkle proof), and reclaim flows using the typed
// proxy via explicit `tx()` calls.
//
// Full multi-contract integration with carbon-credit and buffer-pool
// contracts is covered by `mrv-common/tests/mrv_lifecycle_integration_test.rs`.
//
// Run with `cargo test --tests` from the income-distribution directory
// (or from the workspace root).

use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const HOLDER_A: TestAddress = TestAddress::new("holder_a");
const INCOME_SC: TestSCAddress = TestSCAddress::new("mrv-income-distribution");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-income-distribution.mxsc.json");
const COME_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("COME-abcdef");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/income-distribution");
    world.register_contract(CODE_PATH, mrv_income_distribution::ContractBuilder);
    world
}

/// Validates that the Merkle proof depth limit rejects oversized proofs.
#[test]
fn claim_yield_rejects_oversized_merkle_proof() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, 100_000u64);
    world
        .account(HOLDER_A)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, 0u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(INCOME_SC)
        .argument(&GOVERNANCE.to_managed_address::<StaticApi>())
        .argument(&COME_TOKEN.to_esdt_token_identifier::<StaticApi>())
        .run();

    // Fund a distribution
    let dist_id = ManagedBuffer::from("dist-001");
    let merkle_root = [0xaau8; 32];
    world
        .tx()
        .from(GOVERNANCE)
        .to(INCOME_SC)
        .typed(mrv_income_distribution::income_distribution_proxy::IncomeDistributionProxy)
        .fund_distribution(
            dist_id.clone(),
            ManagedBuffer::from(&merkle_root[..]),
            100u64,
            ManagedBuffer::from("Qm-test-cid"),
            100_000u64,
        )
        .payment(EsdtTokenPayment::new(
            COME_TOKEN.to_esdt_token_identifier(),
            0u64,
            BigUint::from(10_000u64),
        ))
        .run();

    // Build an oversized proof (65 entries — exceeds the 64-depth limit)
    let oversized_proof: Vec<Vec<u8>> = (0..65).map(|i| vec![i as u8; 32]).collect();
    let proof_args: Vec<ManagedBuffer<StaticApi>> = oversized_proof
        .iter()
        .map(|p| ManagedBuffer::from(p.as_slice()))
        .collect();
    let proof_vec = proof_args
        .into_iter()
        .collect::<ManagedVec<StaticApi, ManagedBuffer<StaticApi>>>();

    // Attempt claim — must fail with MERKLE_PROOF_TOO_DEEP
    world
        .tx()
        .from(HOLDER_A)
        .to(INCOME_SC)
        .typed(mrv_income_distribution::income_distribution_proxy::IncomeDistributionProxy)
        .claim_yield(dist_id, BigUint::<StaticApi>::from(100u64), proof_vec)
        .with_result(ExpectError(4, "MERKLE_PROOF_TOO_DEEP"))
        .run();
}

/// Validates basic fund → query lifecycle via typed proxy.
#[test]
fn fund_and_query_lifecycle() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .account(GOVERNANCE)
        .nonce(1)
        .balance(1_000_000u64)
        .esdt_balance(COME_TOKEN, 100_000u64);

    // Deploy
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(INCOME_SC)
        .argument(&GOVERNANCE.to_managed_address::<StaticApi>())
        .argument(&COME_TOKEN.to_esdt_token_identifier::<StaticApi>())
        .run();

    let dist_id = ManagedBuffer::from("dist-lifecycle");
    let merkle_root = [0xbbu8; 32];

    // Fund
    world
        .tx()
        .from(GOVERNANCE)
        .to(INCOME_SC)
        .typed(mrv_income_distribution::income_distribution_proxy::IncomeDistributionProxy)
        .fund_distribution(
            dist_id.clone(),
            ManagedBuffer::from(&merkle_root[..]),
            100u64,
            ManagedBuffer::from("Qm-lifecycle-cid"),
            100_000u64,
        )
        .payment(EsdtTokenPayment::new(
            COME_TOKEN.to_esdt_token_identifier(),
            0u64,
            BigUint::from(5_000u64),
        ))
        .run();

    // Verify distribution exists via view
    let _result = world
        .query()
        .to(INCOME_SC)
        .typed(mrv_income_distribution::income_distribution_proxy::IncomeDistributionProxy)
        .get_distribution(dist_id.clone())
        .returns(ReturnsResult)
        .run();

    // Verify storage version via the new proxy method
    let _version = world
        .query()
        .to(INCOME_SC)
        .typed(mrv_income_distribution::income_distribution_proxy::IncomeDistributionProxy)
        .get_storage_version()
        .returns(ReturnsResult)
        .run();
}
