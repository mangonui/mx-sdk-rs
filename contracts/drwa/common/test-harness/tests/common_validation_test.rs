use drwa_common_test_harness::DrwaCommonTestHarness;
use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-common-test-harness");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-common-test-harness.mxsc.json");

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/drwa/common/test-harness");
    world.register_contract(CODE_PATH, drwa_common_test_harness::ContractBuilder);
    world
}

fn deploy(world: &mut ScenarioWorld) {
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.init();
        });
}

// ── require_valid_token_id: valid inputs ────────────────────────────

#[test]
fn token_id_valid_standard() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CARBON-ab12cd"));
        });
}

#[test]
fn token_id_valid_short_ticker() {
    let mut world = world();
    deploy(&mut world);

    // 3-char ticker: minimum valid ticker length
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"ABC-a1b2c3"));
        });
}

#[test]
fn token_id_valid_long_ticker() {
    let mut world = world();
    deploy(&mut world);

    // 10-char ticker: maximum valid ticker length
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"ABCDEFGHIJ-a1b2c3"));
        });
}

#[test]
fn token_id_valid_digits_in_ticker() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"TOK3N1-aabbcc"));
        });
}

// ── require_valid_token_id: invalid inputs ──────────────────────────

#[test]
fn token_id_rejects_empty() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id must not be empty"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::new());
        });
}

#[test]
fn token_id_rejects_too_short() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id is too short"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"AB-1234"));
        });
}

#[test]
fn token_id_rejects_no_hyphen() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "token_id must contain exactly one hyphen",
        ))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CARBONab12cd"));
        });
}

#[test]
fn token_id_rejects_multiple_hyphens() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "token_id must contain exactly one hyphen",
        ))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CAR-BO-ab12cd"));
        });
}

#[test]
fn token_id_rejects_ticker_too_short() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id ticker is too short"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"AB-ab12cd"));
        });
}

#[test]
fn token_id_rejects_ticker_too_long() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id is too long"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"ABCDEFGHIJK-ab12cd"));
        });
}

#[test]
fn token_id_rejects_suffix_too_short() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id suffix must be 6 characters"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CARBON-ab12c"));
        });
}

#[test]
fn token_id_rejects_suffix_too_long() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id suffix must be 6 characters"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CARBON-ab12cde"));
        });
}

#[test]
fn token_id_rejects_lowercase_ticker() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "token_id ticker must be uppercase alphanumeric",
        ))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"carbon-ab12cd"));
        });
}

#[test]
fn token_id_rejects_uppercase_suffix() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id suffix must be lowercase hex"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CARBON-AB12CD"));
        });
}

#[test]
fn token_id_rejects_non_hex_suffix() {
    let mut world = world();
    deploy(&mut world);

    // 'g' is not valid hex
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id suffix must be lowercase hex"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CARBON-ab12gx"));
        });
}

#[test]
fn token_id_rejects_null_bytes() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id must not contain null bytes"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CARBON\x00ab12cd"));
        });
}

#[test]
fn token_id_rejects_special_chars_in_ticker() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "token_id ticker must be uppercase alphanumeric",
        ))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_token_id(ManagedBuffer::from(b"CAR!ON-ab12cd"));
        });
}

// ── require_valid_kyc_status ────────────────────────────────────────

#[test]
fn kyc_status_accepts_all_valid_values() {
    let mut world = world();
    deploy(&mut world);

    let valid_statuses: &[&[u8]] = &[
        b"approved",
        b"pending",
        b"rejected",
        b"expired",
        b"not_started",
        b"deactivated",
    ];

    for status in valid_statuses {
        world.tx().from(OWNER).to(SC_ADDRESS).whitebox(
            drwa_common_test_harness::contract_obj,
            |sc| {
                sc.validate_kyc_status(ManagedBuffer::from(*status));
            },
        );
    }
}

#[test]
fn kyc_status_rejects_unknown_value() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "invalid kyc_status: must be one of approved, pending, rejected, expired, not_started, deactivated",
        ))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_kyc_status(ManagedBuffer::from(b"unknown"));
        });
}

#[test]
fn kyc_status_rejects_empty() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "invalid status length"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_kyc_status(ManagedBuffer::new());
        });
}

#[test]
fn kyc_status_rejects_case_variant() {
    let mut world = world();
    deploy(&mut world);

    // "Approved" (capitalized) must be rejected — strict match
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "invalid kyc_status: must be one of approved, pending, rejected, expired, not_started, deactivated",
        ))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_kyc_status(ManagedBuffer::from(b"Approved"));
        });
}

