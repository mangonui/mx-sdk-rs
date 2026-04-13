#![no_std]
//! Minimal test-harness contract that exposes drwa-common validation and
//! serialization functions as endpoints so they can be exercised via the
//! standard MultiversX whitebox test infrastructure.

multiversx_sc::imports!();

use drwa_common::{
    DrwaCallerDomain, DrwaSyncOperation, DrwaSyncOperationType,
    push_len_prefixed, require_valid_aml_status, require_valid_kyc_status,
    require_valid_token_id, serialize_sync_envelope_payload,
};

#[multiversx_sc::contract]
pub trait DrwaCommonTestHarness {
    #[init]
    fn init(&self) {}

    #[upgrade]
    fn upgrade(&self) {}

    /// Wraps `require_valid_token_id` so it can be called as an endpoint.
    #[endpoint(validateTokenId)]
    fn validate_token_id(&self, token_id: ManagedBuffer) {
        require_valid_token_id::<Self::Api>(&token_id);
    }

    /// Wraps `require_valid_kyc_status` so it can be called as an endpoint.
    #[endpoint(validateKycStatus)]
    fn validate_kyc_status(&self, status: ManagedBuffer) {
        require_valid_kyc_status::<Self::Api>(&status);
    }

    /// Wraps `require_valid_aml_status` so it can be called as an endpoint.
    #[endpoint(validateAmlStatus)]
    fn validate_aml_status(&self, status: ManagedBuffer) {
        require_valid_aml_status::<Self::Api>(&status);
    }

    /// Wraps `push_len_prefixed` and returns the result for assertion.
    #[endpoint(testPushLenPrefixed)]
    fn test_push_len_prefixed(&self, value: ManagedBuffer) -> ManagedBuffer {
        let mut dest = ManagedBuffer::new();
        push_len_prefixed(&mut dest, &value);
        dest
    }

    /// Wraps `serialize_sync_envelope_payload` with a single TokenPolicy
    /// operation and returns the serialized bytes for assertion.
    #[endpoint(testSerializeSyncPayload)]
    fn test_serialize_sync_payload(
        &self,
        caller_domain_tag: u8,
        op_type_tag: u8,
        token_id: ManagedBuffer,
        holder: ManagedAddress,
        version: u64,
        body: ManagedBuffer,
    ) -> ManagedBuffer {
        let caller_domain = match caller_domain_tag {
            0 => DrwaCallerDomain::PolicyRegistry,
            1 => DrwaCallerDomain::AssetManager,
            2 => DrwaCallerDomain::IdentityRegistry,
            3 => DrwaCallerDomain::Attestation,
            4 => DrwaCallerDomain::RecoveryAdmin,
            _ => sc_panic!("invalid caller domain tag"),
        };
        let op_type = match op_type_tag {
            0 => DrwaSyncOperationType::TokenPolicy,
            1 => DrwaSyncOperationType::AssetRecord,
            2 => DrwaSyncOperationType::HolderMirror,
            3 => DrwaSyncOperationType::HolderProfile,
            4 => DrwaSyncOperationType::HolderAuditorAuthorization,
            5 => DrwaSyncOperationType::HolderMirrorDelete,
            _ => sc_panic!("invalid op type tag"),
        };

        let mut operations = ManagedVec::new();
        operations.push(DrwaSyncOperation {
            operation_type: op_type,
            token_id,
            holder,
            version,
            body,
        });

        serialize_sync_envelope_payload(&caller_domain, &operations)
    }
}
