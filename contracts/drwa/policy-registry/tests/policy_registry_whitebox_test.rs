use drwa_common::{
    DrwaCallerDomain, DrwaGovernanceModule, DrwaSyncOperationType, set_drwa_sync_hook_test_result,
};
use drwa_policy_registry::DrwaPolicyRegistry;
use multiversx_sc::types::{ManagedBuffer, ManagedVec};
use multiversx_sc_scenario::imports::*;

const OWNER: TestAddress = TestAddress::new("owner");
const GOVERNANCE: TestAddress = TestAddress::new("governance");
const SC_ADDRESS: TestSCAddress = TestSCAddress::new("drwa-policy-registry");
const CODE_PATH: MxscPath = MxscPath::new("mxsc:output/drwa-policy-registry.mxsc.json");
const TOKEN_ID_1: &[u8] = b"CARBON-ab12cd";
const TOKEN_ID_2: &[u8] = b"CARBON-bc23de";
const TOKEN_ID_3: &[u8] = b"CARBON-cd34ef";

fn world() -> ScenarioWorld {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::full_suite());
    world.set_current_dir_from_workspace("contracts/drwa/policy-registry");
    world.register_contract(CODE_PATH, drwa_policy_registry::ContractBuilder);
    world
}

#[test]
fn policy_registry_whitebox_flow() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut investor_classes = ManagedVec::new();
            investor_classes.push(ManagedBuffer::from(b"ACCREDITED"));

            let mut jurisdictions = ManagedVec::new();
            jurisdictions.push(ManagedBuffer::from(b"SG"));

            let envelope = sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                true,
                true,
                investor_classes,
                jurisdictions,
            );

            assert!(envelope.caller_domain == DrwaCallerDomain::PolicyRegistry);
            assert_eq!(envelope.operations.len(), 1);

            let operation = envelope.operations.get(0);
            assert!(operation.operation_type == DrwaSyncOperationType::TokenPolicy);
            assert_eq!(operation.version, 1);
            assert!(!envelope.payload_hash.is_empty());
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(TOKEN_ID_1);
            let policy = sc.token_policy(&token_id).get();
            assert!(policy.drwa_enabled);
            assert!(policy.strict_auditor_mode);
            assert!(policy.metadata_protection_enabled);
            assert_eq!(policy.token_policy_version, 1);
            assert_eq!(sc.token_policy_version(&token_id).get(), 1);
        });
}

#[test]
fn policy_registry_sync_hook_failure_reverts_policy_update() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    set_drwa_sync_hook_test_result(9);
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "native mirror sync failed"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                true,
                true,
                ManagedVec::new(),
                ManagedVec::new(),
            );
        });
    set_drwa_sync_hook_test_result(0);

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(TOKEN_ID_1);
            assert!(sc.token_policy(&token_id).is_empty());
            assert!(sc.token_policy_version(&token_id).is_empty());
        });
}

#[test]
fn policy_registry_increments_version_and_rejects_non_owner() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    for version in [1u64, 2u64] {
        world
            .tx()
            .from(GOVERNANCE)
            .to(SC_ADDRESS)
            .whitebox(drwa_policy_registry::contract_obj, |sc| {
                let mut investor_classes = ManagedVec::new();
                investor_classes.push(ManagedBuffer::from(b"ACCREDITED"));

                let mut jurisdictions = ManagedVec::new();
                jurisdictions.push(ManagedBuffer::from(b"SG"));

                let envelope = sc.set_token_policy(
                    ManagedBuffer::from(TOKEN_ID_1),
                    true,
                    version == 2,
                    true,
                    true,
                    investor_classes,
                    jurisdictions,
                );
                assert_eq!(envelope.operations.get(0).version, version);
            });
    }

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(TOKEN_ID_1);
            let policy = sc.token_policy(&token_id).get();
            assert_eq!(policy.token_policy_version, 2);
            assert!(policy.global_pause);
        });
}

#[test]
fn policy_registry_persists_explicit_drwa_enabled_state() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_2),
                false,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let token_id = ManagedBuffer::from(TOKEN_ID_2);
            let policy = sc.token_policy(&token_id).get();
            assert!(!policy.drwa_enabled);
        });
}

#[test]
fn policy_registry_allows_governance_to_set_policy() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.accept_governance();
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_3),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        });
}

