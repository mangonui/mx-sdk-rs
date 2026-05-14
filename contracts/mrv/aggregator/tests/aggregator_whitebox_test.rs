use ed25519_dalek::{Signer, SigningKey};
use mrv_aggregator::MrvAggregator;
use mrv_common::MrvGovernanceModule;
use mrv_governance::MrvGovernance;
use multiversx_sc::api::ManagedTypeApi;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const ORACLE_ONE: TestAddress = TestAddress::new("oracle-one");
const ORACLE_TWO: TestAddress = TestAddress::new("oracle-two");
const SIGNER_ONE: TestAddress = TestAddress::new("signer-one");
const SIGNER_TWO: TestAddress = TestAddress::new("signer-two");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("mrv-aggregator");
const GOVERNANCE_SC: TestSCAddress = TestSCAddress::new("mrv-governance");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/mrv-aggregator.mxsc.json");
const GOVERNANCE_CODE: MxscPath =
    MxscPath::new("mxsc:../../governance/output/mrv-governance.mxsc.json");
const TEST_DEVICE_SECRET: [u8; 32] = [7u8; 32];

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/mrv/aggregator");
    world.register_contract(CODE_PATH, mrv_aggregator::ContractBuilder);
    world.register_contract(GOVERNANCE_CODE, mrv_governance::ContractBuilder);
    world
}

fn test_device_public_key<M: ManagedTypeApi>() -> ManagedBuffer<M> {
    let signing_key = SigningKey::from_bytes(&TEST_DEVICE_SECRET);
    ManagedBuffer::from(signing_key.verifying_key().as_bytes())
}

fn iot_signature<M: ManagedTypeApi>(
    device: TestAddress,
    pai_id: &[u8],
    period_start: u64,
    period_end: u64,
    data_cid: &[u8],
    source_timestamp: u64,
) -> ManagedBuffer<M>
where
    M: ManagedTypeApi,
{
    let signing_key = SigningKey::from_bytes(&TEST_DEVICE_SECRET);
    let payload = oracle_reading_signature_payload(
        device,
        pai_id,
        period_start,
        period_end,
        0u8,
        data_cid,
        source_timestamp,
    );
    ManagedBuffer::from(signing_key.sign(&payload).to_bytes().as_slice())
}

fn oracle_reading_signature_payload(
    device: TestAddress,
    pai_id: &[u8],
    period_start: u64,
    period_end: u64,
    source: u8,
    data_cid: &[u8],
    source_timestamp: u64,
) -> Vec<u8> {
    oracle_reading_signature_payload_for_sc(
        SC_ADDRESS.eval_to_array().as_slice(),
        device,
        pai_id,
        period_start,
        period_end,
        source,
        data_cid,
        source_timestamp,
    )
}

fn oracle_reading_signature_payload_for_sc(
    sc_address: &[u8],
    device: TestAddress,
    pai_id: &[u8],
    period_start: u64,
    period_end: u64,
    source: u8,
    data_cid: &[u8],
    source_timestamp: u64,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"mrv_oracle_reading_v2");
    payload.push(0);
    append_len_prefixed(&mut payload, sc_address);
    append_len_prefixed(&mut payload, pai_id);
    payload.extend_from_slice(&period_start.to_be_bytes());
    payload.extend_from_slice(&period_end.to_be_bytes());
    payload.push(source);
    append_len_prefixed(&mut payload, data_cid);
    payload.extend_from_slice(&source_timestamp.to_be_bytes());
    append_len_prefixed(&mut payload, &device.eval_to_array());
    payload
}