// ── require_valid_aml_status ────────────────────────────────────────

#[test]
fn aml_status_accepts_all_valid_values() {
    let mut world = world();
    deploy(&mut world);

    let valid_statuses: &[&[u8]] = &[
        b"clear",
        b"flagged",
        b"review",
        b"blocked",
        b"not_started",
        b"deactivated",
    ];

    for status in valid_statuses {
        world.tx().from(OWNER).to(SC_ADDRESS).whitebox(
            drwa_common_test_harness::contract_obj,
            |sc| {
                sc.validate_aml_status(ManagedBuffer::from(*status));
            },
        );
    }
}

#[test]
fn aml_status_rejects_unknown_value() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "invalid aml_status: must be one of clear, pending, flagged, review, blocked, not_started, deactivated",
        ))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_aml_status(ManagedBuffer::from(b"suspicious"));
        });
}

#[test]
fn aml_status_rejects_empty() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "invalid status length"))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_aml_status(ManagedBuffer::new());
        });
}

#[test]
fn aml_status_rejects_case_variant() {
    let mut world = world();
    deploy(&mut world);

    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "invalid aml_status: must be one of clear, pending, flagged, review, blocked, not_started, deactivated",
        ))
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            sc.validate_aml_status(ManagedBuffer::from(b"CLEAR"));
        });
}

// ── push_len_prefixed ───────────────────────────────────────────────

#[test]
fn push_len_prefixed_empty_value() {
    let mut world = world();
    deploy(&mut world);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            let result = sc.test_push_len_prefixed(ManagedBuffer::new());
            // Empty value: 4-byte length (0) followed by no data
            assert_eq!(result.len(), 4);
            let bytes = result.to_boxed_bytes();
            assert_eq!(bytes.as_slice(), &[0u8, 0, 0, 0]);
        });
}

#[test]
fn push_len_prefixed_non_empty_value() {
    let mut world = world();
    deploy(&mut world);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            let result = sc.test_push_len_prefixed(ManagedBuffer::from(b"ABC"));
            // 4-byte big-endian length (3) + 3 data bytes
            assert_eq!(result.len(), 7);
            let bytes = result.to_boxed_bytes();
            assert_eq!(bytes.as_slice(), &[0u8, 0, 0, 3, b'A', b'B', b'C']);
        });
}

#[test]
fn push_len_prefixed_binary_data() {
    let mut world = world();
    deploy(&mut world);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            let result = sc.test_push_len_prefixed(ManagedBuffer::from(&[0xFF, 0x00, 0xAB]));
            assert_eq!(result.len(), 7);
            let bytes = result.to_boxed_bytes();
            assert_eq!(bytes.as_slice(), &[0u8, 0, 0, 3, 0xFF, 0x00, 0xAB]);
        });
}

// ── serialize_sync_envelope_payload ─────────────────────────────────

#[test]
fn serialize_sync_payload_token_policy_operation() {
    let mut world = world();
    deploy(&mut world);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            let result = sc.test_serialize_sync_payload(
                0u8, // PolicyRegistry
                0u8, // TokenPolicy
                ManagedBuffer::from(b"TOK-aabbcc"),
                ManagedAddress::zero(),
                1u64,
                ManagedBuffer::from(b"{}"),
            );

            let bytes = result.to_boxed_bytes();
            let b = bytes.as_slice();

            // Bytes 0..2: sync envelope schema version (v1)
            assert_eq!(&b[0..2], &[0u8, 1]);
            // Byte 2: caller domain tag (PolicyRegistry = 0)
            assert_eq!(b[2], 0u8);
            // Byte 3: operation type tag (TokenPolicy = 0)
            assert_eq!(b[3], 0u8);

            // Bytes 4..8: token_id length (10 = len("TOK-aabbcc"))
            assert_eq!(&b[4..8], &[0u8, 0, 0, 10]);
            // Bytes 8..18: token_id data
            assert_eq!(&b[8..18], b"TOK-aabbcc");

            // Bytes 18..22: holder length (32 = zero address length)
            assert_eq!(&b[18..22], &[0u8, 0, 0, 32]);
            // Bytes 22..54: 32-byte zero address
            assert_eq!(&b[22..54], &[0u8; 32]);

            // Bytes 54..62: version (1) as big-endian u64
            assert_eq!(&b[54..62], &[0u8, 0, 0, 0, 0, 0, 0, 1]);

            // Bytes 62..66: body length (2 = len("{}"))
            assert_eq!(&b[62..66], &[0u8, 0, 0, 2]);
            // Bytes 66..68: body data
            assert_eq!(&b[66..68], b"{}");

            // Total: 2 + 1 + 1 + (4+10) + (4+32) + 8 + (4+2) = 68
            assert_eq!(b.len(), 68);
        });
}

