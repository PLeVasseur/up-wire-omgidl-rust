# up-wire-omg-idl

`up-wire-omg-idl` is a non-published Rust 1.88 crate implementing a strict
OMG Interface Definition Language (IDL) XCDR1 little-endian selected-wire
payload codec for Eclipse uProtocol.
It composes with an up-rust encoded transport core through the native-prefix
selected-wire adapter; it is not a physical DDS transport.

## Payload Contract

OMG IDL defines the language-independent application type. OMG DDS-XTypes
defines the DDS type system and XCDR representation used here. The payload is
one complete DDS SerializedPayload value, including its
four-byte encapsulation header. The accepted representations are only XCDR1
little-endian `CDR_LE` (`0x0001`) and `PL_CDR_LE` (`0x0003`). The options byte,
terminal padding count, four-byte total alignment, and zero padding are checked.
Big-endian XCDR1, every XCDR2 representation, malformed values, non-canonical
scalar encodings, truncation, and trailing bytes are rejected.

After Dust DDS decodes a value, the codec canonically re-encodes it and requires
exact byte equality. This compensates for the generic Dust decoder accepting
multiple representations and not exposing consumed length. Both encode and
decode panics from the dependency are caught and returned as `UWireError`.

Payloads are limited to 16 MiB. Contiguous decode checks the limit before
calling Dust DDS. Reader decode checks before allocation, reads exactly the
declared length, and proves EOF. Dust DDS exposes a one-shot `Vec` serializer,
so encode can enforce the limit only immediately after that serializer returns;
callers must not use this codec to serialize already-unbounded in-memory values.

The normative reference IDL is `idl/vehicle_status_v1.idl`. Mapping, source,
and independent golden-fixture provenance are recorded in
[`docs/provenance.md`](docs/provenance.md).

## Identities

Compact IDs `0xD101` (wire) and `0xD102` (payload family) are distinct values in
up-rust's `0x8000..=0xFFFE` local/experimental range. Their literal IDs also say
`experimental`. They are provisional and are not globally registered
interoperability identities.

## Encoding Costs

XCDR1 size is dynamic for arbitrary IDL types. `payload_layout` performs one
complete probe serialization. `encode_payload` performs one serialization into
Dust DDS's temporary vector and copies the encoded prefix into the destination.
The overridden `encode_payload_owned` performs exactly one serialization and no
additional payload copy by this crate. A loaned flow that probes and then calls
direct encode therefore serializes twice. Strict contiguous decode includes one
canonical re-encode; reader decode adds one exact-length input copy.

Criterion reports these operations separately and names those costs explicitly.

## Validation

```text
cargo +1.88.0 check --locked --all-targets
cargo +1.88.0 test --locked --all-targets
cargo +1.95.0 fmt --check
cargo +1.95.0 clippy --locked --all-targets -- -D warnings
cargo +1.95.0 test --locked --all-targets
RUSTDOCFLAGS="-D warnings" cargo +1.95.0 doc --locked --no-deps
cargo +1.95.0 test --locked --doc
cargo +1.95.0 check --locked --benches
cargo +1.95.0 bench --locked --no-run
cargo +1.95.0 package --allow-dirty --list
cargo +1.95.0 tree --locked -e features
cargo deny check advisories licenses bans sources
```

## License

Apache-2.0. See `LICENSE` and `NOTICE`.