fn iot_signature_for_sc<M: ManagedTypeApi>(
    sc_address: &[u8],
    device: TestAddress,
    pai_id: &[u8],
    period_start: u64,
    period_end: u64,
    data_cid: &[u8],
    source_timestamp: u64,
) -> ManagedBuffer<M>
where
    M: ManagedTypeApi,
{
    let signing_key = SigningKey::from_bytes(&TEST_DEVICE_SECRET);
    let payload = oracle_reading_signature_payload_for_sc(
        sc_address,
        device,
        pai_id,
        period_start,
        period_end,
        0u8,
        data_cid,
        source_timestamp,
    );
    ManagedBuffer::from(signing_key.sign(&payload).to_bytes().as_slice())
}

fn append_len_prefixed(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u64).to_be_bytes());
    out.extend_from_slice(value);
}

#[test]
fn aggregator_init_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert_eq!(sc.quorum().get(), 2u32);
            assert_eq!(sc.iot_window().get(), 172800u64);
            assert_eq!(sc.satellite_window().get(), 864000u64);
            assert_eq!(sc.govt_lab_window().get(), 2592000u64);
            assert_eq!(sc.divergence_threshold_bps().get(), 3000u64);
        });
}

#[test]
fn aggregator_submit_oracle_reading_and_try_seal_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    // Register oracles before submitting readings
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            // Register devices so submit_oracle_reading passes DEVICE_NOT_REGISTERED guard
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
            sc.register_device_public_key(
                ORACLE_TWO.to_managed_address(),
                test_device_public_key(),
            );
        });

    // Set block timestamp so oracle readings are not rejected as FUTURE_TIMESTAMP
    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    // Submit IoT reading (source=0)
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-001"),
                1_710_600_000u64, // period_start
                1_710_720_000u64, // period_end
                0u8,              // SOURCE_IOT
                ManagedBuffer::from(b"bafyiot001"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-001",
                    1_710_600_000u64,
                    1_710_720_000u64,
                    b"bafyiot001",
                    1_710_719_000u64,
                ),
            );
        });

    // Submit Satellite reading (source=1)
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-001"),
                1_710_600_000u64, // period_start
                1_710_720_000u64, // period_end
                1u8,              // SOURCE_SATELLITE
                ManagedBuffer::from(b"bafysat001"),
                1_710_710_000u64,
                ORACLE_TWO.to_managed_address(),
                ManagedBuffer::new(), // device_signature (empty OK for non-IoT)
            );
        });

    // Set block timestamp past period_end
    world
        .current_block()
        .block_timestamp_seconds(1_710_720_001u64);

    // Acknowledge semantic discrepancy (IoT != Satellite CIDs)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.acknowledge_discrepancy(
                ManagedBuffer::from(b"pai-001"),
                1_710_720_000u64,
                ManagedBuffer::from(b"vvb-ack-cid-001"),
            );
        });

    // Seal with quorum=2 met
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-001"), 1_710_720_000u64);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert!(sc.is_sealed(ManagedBuffer::from(b"pai-001"), 1_710_720_000u64));
            let sealed = sc
                .get_sealed_event(ManagedBuffer::from(b"pai-001"), 1_710_720_000u64)
                .into_option()
                .unwrap();
            assert_eq!(sealed.reading_count, 2u32);
            // IoT CID != Satellite CID => semantic_discrepancy = true
            assert!(sealed.semantic_discrepancy);
        });
}

#[test]
fn aggregator_rejects_reading_before_period_start_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "reading predates period"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-predates-period"),
                1_710_600_000u64,
                1_710_720_000u64,
                0u8,
                ManagedBuffer::from(b"bafyiot-predates-period"),
                1_710_599_999u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-predates-period",
                    1_710_600_000u64,
                    1_710_720_000u64,
                    b"bafyiot-predates-period",
                    1_710_599_999u64,
                ),
            );
        });
}

