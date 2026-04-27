use drwa_common::{
    DRWA_SYNC_ENVELOPE_SCHEMA_VERSION, DRWA_SYNC_ENVELOPE_SCHEMA_VERSION_WITH_RECOVERY,
    DrwaCallerDomain, DrwaSyncEnvelope, DrwaSyncOperation, DrwaSyncOperationType,
    build_sync_hook_payload, build_sync_hook_payload_with_recovery_metadata,
};
use multiversx_sc::{
    codec::top_encode_to_vec_u8_or_panic,
    types::{ManagedAddress, ManagedBuffer, ManagedVec},
};
use multiversx_sc_scenario::imports::StaticApi;

#[test]
fn drwa_sync_envelope_top_encode_smoke() {
    let mut operations = ManagedVec::<StaticApi, DrwaSyncOperation<StaticApi>>::new();
    operations.push(DrwaSyncOperation {
        operation_type: DrwaSyncOperationType::TokenPolicy,
        token_id: ManagedBuffer::from(b"CARBON-ab12cd"),
        holder: ManagedAddress::zero(),
        version: 7,
        body: ManagedBuffer::from(
            br#"{"drwa_enabled":true,"global_pause":false,"strict_auditor_mode":true}"#,
        ),
    });

    let envelope = DrwaSyncEnvelope::<StaticApi> {
        schema_version: DRWA_SYNC_ENVELOPE_SCHEMA_VERSION,
        caller_domain: DrwaCallerDomain::PolicyRegistry,
        payload_hash: ManagedBuffer::from(&[9u8; 32]),
        operations,
        pre_recovery_state_hash: ManagedBuffer::new(),
        recovery_scope: ManagedVec::new(),
    };

    let encoded = top_encode_to_vec_u8_or_panic(&envelope);
    assert!(!encoded.is_empty(), "encoded envelope must not be empty");
}

#[test]
fn drwa_sync_hook_payload_handles_max_operations_near_payload_cap() {
    const MAX_OPS: usize = 256;
    const MAX_NATIVE_PAYLOAD_BYTES: usize = 1 << 20;
    const BODY_BYTES: usize = 4_029;

    let mut operations = ManagedVec::<StaticApi, DrwaSyncOperation<StaticApi>>::new();
    let body = [b'a'; BODY_BYTES];
    for version in 1..=MAX_OPS as u64 {
        operations.push(DrwaSyncOperation {
            operation_type: DrwaSyncOperationType::TokenPolicy,
            token_id: ManagedBuffer::from(b"CARBON-ab12cd"),
            holder: ManagedAddress::zero(),
            version,
            body: ManagedBuffer::from(&body[..]),
        });
    }

    let payload_hash = ManagedBuffer::<StaticApi>::from(&[7u8; 32]);
    let hook_payload = build_sync_hook_payload(
        &DrwaCallerDomain::PolicyRegistry,
        &operations,
        &payload_hash,
    );

    assert_eq!(operations.len(), MAX_OPS);
    assert!(
        hook_payload.len() <= MAX_NATIVE_PAYLOAD_BYTES,
        "max-operation sync payload must stay within the native 1 MiB cap"
    );
    assert!(
        hook_payload.len() > MAX_NATIVE_PAYLOAD_BYTES - 4_096,
        "regression should exercise the near-cap payload shape"
    );
}

#[test]
fn drwa_recovery_hook_payload_serializes_governance_metadata_and_tags() {
    let mut operations = ManagedVec::<StaticApi, DrwaSyncOperation<StaticApi>>::new();
    operations.push(DrwaSyncOperation {
        operation_type: DrwaSyncOperationType::GovernanceApprove,
        token_id: ManagedBuffer::new(),
        holder: ManagedAddress::zero(),
        version: 1,
        body: ManagedBuffer::from(&[3u8; 32]),
    });
    operations.push(DrwaSyncOperation {
        operation_type: DrwaSyncOperationType::GovernanceExecute,
        token_id: ManagedBuffer::new(),
        holder: ManagedAddress::zero(),
        version: 2,
        body: ManagedBuffer::from(&[4u8; 32]),
    });

    let mut recovery_scope = ManagedVec::<StaticApi, ManagedBuffer<StaticApi>>::new();
    recovery_scope.push(ManagedBuffer::from(b"CARBON-ab12cd"));

    let payload_hash = ManagedBuffer::<StaticApi>::from(&[5u8; 32]);
    let pre_recovery_state_hash = ManagedBuffer::<StaticApi>::from(&[6u8; 32]);
    let hook_payload = build_sync_hook_payload_with_recovery_metadata(
        &DrwaCallerDomain::RecoveryAdmin,
        &operations,
        &payload_hash,
        &pre_recovery_state_hash,
        &recovery_scope,
    );

    let bytes = hook_payload.to_boxed_bytes();
    let bytes = bytes.as_slice();
    assert_eq!(&bytes[..32], &[5u8; 32]);
    assert_eq!(
        u16::from_be_bytes([bytes[32], bytes[33]]),
        DRWA_SYNC_ENVELOPE_SCHEMA_VERSION_WITH_RECOVERY
    );
    assert_eq!(bytes[34], 4, "recovery_admin caller tag");

    let mut offset = 35;
    assert_eq!(read_u32(bytes, &mut offset), 32);
    assert_eq!(&bytes[offset..offset + 32], &[6u8; 32]);
    offset += 32;
    assert_eq!(read_u16(bytes, &mut offset), 1);
    assert_eq!(read_u32(bytes, &mut offset), 13);
    assert_eq!(&bytes[offset..offset + 13], b"CARBON-ab12cd");
    offset += 13;
    assert_eq!(read_u16(bytes, &mut offset), 2);
    assert_eq!(bytes[offset], 7, "governance approve operation tag");
    offset = skip_operation_after_tag(bytes, offset + 1);
    assert_eq!(bytes[offset], 8, "governance execute operation tag");
}

fn read_u16(bytes: &[u8], offset: &mut usize) -> u16 {
    let value = u16::from_be_bytes([bytes[*offset], bytes[*offset + 1]]);
    *offset += 2;
    value
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> u32 {
    let value = u32::from_be_bytes([
        bytes[*offset],
        bytes[*offset + 1],
        bytes[*offset + 2],
        bytes[*offset + 3],
    ]);
    *offset += 4;
    value
}

fn skip_len_prefixed(bytes: &[u8], offset: &mut usize) {
    let len = read_u32(bytes, offset) as usize;
    *offset += len;
}

fn skip_operation_after_tag(bytes: &[u8], mut offset: usize) -> usize {
    skip_len_prefixed(bytes, &mut offset);
    skip_len_prefixed(bytes, &mut offset);
    offset += 8;
    skip_len_prefixed(bytes, &mut offset);
    offset
}