#[test]
fn policy_registry_requires_pending_governance_acceptance() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_governance(GOVERNANCE.to_managed_address());
            assert_eq!(
                sc.pending_governance().get(),
                GOVERNANCE.to_managed_address()
            );
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            assert_eq!(sc.governance().get(), GOVERNANCE.to_managed_address());
        });
}

#[test]
fn policy_registry_rejects_invalid_token_id_format() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token_id suffix must be 6 characters"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(b"CARBON-001"),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
        });
}

#[test]
fn policy_registry_rejects_too_many_investor_classes() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "too many investor classes: max 100"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut investor_classes = ManagedVec::new();
            for i in 0u32..101 {
                let mut buf = ManagedBuffer::new();
                buf.append_bytes(b"CLASS");
                buf.append_bytes(&i.to_be_bytes());
                investor_classes.push(buf);
            }

            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                investor_classes,
                ManagedVec::new(),
            );
        });
}

#[test]
fn policy_registry_rejects_too_many_jurisdictions() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "too many jurisdictions: max 200"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut jurisdictions = ManagedVec::new();
            for i in 0u32..201 {
                let mut buf = ManagedBuffer::new();
                buf.append_bytes(b"JUR");
                buf.append_bytes(&i.to_be_bytes());
                jurisdictions.push(buf);
            }

            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                jurisdictions,
            );
        });
}

#[test]
fn policy_registry_rejects_unsafe_json_key() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "policy key contains unsupported character",
        ))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut investor_classes = ManagedVec::new();
            investor_classes.push(ManagedBuffer::from(b"CLASS{\"inject\":true}"));

            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                investor_classes,
                ManagedVec::new(),
            );
        });
}

#[test]
fn policy_registry_deactivate_token_policy() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // First set a policy
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
        });

    // Deactivate
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.deactivate_token_policy(ManagedBuffer::from(TOKEN_ID_1));
            assert!(envelope.caller_domain == DrwaCallerDomain::PolicyRegistry);
            assert_eq!(envelope.operations.get(0).version, 2);
        });

    // Verify deactivated
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let policy = sc.token_policy(&ManagedBuffer::from(TOKEN_ID_1)).get();
            assert!(!policy.drwa_enabled);
            assert_eq!(policy.token_policy_version, 2);
        });
}

#[test]
fn policy_registry_deactivate_nonexistent_policy_fails() {
    let mut world = world();

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "token policy does not exist"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.deactivate_token_policy(ManagedBuffer::from(TOKEN_ID_1));
        });
}

// ── Fuzz-like injection tests for policy JSON serialization ──────────

/// Helper: deploys the policy-registry and attempts to set a token policy
/// with the given investor_class and jurisdiction values. All payloads
/// containing characters outside `[a-zA-Z0-9._-]` must be rejected with
/// "policy key contains unsupported character".
fn assert_json_injection_rejected(
    investor_class_payloads: &[&[u8]],
    jurisdiction_payloads: &[&[u8]],
) {
    for payload in investor_class_payloads {
        let mut world = world();
        world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
        world
            .tx()
            .from(OWNER)
            .raw_deploy()
            .code(CODE_PATH)
            .new_address(SC_ADDRESS)
            .whitebox(drwa_policy_registry::contract_obj, |sc| {
                sc.init(GOVERNANCE.to_managed_address());
            });

        world
            .tx()
            .from(GOVERNANCE)
            .to(SC_ADDRESS)
            .returns(ExpectError(
                4u64,
                "policy key contains unsupported character",
            ))
            .whitebox(drwa_policy_registry::contract_obj, |sc| {
                let mut investor_classes = ManagedVec::new();
                investor_classes.push(ManagedBuffer::from(*payload));

                sc.set_token_policy(
                    ManagedBuffer::from(TOKEN_ID_1),
                    true,
                    false,
                    false,
                    false,
                    investor_classes,
                    ManagedVec::new(),
                );
            });
    }

    for payload in jurisdiction_payloads {
        let mut world = world();
        world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
        world
            .tx()
            .from(OWNER)
            .raw_deploy()
            .code(CODE_PATH)
            .new_address(SC_ADDRESS)
            .whitebox(drwa_policy_registry::contract_obj, |sc| {
                sc.init(GOVERNANCE.to_managed_address());
            });

        world
            .tx()
            .from(GOVERNANCE)
            .to(SC_ADDRESS)
            .returns(ExpectError(
                4u64,
                "policy key contains unsupported character",
            ))
            .whitebox(drwa_policy_registry::contract_obj, |sc| {
                let mut jurisdictions = ManagedVec::new();
                jurisdictions.push(ManagedBuffer::from(*payload));

                sc.set_token_policy(
                    ManagedBuffer::from(TOKEN_ID_1),
                    true,
                    false,
                    false,
                    false,
                    ManagedVec::new(),
                    jurisdictions,
                );
            });
    }
}