#[test]
fn aggregator_mrv_root_binds_pai_id_and_period_end_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
            sc.register_device_public_key(
                ORACLE_TWO.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_900_000u64);

    // Seal A: pai-a / period-1.
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-root-a"),
                1_710_600_000u64,
                1_710_720_000u64,
                0u8,
                ManagedBuffer::from(b"cid-shared-001"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-root-a",
                    1_710_600_000u64,
                    1_710_720_000u64,
                    b"cid-shared-001",
                    1_710_719_000u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-root-a"),
                1_710_600_000u64,
                1_710_720_000u64,
                1u8,
                ManagedBuffer::from(b"cid-shared-001"),
                1_710_710_000u64,
                ORACLE_TWO.to_managed_address(),
                ManagedBuffer::new(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-root-a"), 1_710_720_000u64);
        });

    // Seal B: same readings, different pai_id.
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-root-b"),
                1_710_600_000u64,
                1_710_720_000u64,
                0u8,
                ManagedBuffer::from(b"cid-shared-001"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-root-b",
                    1_710_600_000u64,
                    1_710_720_000u64,
                    b"cid-shared-001",
                    1_710_719_000u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-root-b"),
                1_710_600_000u64,
                1_710_720_000u64,
                1u8,
                ManagedBuffer::from(b"cid-shared-001"),
                1_710_710_000u64,
                ORACLE_TWO.to_managed_address(),
                ManagedBuffer::new(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-root-b"), 1_710_720_000u64);
        });

    // Seal C: same readings, same pai_id as A, different period_end.
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-root-a"),
                1_710_720_001u64,
                1_710_820_000u64,
                0u8,
                ManagedBuffer::from(b"cid-shared-001"),
                1_710_819_000u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-root-a",
                    1_710_720_001u64,
                    1_710_820_000u64,
                    b"cid-shared-001",
                    1_710_819_000u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-root-a"),
                1_710_720_001u64,
                1_710_820_000u64,
                1u8,
                ManagedBuffer::from(b"cid-shared-001"),
                1_710_810_000u64,
                ORACLE_TWO.to_managed_address(),
                ManagedBuffer::new(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-root-a"), 1_710_820_000u64);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            let sealed_a = sc
                .get_sealed_event(ManagedBuffer::from(b"pai-root-a"), 1_710_720_000u64)
                .into_option()
                .expect("sealed A should exist");
            let sealed_b = sc
                .get_sealed_event(ManagedBuffer::from(b"pai-root-b"), 1_710_720_000u64)
                .into_option()
                .expect("sealed B should exist");
            let sealed_c = sc
                .get_sealed_event(ManagedBuffer::from(b"pai-root-a"), 1_710_820_000u64)
                .into_option()
                .expect("sealed C should exist");

            assert_ne!(sealed_a.mrv_root, sealed_b.mrv_root);
            assert_ne!(sealed_a.mrv_root, sealed_c.mrv_root);
        });
}

#[test]
fn aggregator_rejects_seal_below_quorum_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    // Set block timestamp so oracle readings are not rejected as FUTURE_TIMESTAMP
    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    // Submit only 1 reading — below quorum of 2
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-002"),
                1_710_600_000u64, // period_start
                1_710_720_000u64, // period_end
                0u8,
                ManagedBuffer::from(b"bafyiot002"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-002",
                    1_710_600_000u64,
                    1_710_720_000u64,
                    b"bafyiot002",
                    1_710_719_000u64,
                ),
            );
        });

    // Set timestamp past period_end so we reach the quorum check
    world
        .current_block()
        .block_timestamp_seconds(1_710_720_001u64);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "insufficient oracle readings for quorum"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-002"), 1_710_720_000u64);
        });
}

