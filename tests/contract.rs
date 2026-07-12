// SPDX-License-Identifier: Apache-2.0

use std::io::{self, Cursor, Read};
use std::panic::{catch_unwind, AssertUnwindSafe};

use dust_dds::infrastructure::type_support::DdsType;
use up_rust::wire_implementer_api::{
    UProtocolNativeWire, UWire, WireIdentityRef, NATIVE_PREFIX_METADATA_LAYOUT_ID,
};
use up_rust::{DecodePayload, EncodePayload, ReadDecodePayload, UWireError};
use up_wire_omgidl::{
    OmgIdlWire, VehicleStatusV1, MAX_OMG_IDL_PAYLOAD_LEN, OMG_IDL_PAYLOAD_FAMILY_ID,
    OMG_IDL_WIRE_ID,
};

fn encode(value: &VehicleStatusV1) -> Vec<u8> {
    <OmgIdlWire as EncodePayload<VehicleStatusV1>>::encode_payload_owned(value)
        .expect("encode fixture")
        .to_vec()
}

fn decode(bytes: &[u8]) -> Result<VehicleStatusV1, UWireError> {
    <OmgIdlWire as DecodePayload<'_, VehicleStatusV1>>::decode_payload(bytes)
}

fn assert_invalid(result: Result<VehicleStatusV1, UWireError>) {
    assert!(matches!(result, Err(UWireError::InvalidPayload(_))));
}

#[test]
fn round_trip_and_layout_are_exact() {
    let value = VehicleStatusV1::fixture(42);
    let bytes = encode(&value);
    let layout = <OmgIdlWire as EncodePayload<VehicleStatusV1>>::payload_layout(&value)
        .expect("layout probe");
    assert_eq!(layout.len(), bytes.len());
    assert_eq!(layout.align(), 1);
    assert_eq!(decode(&bytes).expect("decode"), value);
}

#[test]
fn direct_encode_uses_prefix_semantics() {
    let value = VehicleStatusV1::fixture(9);
    let expected = encode(&value);
    let mut short = vec![0_u8; expected.len() - 1];
    assert_eq!(
        <OmgIdlWire as EncodePayload<VehicleStatusV1>>::encode_payload(&value, &mut short),
        Err(UWireError::BufferTooSmall {
            expected: expected.len(),
            actual: expected.len() - 1,
        })
    );

    let mut large = vec![0xA5; expected.len() + 5];
    <OmgIdlWire as EncodePayload<VehicleStatusV1>>::encode_payload(&value, &mut large)
        .expect("encode into larger destination");
    assert_eq!(&large[..expected.len()], expected.as_slice());
    assert!(large[expected.len()..].iter().all(|byte| *byte == 0xA5));
}

#[test]
fn exact_reader_rejects_short_and_overlong_sources() {
    let bytes = encode(&VehicleStatusV1::fixture(3));
    assert_invalid(
        <OmgIdlWire as ReadDecodePayload<VehicleStatusV1>>::decode_payload_from_reader(
            Cursor::new(&bytes[..bytes.len() - 1]),
            bytes.len(),
        ),
    );

    let mut overlong = bytes.clone();
    overlong.push(0xAA);
    assert_invalid(
        <OmgIdlWire as ReadDecodePayload<VehicleStatusV1>>::decode_payload_from_reader(
            Cursor::new(overlong),
            bytes.len(),
        ),
    );
}

struct PanicReader;

impl Read for PanicReader {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        panic!("oversized input must be rejected before reading")
    }
}

#[test]
fn oversized_reader_length_is_rejected_before_read_or_allocation() {
    assert_invalid(
        <OmgIdlWire as ReadDecodePayload<VehicleStatusV1>>::decode_payload_from_reader(
            PanicReader,
            MAX_OMG_IDL_PAYLOAD_LEN + 1,
        ),
    );
}

#[test]
fn oversized_contiguous_payload_is_rejected() {
    let oversized = vec![0_u8; MAX_OMG_IDL_PAYLOAD_LEN + 1];
    assert_invalid(decode(&oversized));
}