#[test]
fn serialize_sync_payload_identity_registry_holder_profile() {
    let mut world = world();
    deploy(&mut world);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_common_test_harness::contract_obj, |sc| {
            let result = sc.test_serialize_sync_payload(
                2u8, // IdentityRegistry
                3u8, // HolderProfile
                ManagedBuffer::new(),
                ManagedAddress::zero(),
                42u64,
                ManagedBuffer::from(b"profile-body"),
            );

            let bytes = result.to_boxed_bytes();
            let b = bytes.as_slice();

            // Bytes 0..2: sync envelope schema version (v1)
            assert_eq!(&b[0..2], &[0u8, 1]);
            // Byte 2: IdentityRegistry = 2
            assert_eq!(b[2], 2u8);
            // Byte 3: HolderProfile = 3
            assert_eq!(b[3], 3u8);

            // Bytes 4..8: empty token_id length (0)
            assert_eq!(&b[4..8], &[0u8, 0, 0, 0]);

            // Bytes 8..12: holder length (32)
            assert_eq!(&b[8..12], &[0u8, 0, 0, 32]);

            // Bytes 12..44: 32-byte zero address
            assert_eq!(&b[12..44], &[0u8; 32]);

            // Bytes 44..52: version (42) as big-endian u64
            assert_eq!(&b[44..52], &[0u8, 0, 0, 0, 0, 0, 0, 42]);

            // Bytes 52..56: body length (12)
            assert_eq!(&b[52..56], &[0u8, 0, 0, 12]);
            // Bytes 56..68: body data
            assert_eq!(&b[56..68], b"profile-body");
        });
}

#[test]
fn serialize_sync_payload_all_caller_domains() {
    let mut world = world();
    deploy(&mut world);

    // Verify all 5 caller domain tags encode correctly
    let expected_tags: &[(u8, u8)] = &[
        (0, 0), // PolicyRegistry -> 0
        (1, 1), // AssetManager -> 1
        (2, 2), // IdentityRegistry -> 2
        (3, 3), // Attestation -> 3
        (4, 4), // RecoveryAdmin -> 4
    ];

    for &(input_tag, expected_byte) in expected_tags {
        world
            .query()
            .to(SC_ADDRESS)
            .whitebox(drwa_common_test_harness::contract_obj, |sc| {
                let result = sc.test_serialize_sync_payload(
                    input_tag,
                    0u8,
                    ManagedBuffer::new(),
                    ManagedAddress::zero(),
                    0u64,
                    ManagedBuffer::new(),
                );
                let bytes = result.to_boxed_bytes();
                assert_eq!(&bytes.as_slice()[0..2], &[0u8, 1]);
                assert_eq!(bytes.as_slice()[2], expected_byte);
            });
    }
}

#[test]
fn serialize_sync_payload_all_operation_types() {
    let mut world = world();
    deploy(&mut world);

    // Verify all 6 operation type tags encode correctly
    let expected_tags: &[(u8, u8)] = &[
        (0, 0), // TokenPolicy -> 0
        (1, 1), // AssetRecord -> 1
        (2, 2), // HolderMirror -> 2
        (3, 3), // HolderProfile -> 3
        (4, 4), // HolderAuditorAuthorization -> 4
        (5, 5), // HolderMirrorDelete -> 5
    ];

    for &(input_tag, expected_byte) in expected_tags {
        world
            .query()
            .to(SC_ADDRESS)
            .whitebox(drwa_common_test_harness::contract_obj, |sc| {
                let result = sc.test_serialize_sync_payload(
                    0u8,
                    input_tag,
                    ManagedBuffer::new(),
                    ManagedAddress::zero(),
                    0u64,
                    ManagedBuffer::new(),
                );
                let bytes = result.to_boxed_bytes();
                // Byte 3 is the operation type tag after the v1 schema prefix
                assert_eq!(bytes.as_slice()[3], expected_byte);
            });
    }
}