#[test]
fn aggregator_force_seal_after_timeout_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
            sc.register_device_public_key(
                ORACLE_TWO.to_managed_address(),
                test_device_public_key(),
            );
        });

    // Set block timestamp so oracle readings are not rejected as FUTURE_TIMESTAMP
    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    // Submit the configured quorum of 2 non-discrepant readings.
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-003"),
                period_end - 200_000u64, // period_start
                period_end,
                0u8, // SOURCE_IOT
                ManagedBuffer::from(b"bafyiot003"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-003",
                    period_end - 200_000u64,
                    period_end,
                    b"bafyiot003",
                    period_end - 100u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-003"),
                period_end - 200_000u64,
                period_end,
                1u8, // SOURCE_SATELLITE
                ManagedBuffer::from(b"bafyiot003"),
                period_end - 10_000u64,
                ORACLE_TWO.to_managed_address(),
                ManagedBuffer::new(),
            );
        });

    // Set block timestamp past period_end + govt_lab_window (2592000)
    world
        .current_block()
        .block_timestamp_seconds(period_end + 2_592_001u64);

    // Force seal should succeed with configured quorum and non-discrepant readings after timeout.
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.force_seal_after_timeout(ManagedBuffer::from(b"pai-003"), period_end);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert!(sc.is_sealed(ManagedBuffer::from(b"pai-003"), period_end));
            let sealed = sc
                .get_sealed_event(ManagedBuffer::from(b"pai-003"), period_end)
                .into_option()
                .unwrap();
            assert_eq!(sealed.reading_count, 2u32);
            assert!(!sealed.semantic_discrepancy);
        });
}

#[test]
fn aggregator_force_seal_after_timeout_requires_configured_quorum_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(3u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
            sc.register_device_public_key(
                ORACLE_TWO.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-force-quorum"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"bafyforcequorum"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-force-quorum",
                    period_end - 200_000u64,
                    period_end,
                    b"bafyforcequorum",
                    period_end - 100u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-force-quorum"),
                period_end - 200_000u64,
                period_end,
                1u8,
                ManagedBuffer::from(b"bafyforcequorum"),
                period_end - 10_000u64,
                ORACLE_TWO.to_managed_address(),
                ManagedBuffer::new(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 2_592_001u64);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "insufficient oracle readings for configured quorum",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.force_seal_after_timeout(ManagedBuffer::from(b"pai-force-quorum"), period_end);
        });
}

#[test]
fn aggregator_force_seal_after_timeout_rejects_discrepant_readings_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-003-discrepant"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"bafyiot003"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-003-discrepant",
                    period_end - 200_000u64,
                    period_end,
                    b"bafyiot003",
                    period_end - 100u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-003-discrepant"),
                period_end - 200_000u64,
                period_end,
                1u8,
                ManagedBuffer::from(b"bafysat003"),
                period_end - 10_000u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 2_592_001u64);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "TIMEOUT_FORCE_SEAL_REQUIRES_NON_DISCREPANT_IOT_SATELLITE: cannot force-seal missing or divergent IoT/Satellite readings",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.force_seal_after_timeout(
                ManagedBuffer::from(b"pai-003-discrepant"),
                period_end,
            );
        });
}

#[test]
fn aggregator_force_seal_after_timeout_rejects_single_reading_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-003-single"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"bafyiot003single"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-003-single",
                    period_end - 200_000u64,
                    period_end,
                    b"bafyiot003single",
                    period_end - 100u64,
                ),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 2_592_001u64);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "insufficient oracle readings for configured quorum",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.force_seal_after_timeout(ManagedBuffer::from(b"pai-003-single"), period_end);
        });
}

#[test]
fn aggregator_force_seal_before_timeout_fails_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    // Set block timestamp so oracle readings are not rejected as FUTURE_TIMESTAMP
    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-004"),
                period_end - 200_000u64, // period_start
                period_end,
                0u8,
                ManagedBuffer::from(b"bafyiot004"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-004",
                    period_end - 200_000u64,
                    period_end,
                    b"bafyiot004",
                    period_end - 100u64,
                ),
            );
        });

    // Set timestamp after period_end but before timeout window
    world
        .current_block()
        .block_timestamp_seconds(period_end + 1_000u64);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "timeout window has not elapsed \u{2014} wait for coherence window to expire",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.force_seal_after_timeout(ManagedBuffer::from(b"pai-004"), period_end);
        });
}

const NEW_ORACLE: TestAddress = TestAddress::new("new-oracle");