#[test]
fn policy_json_injection_curly_braces() {
    // Direct JSON object injection in investor_class and jurisdiction
    assert_json_injection_rejected(
        &[b"{\"inject\":true}", b"CLASS{hidden}", b"}extra"],
        &[b"{\"overwrite\":\"all\"}", b"SG{x}"],
    );
}

#[test]
fn policy_json_injection_square_brackets() {
    // Array injection
    assert_json_injection_rejected(&[b"[\"all\"]", b"CLASS[0]"], &[b"[true]", b"SG[0]"]);
}

#[test]
fn policy_json_injection_quotes() {
    // Double-quote injection: break out of JSON string context
    assert_json_injection_rejected(
        &[b"CLASS\",:true,\"x", b"\"injected\"", b"A\"B"],
        &[b"SG\":true,\"extra\":\"", b"\""],
    );
}

#[test]
fn policy_json_injection_backslash_and_escape_sequences() {
    // Backslash sequences that could alter JSON parsing
    assert_json_injection_rejected(&[b"CLASS\\\"extra", b"\\n", b"\\u0000"], &[b"SG\\", b"\\t"]);
}

#[test]
fn policy_json_injection_colons_and_commas() {
    // Structural JSON delimiters
    assert_json_injection_rejected(
        &[b"key:value", b"a,b", b"CLASS:true"],
        &[b"SG,US", b"key:val"],
    );
}

#[test]
fn policy_json_injection_control_characters() {
    // Null bytes, newlines, tabs — could confuse parsers
    assert_json_injection_rejected(
        &[b"CLASS\x00", b"CLASS\n", b"CLASS\t", b"CLASS\r"],
        &[b"SG\x00extra", b"SG\n"],
    );
}

#[test]
fn policy_json_injection_html_and_script() {
    // XSS-style payloads that might pass through to a UI
    assert_json_injection_rejected(
        &[b"<script>alert(1)</script>", b"CLASS<img>"],
        &[b"<div>SG</div>"],
    );
}

#[test]
fn policy_json_injection_spaces_and_whitespace() {
    // Spaces are not in the allowed set [a-zA-Z0-9._-]
    assert_json_injection_rejected(
        &[b"CLASS ONE", b" ACCREDITED", b"ACCREDITED "],
        &[b"S G", b" US"],
    );
}

#[test]
fn policy_json_injection_unicode_sequences() {
    // Raw UTF-8 multi-byte sequences
    assert_json_injection_rejected(
        &[
            // U+00E9 (e-acute) encoded as UTF-8
            &[0xC3, 0xA9],
            // U+0000 null in overlong UTF-8
            &[0xC0, 0x80],
        ],
        &[&[0xC3, 0xA9]],
    );
}

#[test]
fn policy_json_injection_empty_key_rejected() {
    // Empty investor_class or jurisdiction key must be rejected
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "policy key must not be empty"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut investor_classes = ManagedVec::new();
            investor_classes.push(ManagedBuffer::new());

            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                investor_classes,
                ManagedVec::new(),
            );
        });
}

#[test]
fn policy_json_injection_empty_jurisdiction_key_rejected() {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "policy key must not be empty"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut jurisdictions = ManagedVec::new();
            jurisdictions.push(ManagedBuffer::new());

            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                jurisdictions,
            );
        });
}

#[test]
fn policy_registry_owner_cannot_bypass_configured_governance() {
    let mut world = world();

    const NON_OWNER: TestAddress = TestAddress::new("non_owner");

    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world.account(NON_OWNER).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    // Step 1: owner cannot propose a replacement once governance is active.
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_governance(NON_OWNER.to_managed_address());
        });

    // Step 2: governance can call setTokenPolicy
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        });

    // Step 3: owner cannot revoke configured governance.
    world
        .tx()
        .from(OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.revoke_governance();
        });

    // Step 4: governance remains active after the rejected owner revoke.
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_2),
                true,
                false,
                true,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
            assert_eq!(envelope.operations.get(0).version, 1);
        });

    // Verify the policy was set correctly by governance.
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let policy = sc.token_policy(&ManagedBuffer::from(TOKEN_ID_2)).get();
            assert!(policy.drwa_enabled);
            assert!(policy.strict_auditor_mode);
        });
}

