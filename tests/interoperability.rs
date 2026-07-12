// SPDX-License-Identifier: Apache-2.0

use std::io::Cursor;

use up_rust::{DecodePayload, EncodePayload, ReadDecodePayload};
use up_wire_dds_idl::{DdsIdlWire, VehicleStatusV1};

const INDEPENDENT_XCDR1_LE_HEX: &str = include_str!("fixtures/vehicle_status_v1_xcdr1_le.hex");

fn independent_fixture_bytes() -> Vec<u8> {
    INDEPENDENT_XCDR1_LE_HEX
        .split_ascii_whitespace()
        .map(|octet| u8::from_str_radix(octet, 16).expect("fixture contains hexadecimal octets"))
        .collect()
}

fn independent_fixture_value() -> VehicleStatusV1 {
    VehicleStatusV1 {
        stamp_ns: 0x0102_0304_0506_0708,
        speed_mmps: -2,
        steering_mdeg: 0x1122_3344,
        gear: 7,
        park_brake: true,
    }
}

#[test]
fn independent_normative_fixture_decodes() {
    let bytes = independent_fixture_bytes();
    let actual = <DdsIdlWire as DecodePayload<'_, VehicleStatusV1>>::decode_payload(&bytes)
        .expect("decode independently authored fixture");
    assert_eq!(actual, independent_fixture_value());
}

#[test]
fn dust_encoding_matches_independent_normative_fixture() {
    let encoded = <DdsIdlWire as EncodePayload<VehicleStatusV1>>::encode_payload_owned(
        &independent_fixture_value(),
    )
    .expect("encode fixture");
    assert_eq!(encoded.as_ref(), independent_fixture_bytes());
}

#[test]
fn independent_fixture_decodes_from_exact_reader() {
    let bytes = independent_fixture_bytes();
    let actual = <DdsIdlWire as ReadDecodePayload<VehicleStatusV1>>::decode_payload_from_reader(
        Cursor::new(&bytes),
        bytes.len(),
    )
    .expect("decode exact reader");
    assert_eq!(actual, independent_fixture_value());
}

#[test]
fn normative_idl_contains_the_tested_field_order_and_final_extensibility() {
    let idl = include_str!("../idl/vehicle_status_v1.idl");
    let required_fragments = [
        "@final",
        "struct VehicleStatusV1",
        "unsigned long long stamp_ns;",
        "long speed_mmps;",
        "long steering_mdeg;",
        "octet gear;",
        "boolean park_brake;",
    ];
    let mut previous = 0;
    for fragment in required_fragments {
        let relative = idl
            .get(previous..)
            .and_then(|remainder| remainder.find(fragment))
            .expect("normative IDL fragment is present in order");
        previous += relative + fragment.len();
    }
}