#[test]
fn aggregator_oracle_rotation_lifecycle_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(NEW_ORACLE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
        });

    // Propose oracle rotation
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.propose_oracle_update(
                ORACLE_ONE.to_managed_address(),
                NEW_ORACLE.to_managed_address(),
                100_000u64,
            );
        });

    // New oracle accepts
    world
        .tx()
        .from(NEW_ORACLE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.accept_oracle_update(ORACLE_ONE.to_managed_address());
        });

    // Verify old oracle is deregistered and new one is active
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert!(!sc.is_oracle_authorized(ORACLE_ONE.to_managed_address()));
            assert!(sc.is_oracle_authorized(NEW_ORACLE.to_managed_address()));
        });
}

const DEVICE_ONE: TestAddress = TestAddress::new("device-one");

#[test]
fn aggregator_device_registration_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(DEVICE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_device_public_key(
                DEVICE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert!(sc.is_device_registered(DEVICE_ONE.to_managed_address()));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.deregister_device(DEVICE_ONE.to_managed_address());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert!(!sc.is_device_registered(DEVICE_ONE.to_managed_address()));
        });
}

#[test]
fn aggregator_legacy_register_device_rejects_address_as_public_key_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "registerDevicePublicKey required"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_device(DEVICE_ONE.to_managed_address());
        });
}

#[test]
fn aggregator_duplicate_reading_rejection_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    // Set block timestamp so oracle readings are not rejected as FUTURE_TIMESTAMP
    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    // Submit first IoT reading
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-dup"),
                1_710_600_000u64,
                1_710_720_000u64,
                0u8,
                ManagedBuffer::from(b"bafydup001"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-dup",
                    1_710_600_000u64,
                    1_710_720_000u64,
                    b"bafydup001",
                    1_710_719_000u64,
                ),
            );
        });

    // Submit duplicate reading for same source/period
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "READING_ALREADY_SUBMITTED: reading already exists for this source/period",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-dup"),
                1_710_600_000u64,
                1_710_720_000u64,
                0u8,
                ManagedBuffer::from(b"bafydup002"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-dup",
                    1_710_600_000u64,
                    1_710_720_000u64,
                    b"bafydup002",
                    1_710_719_000u64,
                ),
            );
        });
}

const VERIFIER_ONE: TestAddress = TestAddress::new("verifier-one");
const ORACLE_THREE: TestAddress = TestAddress::new("oracle-three");

#[test]
fn aggregator_quorum_boundary_exactly_met_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_THREE).nonce(1).balance(1_000_000u64);

    // Deploy with quorum=3 (all three sources required)
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(3u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_oracle(ORACLE_THREE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    // Submit all 3 source types
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-q3"),
                period_end - 200_000u64,
                period_end,
                0u8, // IoT
                ManagedBuffer::from(b"7500"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-q3",
                    period_end - 200_000u64,
                    period_end,
                    b"7500",
                    period_end - 100u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-q3"),
                period_end - 200_000u64,
                period_end,
                1u8,                          // Satellite
                ManagedBuffer::from(b"7500"), // same value => no discrepancy
                period_end - 100u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });
    world
        .tx()
        .from(ORACLE_THREE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-q3"),
                period_end - 200_000u64,
                period_end,
                2u8, // GovtLab
                ManagedBuffer::from(b"bafygovt"),
                period_end - 100u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 1u64);

    // Seal should succeed with quorum=3 met, no discrepancy (IoT == Satellite CIDs)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-q3"), period_end);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            let sealed = sc
                .get_sealed_event(ManagedBuffer::from(b"pai-q3"), period_end)
                .into_option()
                .unwrap();
            assert_eq!(sealed.reading_count, 3u32);
            assert!(!sealed.semantic_discrepancy);
        });
}