// ── MiCA White Paper CID Tests ────────────────────────────────────────

fn mica_whitebox_setup() -> (ScenarioWorld,) {
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });
    (world,)
}

#[test]
fn mica_set_white_paper_cid_v0() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            // CIDv0: 46 chars starting with "Qm"
            let cid = ManagedBuffer::from(b"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
            let envelope = sc.set_white_paper_cid(ManagedBuffer::from(TOKEN_ID_1), cid);
            assert!(envelope.caller_domain == DrwaCallerDomain::PolicyRegistry);
            assert_eq!(envelope.operations.get(0).version, 1);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let cid = sc.get_white_paper_cid(ManagedBuffer::from(TOKEN_ID_1));
            assert_eq!(
                cid,
                ManagedBuffer::from(b"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG")
            );
        });
}

#[test]
fn mica_set_white_paper_cid_v1() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            // CIDv1: 59 chars starting with "bafy"
            let cid =
                ManagedBuffer::from(b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi");
            let envelope = sc.set_white_paper_cid(ManagedBuffer::from(TOKEN_ID_1), cid);
            assert_eq!(envelope.operations.get(0).version, 1);
        });
}

#[test]
fn mica_rejects_empty_cid() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "white paper CID is required"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_white_paper_cid(ManagedBuffer::from(TOKEN_ID_1), ManagedBuffer::new());
        });
}

#[test]
fn mica_rejects_short_cid() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "invalid CID length: must be 46-64 characters",
        ))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_white_paper_cid(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"Qmshort"),
            );
        });
}

#[test]
fn mica_rejects_invalid_cid_prefix() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(
            4u64,
            "CID must start with Qm (v0) or bafy (v1)",
        ))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            // 46 chars but wrong prefix
            sc.set_white_paper_cid(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"ZzYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"),
            );
        });
}

#[test]
fn mica_set_registration_status_valid() {
    let (mut world,) = mica_whitebox_setup();

    let statuses: &[&[u8]] = &[
        b"draft",
        b"submitted",
        b"approved",
        b"rejected",
        b"withdrawn",
    ];
    for (i, status) in statuses.iter().enumerate() {
        world
            .tx()
            .from(GOVERNANCE)
            .to(SC_ADDRESS)
            .whitebox(drwa_policy_registry::contract_obj, |sc| {
                let envelope = sc.set_registration_status(
                    ManagedBuffer::from(TOKEN_ID_1),
                    ManagedBuffer::from(*status),
                );
                assert_eq!(envelope.operations.get(0).version, (i as u64) + 1);
            });
    }

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let status = sc.get_registration_status(ManagedBuffer::from(TOKEN_ID_1));
            assert_eq!(status, ManagedBuffer::from(b"withdrawn"));
        });
}

#[test]
fn mica_rejects_invalid_registration_status() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "invalid registration status: must be draft, submitted, approved, rejected, or withdrawn"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_registration_status(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"pending"),
            );
        });
}

#[test]
fn mica_white_paper_cid_access_control() {
    let (mut world,) = mica_whitebox_setup();

    const NON_OWNER: TestAddress = TestAddress::new("non_owner");
    world.account(NON_OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(NON_OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_white_paper_cid(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"),
            );
        });
}

#[test]
fn mica_registration_status_access_control() {
    let (mut world,) = mica_whitebox_setup();

    const NON_OWNER: TestAddress = TestAddress::new("non_owner");
    world.account(NON_OWNER).nonce(1).balance(1_000_000u64);

    world
        .tx()
        .from(NON_OWNER)
        .to(SC_ADDRESS)
        .returns(ExpectError(4u64, "caller not authorized"))
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_registration_status(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"draft"),
            );
        });
}

#[test]
fn mica_white_paper_cid_increments_version() {
    let (mut world,) = mica_whitebox_setup();

    // First set a token policy so the version starts at 1
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
        });

    // Verify version is 1 after set_token_policy
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            assert_eq!(
                sc.token_policy_version(&ManagedBuffer::from(TOKEN_ID_1))
                    .get(),
                1
            );
        });

    // setWhitePaperCid should increment version to 2
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.set_white_paper_cid(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"),
            );
            assert_eq!(envelope.operations.get(0).version, 2);
        });

    // Verify version is now 2
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            assert_eq!(
                sc.token_policy_version(&ManagedBuffer::from(TOKEN_ID_1))
                    .get(),
                2
            );
        });
}