#[test]
fn wrong_endianness_and_xcdr2_representations_are_rejected() {
    let bytes = encode(&VehicleStatusV1::fixture(5));
    for representation in [[0x00, 0x00], [0x00, 0x06], [0x00, 0x07], [0x00, 0x09]] {
        let mut malformed = bytes.clone();
        malformed[..2].copy_from_slice(&representation);
        assert_invalid(decode(&malformed));
    }
}

#[test]
fn representation_and_padding_count_must_be_canonical_for_the_type() {
    let bytes = encode(&VehicleStatusV1::fixture(5));

    let mut parameter_list_for_final_type = bytes.clone();
    parameter_list_for_final_type[..2].copy_from_slice(&[0x00, 0x03]);
    assert_invalid(decode(&parameter_list_for_final_type));

    let mut wrong_padding_count = bytes;
    wrong_padding_count[3] = 0;
    assert_invalid(decode(&wrong_padding_count));
}

#[test]
fn invalid_options_padding_and_trailing_data_are_rejected() {
    let bytes = encode(&VehicleStatusV1::fixture(5));

    let mut reserved = bytes.clone();
    reserved[2] = 1;
    assert_invalid(decode(&reserved));

    let mut excessive_padding = bytes.clone();
    excessive_padding[3] = 4;
    assert_invalid(decode(&excessive_padding));

    let mut nonzero_padding = bytes.clone();
    let last = nonzero_padding.len() - 1;
    nonzero_padding[last] = 1;
    assert_invalid(decode(&nonzero_padding));

    let mut trailing = bytes;
    trailing.extend_from_slice(&[0, 0, 0, 0]);
    assert_invalid(decode(&trailing));
}

#[derive(DdsType, Clone)]
struct WrongType {
    first: u64,
    second: u64,
    third: u64,
    fourth: u64,
}

#[test]
fn wrong_payload_type_is_rejected() {
    let bytes = encode(&VehicleStatusV1::fixture(11));
    let result = <OmgIdlWire as DecodePayload<'_, WrongType>>::decode_payload(&bytes);
    assert!(matches!(result, Err(UWireError::InvalidPayload(_))));
}

#[test]
fn every_truncation_and_a_deterministic_mutation_corpus_is_panic_free() {
    let bytes = encode(&VehicleStatusV1::fixture(17));
    for end in 0..bytes.len() {
        let outcome = catch_unwind(AssertUnwindSafe(|| decode(&bytes[..end])));
        assert!(outcome.is_ok(), "decoder panicked for truncation {end}");
        assert_invalid(outcome.expect("panic checked"));
    }

    for index in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[index] ^= 0xFF;
        let outcome = catch_unwind(AssertUnwindSafe(|| decode(&mutated)));
        assert!(outcome.is_ok(), "decoder panicked for mutation {index}");
        let _ = outcome.expect("panic checked");
    }
}

#[test]
fn identities_are_distinct_experimental_values() {
    assert_ne!(OMG_IDL_WIRE_ID, OMG_IDL_PAYLOAD_FAMILY_ID);
    for identity in [OMG_IDL_WIRE_ID, OMG_IDL_PAYLOAD_FAMILY_ID] {
        assert!((0x8000..=0xFFFE).contains(&identity.compact_id()));
        assert!(identity.literal_id().contains("experimental"));
    }
    assert_eq!(OmgIdlWire::WIRE_ID, OMG_IDL_WIRE_ID);
    assert_eq!(OmgIdlWire::PAYLOAD_FAMILY_ID, OMG_IDL_PAYLOAD_FAMILY_ID);
    assert_eq!(
        OmgIdlWire::METADATA_LAYOUT_ID,
        NATIVE_PREFIX_METADATA_LAYOUT_ID
    );
    assert_eq!(
        OmgIdlWire::FORMAT_VERSION,
        UProtocolNativeWire::FORMAT_VERSION
    );
    assert!(OmgIdlWire::wire_compatibility(&WireIdentityRef::Compact(
        OMG_IDL_WIRE_ID.compact_id()
    ))
    .is_compatible());
    assert!(!OmgIdlWire::wire_compatibility(&WireIdentityRef::Compact(0xA201)).is_compatible());
    assert!(
        !OmgIdlWire::payload_family_compatibility(&WireIdentityRef::Compact(0xD103))
            .is_compatible()
    );
}