#[test]
fn aggregator_quorum_increase_rejects_previously_valid_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    // Submit only 1 reading
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-qinc"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"bafyqinc"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-qinc",
                    period_end - 200_000u64,
                    period_end,
                    b"bafyqinc",
                    period_end - 100u64,
                ),
            );
        });

    // Increase quorum to 3 before sealing
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.set_quorum(3u32);
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 1u64);

    // Now try seal — should fail with new quorum
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "insufficient oracle readings for quorum"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-qinc"), period_end);
        });
}

#[test]
fn aggregator_quorum_decrease_is_rejected_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(3u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "QUORUM_DECREASE_DISABLED: quorum changes must not reduce the current threshold",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.set_quorum(2u32);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert_eq!(sc.quorum().get(), 3u32);
        });
}

#[test]
fn aggregator_admin_actions_use_shared_mrv_governance_after_acceptance_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.set_governance(SIGNER_ONE.to_managed_address());
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.accept_governance();
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_verifier(SIGNER_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
            sc.set_quorum(3u32);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert!(sc.is_oracle_authorized(ORACLE_ONE.to_managed_address()));
            assert!(sc.is_verifier_authorized(SIGNER_TWO.to_managed_address()));
            assert!(sc.is_device_registered(ORACLE_ONE.to_managed_address()));
            assert_eq!(sc.quorum().get(), 3u32);
        });
}

#[test]
fn aggregator_set_coherence_windows_rejects_out_of_bounds_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "coherence window below minimum"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.set_coherence_windows(0u64, 864000u64, 2592000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "coherence window exceeds maximum"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.set_coherence_windows(172800u64, 864000u64, 2592001u64);
        });
}

#[test]
fn aggregator_quorum_above_source_count_is_rejected_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "quorum exceeds available oracle source count",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.set_quorum(4u32);
        });
}

#[test]
fn aggregator_init_quorum_above_source_count_is_rejected_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "quorum exceeds available oracle source count",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(4u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });
}

#[test]
fn aggregator_rejects_oracle_reading_for_ungranted_source_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle_for_source(ORACLE_ONE.to_managed_address(), 0u8);
            assert!(sc.is_oracle_source_authorized(ORACLE_ONE.to_managed_address(), 0u8));
            assert!(!sc.is_oracle_source_authorized(ORACLE_ONE.to_managed_address(), 1u8));
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "ORACLE_SOURCE_NOT_AUTHORIZED: caller is not authorized for this source",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-source-auth"),
                1_710_600_000u64,
                1_710_720_000u64,
                1u8,
                ManagedBuffer::from(b"bafysat001"),
                1_710_710_000u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });
}

#[test]
fn aggregator_source_registration_preserves_legacy_full_source_oracle_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            assert!(sc.is_oracle_source_authorized(ORACLE_ONE.to_managed_address(), 0u8));
            assert!(sc.is_oracle_source_authorized(ORACLE_ONE.to_managed_address(), 1u8));
            assert!(sc.is_oracle_source_authorized(ORACLE_ONE.to_managed_address(), 2u8));

            sc.register_oracle_for_source(ORACLE_ONE.to_managed_address(), 0u8);
            assert!(sc.is_oracle_source_authorized(ORACLE_ONE.to_managed_address(), 0u8));
            assert!(sc.is_oracle_source_authorized(ORACLE_ONE.to_managed_address(), 1u8));
            assert!(sc.is_oracle_source_authorized(ORACLE_ONE.to_managed_address(), 2u8));
        });
}

#[test]
fn aggregator_numeric_divergence_below_threshold_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    // Set divergence threshold to 3000 bps (30%)
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    // Submit IoT = 7500, Satellite = 7000 => diff = 500 < threshold 3000
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-div"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"7500"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-div",
                    period_end - 200_000u64,
                    period_end,
                    b"7500",
                    period_end - 100u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-div"),
                period_end - 200_000u64,
                period_end,
                1u8,
                ManagedBuffer::from(b"7000"),
                period_end - 100u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 1u64);

    // Should seal without discrepancy acknowledgement since diff (500) <= threshold (3000)
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-div"), period_end);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            let sealed = sc
                .get_sealed_event(ManagedBuffer::from(b"pai-div"), period_end)
                .into_option()
                .unwrap();
            assert!(!sealed.semantic_discrepancy);
        });
}

