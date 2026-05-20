# DRWA sync fixtures

These fixtures are Rust-owned golden bytes for the DRWA sync hook boundary.
`tests/drwa_sync_envelope_codec_test.rs` rebuilds each envelope with the live
Rust producer and asserts byte-for-byte equality with these files. Go tests in
`mx-chain-vm-go` and `mx-chain-go` read the same files to verify the VM counter,
VM hook forwarding, chain decoder, native key writes, and payload cap handling.

If the Rust sync envelope format intentionally changes:

1. Update the Rust builder/test cases first.
2. Regenerate these hex files from the live Rust builder output.
3. Run `cargo test -p drwa-common` and the Go DRWA fixture tests.

Go tests look for these fixtures through `DRWA_SYNC_FIXTURE_DIR` when set. In
the monorepo layout they fall back to the relative `mx-sdk-rs/.../testdata`
path.