#[test]
fn mica_registration_status_increments_version() {
    let (mut world,) = mica_whitebox_setup();

    // First set a token policy so the version starts at 1
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
        });

    // Verify version is 1 after set_token_policy
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            assert_eq!(
                sc.token_policy_version(&ManagedBuffer::from(TOKEN_ID_1))
                    .get(),
                1
            );
        });

    // setRegistrationStatus should increment version to 2
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.set_registration_status(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"draft"),
            );
            assert_eq!(envelope.operations.get(0).version, 2);
        });

    // Verify version is now 2
    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            assert_eq!(
                sc.token_policy_version(&ManagedBuffer::from(TOKEN_ID_1))
                    .get(),
                2
            );
        });

    // A second setRegistrationStatus should increment to 3
    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.set_registration_status(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"submitted"),
            );
            assert_eq!(envelope.operations.get(0).version, 3);
        });

    world
        .query()
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            assert_eq!(
                sc.token_policy_version(&ManagedBuffer::from(TOKEN_ID_1))
                    .get(),
                3
            );
        });
}

#[test]
fn policy_registry_identical_registration_status_is_noop() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
            let envelope = sc.set_registration_status(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"draft"),
            );
            assert_eq!(envelope.operations.get(0).version, 2);
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let envelope = sc.set_registration_status(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"draft"),
            );
            assert_eq!(envelope.operations.len(), 0);
            assert_eq!(
                sc.token_policy_version(&ManagedBuffer::from(TOKEN_ID_1))
                    .get(),
                2
            );
        });
}

#[test]
fn mica_registration_status_sync_preserves_existing_white_paper_cid() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
            sc.set_white_paper_cid(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"),
            );

            let envelope = sc.set_registration_status(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"submitted"),
            );
            let body = envelope.operations.get(0).body.to_boxed_bytes();
            let body_str = core::str::from_utf8(body.as_slice()).unwrap();
            assert!(body_str.contains(
                "\"white_paper_cid\":\"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG\""
            ));
            assert!(body_str.contains("\"registration_status\":\"submitted\""));
        });
}

#[test]
fn mica_white_paper_sync_preserves_existing_registration_status() {
    let (mut world,) = mica_whitebox_setup();

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                ManagedVec::new(),
                ManagedVec::new(),
            );
            sc.set_registration_status(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"draft"),
            );

            let envelope = sc.set_white_paper_cid(
                ManagedBuffer::from(TOKEN_ID_1),
                ManagedBuffer::from(b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"),
            );
            let body = envelope.operations.get(0).body.to_boxed_bytes();
            let body_str = core::str::from_utf8(body.as_slice()).unwrap();
            assert!(body_str.contains("\"white_paper_cid\":\"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi\""));
            assert!(body_str.contains("\"registration_status\":\"draft\""));
        });
}

#[test]
fn policy_json_safe_keys_accepted() {
    // Verify that legitimate keys with dots, underscores, and hyphens pass
    let mut world = world();
    world.account(OWNER).nonce(1).balance(1_000_000u64);
    world.account(GOVERNANCE).nonce(1).balance(1_000_000u64);
    world
        .tx()
        .from(OWNER)
        .raw_deploy()
        .code(CODE_PATH)
        .new_address(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            sc.init(GOVERNANCE.to_managed_address());
        });

    world
        .tx()
        .from(GOVERNANCE)
        .to(SC_ADDRESS)
        .whitebox(drwa_policy_registry::contract_obj, |sc| {
            let mut investor_classes = ManagedVec::new();
            investor_classes.push(ManagedBuffer::from(b"ACCREDITED"));
            investor_classes.push(ManagedBuffer::from(b"qualified-investor"));
            investor_classes.push(ManagedBuffer::from(b"tier.1"));
            investor_classes.push(ManagedBuffer::from(b"class_A"));
            investor_classes.push(ManagedBuffer::from(b"TYPE2B"));

            let mut jurisdictions = ManagedVec::new();
            jurisdictions.push(ManagedBuffer::from(b"SG"));
            jurisdictions.push(ManagedBuffer::from(b"US-CA"));
            jurisdictions.push(ManagedBuffer::from(b"EU.MIFID"));
            jurisdictions.push(ManagedBuffer::from(b"ISO_3166"));

            sc.set_token_policy(
                ManagedBuffer::from(TOKEN_ID_1),
                true,
                false,
                false,
                false,
                investor_classes,
                jurisdictions,
            );
        });
}