#[test]
fn aggregator_numeric_divergence_above_threshold_blocks_seal_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    // Low threshold: 100 bps
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 100u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    // diff = 500 > threshold 100 => discrepancy
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-divhi"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"7500"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-divhi",
                    period_end - 200_000u64,
                    period_end,
                    b"7500",
                    period_end - 100u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-divhi"),
                period_end - 200_000u64,
                period_end,
                1u8,
                ManagedBuffer::from(b"7000"),
                period_end - 100u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 1u64);

    // Should block seal due to unacknowledged discrepancy
    world.tx().from(OWNER).to(SC_ADDRESS)
        .returns(ExpectError(4u64, "DISCREPANCY_NOT_ACKNOWLEDGED: IoT-Satellite divergence detected \u{2014} call acknowledgeDiscrepancy before sealing"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-divhi"), period_end);
        });
}

#[test]
fn aggregator_oracle_rotation_non_proposed_rejects_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(NEW_ORACLE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
        });

    // Propose NEW_ORACLE to replace ORACLE_ONE
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.propose_oracle_update(
                ORACLE_ONE.to_managed_address(),
                NEW_ORACLE.to_managed_address(),
                100_000u64,
            );
        });

    // Wrong address tries to accept
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "only the proposed oracle can accept"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.accept_oracle_update(ORACLE_ONE.to_managed_address());
        });
}

#[test]
fn aggregator_double_seal_rejected_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let period_end: u64 = 1_710_720_000;

    // Submit two readings with identical numeric CIDs (no discrepancy)
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-ds"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"8000"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-ds",
                    period_end - 200_000u64,
                    period_end,
                    b"8000",
                    period_end - 100u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-ds"),
                period_end - 200_000u64,
                period_end,
                1u8,
                ManagedBuffer::from(b"8000"),
                period_end - 100u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 1u64);

    // First seal succeeds
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-ds"), period_end);
        });

    // Second seal attempt fails
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "EVENT_ALREADY_SEALED: period already sealed",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-ds"), period_end);
        });
}

#[test]
fn aggregator_iot_short_signature_rejected_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    // Submit IoT reading with signature too short (< 64 bytes)
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "INVALID_DEVICE_SIGNATURE: signature must be exactly 64 bytes (ed25519)",
        ))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-short"),
                1_710_600_000u64,
                1_710_720_000u64,
                0u8,
                ManagedBuffer::from(b"bafyshort"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                ManagedBuffer::from(b"too-short-sig"),
            );
        });
}

#[test]
fn aggregator_iot_invalid_signature_rejected_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(10u64, "ed25519 verify error"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-bad-sig"),
                1_710_600_000u64,
                1_710_720_000u64,
                0u8,
                ManagedBuffer::from(b"bafybad"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                ManagedBuffer::from(&[0xAAu8; 64]),
            );
        });
}

#[test]
fn aggregator_iot_signature_for_other_contract_rejected_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    let other_sc_address = [0x99u8; 32];
    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(10u64, "ed25519 verify error"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-wrong-sc"),
                1_710_600_000u64,
                1_710_720_000u64,
                0u8,
                ManagedBuffer::from(b"bafywrongsc"),
                1_710_719_000u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature_for_sc(
                    &other_sc_address,
                    ORACLE_ONE,
                    b"pai-wrong-sc",
                    1_710_600_000u64,
                    1_710_720_000u64,
                    b"bafywrongsc",
                    1_710_719_000u64,
                ),
            );
        });
}

