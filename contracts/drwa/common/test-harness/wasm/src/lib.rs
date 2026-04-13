#![no_std]

multiversx_sc_wasm_adapter::allocator!();
multiversx_sc_wasm_adapter::panic_handler!();

multiversx_sc_wasm_adapter::endpoints! {
    drwa_common_test_harness
    (
        init => init
        upgrade => upgrade
        validateTokenId => validate_token_id
        validateKycStatus => validate_kyc_status
        validateAmlStatus => validate_aml_status
        testPushLenPrefixed => test_push_len_prefixed
        testSerializeSyncPayload => test_serialize_sync_payload
    )
}
