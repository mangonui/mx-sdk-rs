use multiversx_sc_scenario::imports::*;

use abi_tester::abi_proxy::{
    AbiEnvelope, AbiEnvelopeDomain, AbiManagedComplexVecItem, AbiManagedVecItem, AbiTesterProxy,
    OnlyShowsUpInConstructor,
};

const ABI_TESTER_PATH_EXPR: &str = "mxsc:output/abi-tester.mxsc.json";
const CODE_PATH: MxscPath = MxscPath::new("output/abi-tester.mxsc.json");
const OWNER_ADDRESS: TestAddress = TestAddress::new("owner");
const ABI_ADDRESS: TestSCAddress = TestSCAddress::new("abi-tester");

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new().executor_config(ExecutorConfig::Experimental);
    blockchain.set_current_dir_from_workspace("contracts/feature-tests/abi-tester");
    blockchain.register_contract(ABI_TESTER_PATH_EXPR, abi_tester::ContractBuilder);
    blockchain
}

fn deploy(world: &mut ScenarioWorld) {
    world.account(OWNER_ADDRESS).nonce(1).balance(100);
    world.new_address(OWNER_ADDRESS, 1, ABI_ADDRESS);

    world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(AbiTesterProxy)
        .init(0i32, OnlyShowsUpInConstructor { something: () })
        .code(CODE_PATH)
        .new_address(ABI_ADDRESS)
        .run();
}

#[test]
fn abi_tester_experimental_managed_vec_return_smoke() {
    let mut world = world();
    deploy(&mut world);

    let value: ManagedVec<StaticApi, AbiManagedVecItem> = world
        .query()
        .to(ABI_ADDRESS)
        .typed(AbiTesterProxy)
        .item_for_managed_vec()
        .returns(ReturnsResult)
        .run();

    assert!(value.is_empty());
}

#[test]
fn abi_tester_experimental_managed_address_and_byte_array_return_smoke() {
    let mut world = world();
    deploy(&mut world);

    let address = OWNER_ADDRESS.to_managed_address();
    let bytes = ManagedByteArray::<StaticApi, 32>::new_from_bytes(&[7u8; 32]);

    let value = world
        .tx()
        .from(OWNER_ADDRESS)
        .to(ABI_ADDRESS)
        .typed(AbiTesterProxy)
        .managed_address_vs_byte_array(address.clone(), bytes.clone())
        .returns(ReturnsResult)
        .run();

    assert_eq!(value, MultiValue2((address, bytes)));
}

#[test]
fn abi_tester_experimental_managed_complex_vec_return_smoke() {
    let mut world = world();
    deploy(&mut world);

    let value: ManagedVec<StaticApi, AbiManagedComplexVecItem<StaticApi>> = world
        .query()
        .to(ABI_ADDRESS)
        .typed(AbiTesterProxy)
        .item_for_managed_complex_vec()
        .returns(ReturnsResult)
        .run();

    assert_eq!(value.len(), 1);
    let first = value.get(0);
    assert_eq!(first.token_id, ManagedBuffer::from(b"CARBON-ab12cd"));
    assert_eq!(first.holder, ManagedAddress::zero());
    assert_eq!(first.version, 7);
    assert_eq!(first.body, ManagedBuffer::from(b"{\"ok\":true}"));
}

#[test]
fn abi_tester_experimental_envelope_like_return_smoke() {
    let mut world = world();
    deploy(&mut world);

    let value: AbiEnvelope<StaticApi> = world
        .query()
        .to(ABI_ADDRESS)
        .typed(AbiTesterProxy)
        .envelope_like_result()
        .returns(ReturnsResult)
        .run();

    assert_eq!(value.domain, AbiEnvelopeDomain::Alpha);
    assert_eq!(value.payload_hash, ManagedBuffer::from(&[9u8; 32]));
    assert_eq!(value.operations.len(), 1);
    let first = value.operations.get(0);
    assert_eq!(first.token_id, ManagedBuffer::from(b"CARBON-ab12cd"));
}

#[test]
fn abi_tester_experimental_validate_token_id_and_return_envelope_smoke() {
    let mut world = world();
    deploy(&mut world);

    let value: AbiEnvelope<StaticApi> = world
        .query()
        .to(ABI_ADDRESS)
        .typed(AbiTesterProxy)
        .validate_token_id_and_return_envelope(ManagedBuffer::from(b"CARBON-ab12cd"))
        .returns(ReturnsResult)
        .run();

    assert_eq!(value.domain, AbiEnvelopeDomain::Alpha);
    assert_eq!(value.payload_hash, ManagedBuffer::from(&[9u8; 32]));
    assert_eq!(value.operations.len(), 1);
    let first = value.operations.get(0);
    assert_eq!(first.token_id, ManagedBuffer::from(b"CARBON-ab12cd"));
}

#[test]
fn abi_tester_experimental_validate_constant_token_id_and_return_envelope_smoke() {
    let mut world = world();
    deploy(&mut world);

    let value: AbiEnvelope<StaticApi> = world
        .query()
        .to(ABI_ADDRESS)
        .typed(AbiTesterProxy)
        .validate_constant_token_id_and_return_envelope()
        .returns(ReturnsResult)
        .run();

    assert_eq!(value.domain, AbiEnvelopeDomain::Alpha);
    assert_eq!(value.payload_hash, ManagedBuffer::from(&[9u8; 32]));
    assert_eq!(value.operations.len(), 1);
    let first = value.operations.get(0);
    assert_eq!(first.token_id, ManagedBuffer::from(b"CARBON-ab12cd"));
}

#[test]
fn abi_tester_experimental_validate_token_id_and_return_envelope_tx_smoke() {
    let mut world = world();
    deploy(&mut world);

    let value: AbiEnvelope<StaticApi> = world
        .tx()
        .from(OWNER_ADDRESS)
        .to(ABI_ADDRESS)
        .typed(AbiTesterProxy)
        .validate_token_id_and_return_envelope(ManagedBuffer::from(b"CARBON-ab12cd"))
        .returns(ReturnsResult)
        .run();

    assert_eq!(value.domain, AbiEnvelopeDomain::Alpha);
    assert_eq!(value.payload_hash, ManagedBuffer::from(&[9u8; 32]));
    assert_eq!(value.operations.len(), 1);
    let first = value.operations.get(0);
    assert_eq!(first.token_id, ManagedBuffer::from(b"CARBON-ab12cd"));
}

#[test]
fn abi_tester_experimental_set_token_scoped_value_and_return_envelope_tx_smoke() {
    let mut world = world();
    deploy(&mut world);

    let value: AbiEnvelope<StaticApi> = world
        .tx()
        .from(OWNER_ADDRESS)
        .to(ABI_ADDRESS)
        .typed(AbiTesterProxy)
        .set_token_scoped_value_and_return_envelope(
            ManagedBuffer::from(b"CARBON-ab12cd"),
            ManagedBuffer::from(b"approved"),
        )
        .returns(ReturnsResult)
        .run();

    assert_eq!(value.domain, AbiEnvelopeDomain::Alpha);
    assert_eq!(value.payload_hash, ManagedBuffer::from(&[9u8; 32]));
    assert_eq!(value.operations.len(), 1);
    let first = value.operations.get(0);
    assert_eq!(first.token_id, ManagedBuffer::from(b"CARBON-ab12cd"));
}