#[test]
fn aggregator_verifier_can_seal_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);
    world.account(VERIFIER_ONE).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
            sc.register_verifier(VERIFIER_ONE.to_managed_address());
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);
    let period_end: u64 = 1_710_720_000;

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-vseal"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"9000"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-vseal",
                    period_end - 200_000u64,
                    period_end,
                    b"9000",
                    period_end - 100u64,
                ),
            );
        });
    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-vseal"),
                period_end - 200_000u64,
                period_end,
                1u8,
                ManagedBuffer::from(b"9000"),
                period_end - 100u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });

    world
        .current_block()
        .block_timestamp_seconds(period_end + 1u64);

    // Verifier (not owner/oracle) can also call trySeal
    world
        .tx()
        .from(VERIFIER_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-vseal"), period_end);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            assert!(sc.is_sealed(ManagedBuffer::from(b"pai-vseal"), period_end));
        });
}

#[test]
fn aggregator_governance_pause_blocks_submit_and_seal_rs() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_ONE).nonce(1).balance(1_000_000u64);
    world.account(ORACLE_TWO).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_ONE).nonce(1).balance(1_000_000u64);
    world.account(SIGNER_TWO).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.init(2u32, 172800u64, 864000u64, 2592000u64, 3000u64);
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.register_oracle(ORACLE_ONE.to_managed_address());
            sc.register_oracle(ORACLE_TWO.to_managed_address());
            sc.register_device_public_key(
                ORACLE_ONE.to_managed_address(),
                test_device_public_key(),
            );
            sc.register_device_public_key(
                ORACLE_TWO.to_managed_address(),
                test_device_public_key(),
            );
        });

    let period_end: u64 = 1_710_720_000u64;
    world
        .current_block()
        .block_timestamp_seconds(1_710_800_000u64);

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-paused-seal"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"9000"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-paused-seal",
                    period_end - 200_000u64,
                    period_end,
                    b"9000",
                    period_end - 100u64,
                ),
            );
        });

    world
        .tx()
        .from(ORACLE_TWO)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-paused-seal"),
                period_end - 200_000u64,
                period_end,
                1u8,
                ManagedBuffer::from(b"9000"),
                period_end - 100u64,
                ManagedAddress::zero(),
                ManagedBuffer::new(),
            );
        });

    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(GOVERNANCE_CODE)
        .new_address(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            let mut signers = MultiValueEncoded::new();
            signers.push(SIGNER_ONE.to_managed_address());
            signers.push(SIGNER_TWO.to_managed_address());
            sc.init(2, 3600, signers);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.propose_emergency_pause(ManagedBuffer::from(b"pause-aggregator-001"), true);
        });

    world
        .tx()
        .from(SIGNER_ONE)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-aggregator-001"));
        });

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.approve_proposal(ManagedBuffer::from(b"pause-aggregator-001"));
        });

    world
        .current_block()
        .block_timestamp_seconds(1_710_803_601u64);

    world
        .tx()
        .from(SIGNER_TWO)
        .to(GOVERNANCE_SC)
        .whitebox(mrv_governance::contract_obj, |sc| {
            sc.execute_proposal(ManagedBuffer::from(b"pause-aggregator-001"));
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.set_governance_read_address(GOVERNANCE_SC.to_managed_address());
        });

    world
        .tx()
        .from(ORACLE_ONE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.submit_oracle_reading(
                ManagedBuffer::from(b"pai-paused-submit"),
                period_end - 200_000u64,
                period_end,
                0u8,
                ManagedBuffer::from(b"9000"),
                period_end - 100u64,
                ORACLE_ONE.to_managed_address(),
                iot_signature(
                    ORACLE_ONE,
                    b"pai-paused-submit",
                    period_end - 200_000u64,
                    period_end,
                    b"9000",
                    period_end - 100u64,
                ),
            );
        });

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "MRV_GOVERNANCE_PAUSED"))
        .whitebox(mrv_aggregator::contract_obj, |sc| {
            sc.try_seal(ManagedBuffer::from(b"pai-paused-seal"), period_end);
        });
}
