// SPDX-License-Identifier: Apache-2.0
//! Strict OMG IDL XCDR1 little-endian selected-wire support for Eclipse uProtocol.
//!
//! Payloads are complete DDS SerializedPayload values: a four-byte encapsulation
//! header followed by one XCDR1 value and canonical zero padding. Only `CDR_LE`
//! (`0x0001`) and `PL_CDR_LE` (`0x0003`) are accepted. Big-endian and XCDR2
//! representations are rejected before Dust DDS is invoked.
//!
//! Decoding is deliberately stricter than Dust DDS's generic decoder. The
//! decoded value is encoded again and must reproduce every input byte. This
//! proves full consumption, rejects trailing data and non-canonical scalar or
//! padding encodings, and binds the representation to the payload type.

use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe};

use bytes::Bytes;
use dust_dds::infrastructure::type_support::TypeSupport;
use dust_dds::xtypes::deserializer::CdrDeserializer;
use dust_dds::xtypes::serializer::Cdr1LeSerializer;
use up_rust::selected_wire_user_api::{UNativePrefixWireTransport, UWithNativePrefixWire};
use up_rust::wire_implementer_api::{
    UProtocolNativeWire, UWire, UWirePayload, WireIdentity, NATIVE_PREFIX_METADATA_LAYOUT_ID,
};
use up_rust::{
    DecodePayload, EncodePayload, PayloadEncoding, PayloadFormat, PayloadLayout, ReadDecodePayload,
    UWireError,
};

/// Maximum accepted or produced OMG IDL payload size (16 MiB).
pub const MAX_OMG_IDL_PAYLOAD_LEN: usize = 16 * 1024 * 1024;

/// Provisional local/experimental selected-wire identity.
///
/// Compact ID `0xD101` is not a globally registered identity.
pub const OMG_IDL_WIRE_ID: WireIdentity = WireIdentity::new(
    "org.eclipse.uprotocol.wire.omgidl-xcdr1-le.experimental",
    0xD101,
);

/// Provisional local/experimental payload-family identity.
///
/// Compact ID `0xD102` is not a globally registered identity.
pub const OMG_IDL_PAYLOAD_FAMILY_ID: WireIdentity = WireIdentity::new(
    "org.eclipse.uprotocol.payload.omgidl-xcdr1-le.experimental",
    0xD102,
);

/// Payload encoding identifier carried in frame metadata.
pub const OMG_IDL_ENCODING_ID: &str = "up.omgidl-xcdr1-le";

/// Media type for the strict XCDR1 little-endian payload profile.
pub const OMG_IDL_CONTENT_TYPE: &str = "application/vnd.omg.dds.xcdr1;endianness=little";

const ENCAPSULATION_HEADER_LEN: usize = 4;
const CDR_LE: [u8; 2] = [0x00, 0x01];
const PL_CDR_LE: [u8; 2] = [0x00, 0x03];

/// OMG IDL XCDR1 little-endian selected-wire marker.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OmgIdlWire;

/// Native-prefix transport shape for [`OmgIdlWire`].
pub type OmgIdlNativePrefixTransport<TCore> = UNativePrefixWireTransport<TCore, OmgIdlWire>;

/// Wraps an encoded transport core with the OMG IDL native-prefix selected wire.
#[must_use]
pub fn with_omgidl_native_prefix<TCore>(core: TCore) -> OmgIdlNativePrefixTransport<TCore> {
    core.into_native_prefix_wire_transport(OmgIdlWire)
}

impl UWire for OmgIdlWire {
    const WIRE_ID: WireIdentity = OMG_IDL_WIRE_ID;
    const PAYLOAD_FAMILY_ID: WireIdentity = OMG_IDL_PAYLOAD_FAMILY_ID;
    const METADATA_LAYOUT_ID: WireIdentity = NATIVE_PREFIX_METADATA_LAYOUT_ID;
    const FORMAT_VERSION: u16 = UProtocolNativeWire::FORMAT_VERSION;
}

impl PayloadFormat for OmgIdlWire {
    fn name() -> &'static str {
        "omgidl-xcdr1-le"
    }

    fn encoding() -> PayloadEncoding {
        PayloadEncoding::custom(OMG_IDL_ENCODING_ID, OMG_IDL_CONTENT_TYPE)
            .expect("static OMG IDL payload encoding is valid")
    }
}

/// Marker for Dust DDS type-support values carried by this wire.
///
/// Dust DDS consumes values while creating dynamic samples, so encoding clones
/// the value. The blanket implementation intentionally adds no compatibility
/// shim for prototype-specific payload traits.
pub trait OmgIdlPayload: TypeSupport + Clone {}

impl<T> OmgIdlPayload for T where T: TypeSupport + Clone {}

impl<T> UWirePayload<T> for OmgIdlWire
where
    T: OmgIdlPayload,
{
    type Codec = Self;
}

impl<T> EncodePayload<T> for OmgIdlWire
where
    T: OmgIdlPayload,
{
    fn payload_layout(value: &T) -> Result<PayloadLayout, UWireError> {
        PayloadLayout::new(encode_xcdr1(value)?.len(), 1)
    }

    fn encode_payload(value: &T, dst: &mut [u8]) -> Result<(), UWireError> {
        let bytes = encode_xcdr1(value)?;
        let actual = dst.len();
        let out = dst
            .get_mut(..bytes.len())
            .ok_or_else(|| UWireError::buffer_too_small(bytes.len(), actual))?;
        out.copy_from_slice(&bytes);
        Ok(())
    }

    fn encode_payload_owned(value: &T) -> Result<Bytes, UWireError> {
        encode_xcdr1(value).map(Bytes::from)
    }
}

impl<'a, T> DecodePayload<'a, T> for OmgIdlWire
where
    T: OmgIdlPayload,
{
    fn decode_payload(src: &'a [u8]) -> Result<T, UWireError> {
        decode_xcdr1(src)
    }
}

impl<T> ReadDecodePayload<T> for OmgIdlWire
where
    T: OmgIdlPayload,
{
    fn decode_payload_from_reader<R: Read>(
        mut reader: R,
        payload_len: usize,
    ) -> Result<T, UWireError> {
        ensure_payload_limit(payload_len)?;
        let mut bytes = vec![0_u8; payload_len];
        reader.read_exact(&mut bytes).map_err(|error| {
            UWireError::invalid_payload(format!(
                "OMG IDL payload reader did not yield the declared {payload_len} bytes: {error}"
            ))
        })?;

        let mut extra = [0_u8; 1];
        match reader.read(&mut extra) {
            Ok(0) => decode_xcdr1(&bytes),
            Ok(_) => Err(UWireError::invalid_payload(format!(
                "OMG IDL payload reader yielded more than the declared {payload_len} bytes"
            ))),
            Err(error) => Err(UWireError::invalid_payload(format!(
                "OMG IDL payload reader failed while proving exact length: {error}"
            ))),
        }
    }
}

fn encode_xcdr1<T: OmgIdlPayload>(value: &T) -> Result<Vec<u8>, UWireError> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Cdr1LeSerializer::serialize(&value.clone().create_dynamic_sample())
    }))
    .map_err(|_| UWireError::serialization_error("OMG IDL XCDR1-LE encoder panicked"))?;
    let bytes = result.map_err(|error| {
        UWireError::serialization_error(format!("OMG IDL XCDR1-LE encode failed: {error:?}"))
    })?;
    ensure_payload_limit_for_encode(bytes.len())?;
    validate_encapsulation(&bytes).map_err(|error| {
        UWireError::serialization_error(format!(
            "Dust DDS produced bytes outside the strict XCDR1-LE profile: {error}"
        ))
    })?;
    Ok(bytes)
}

fn decode_xcdr1<T: OmgIdlPayload>(src: &[u8]) -> Result<T, UWireError> {
    ensure_payload_limit(src.len())?;
    validate_encapsulation(src)?;

    let value = catch_unwind(AssertUnwindSafe(|| {
        let dynamic = CdrDeserializer::deserialize(T::get_type(), src)?;
        Ok::<T, dust_dds::xtypes::error::XTypesError>(T::create_sample(dynamic))
    }))
    .map_err(|_| UWireError::invalid_payload("OMG IDL XCDR1-LE decoder panicked"))?
    .map_err(|error| {
        UWireError::invalid_payload(format!("OMG IDL XCDR1-LE decode failed: {error:?}"))
    })?;

    let canonical = encode_xcdr1(&value).map_err(|error| {
        UWireError::invalid_payload(format!("OMG IDL canonical validation failed: {error}"))
    })?;
    if canonical != src {
        return Err(UWireError::invalid_payload(
            "OMG IDL payload is not the exact canonical XCDR1-LE encoding for its decoded value",
        ));
    }
    Ok(value)
}

fn validate_encapsulation(src: &[u8]) -> Result<(), UWireError> {
    let representation = src
        .get(..2)
        .ok_or_else(|| UWireError::invalid_payload("OMG IDL encapsulation header is truncated"))?;
    if representation != CDR_LE && representation != PL_CDR_LE {
        return Err(UWireError::invalid_payload(format!(
            "OMG IDL representation {representation:02x?} is not CDR_LE (0x0001) or PL_CDR_LE (0x0003)"
        )));
    }

    let options = src.get(2..ENCAPSULATION_HEADER_LEN).ok_or_else(|| {
        UWireError::invalid_payload("OMG IDL encapsulation options are truncated")
    })?;
    let reserved = options
        .first()
        .copied()
        .ok_or_else(|| UWireError::invalid_payload("OMG IDL options byte is missing"))?;
    let padding = usize::from(
        options
            .get(1)
            .copied()
            .ok_or_else(|| UWireError::invalid_payload("OMG IDL padding byte is missing"))?,
    );
    if reserved != 0 || padding > 3 {
        return Err(UWireError::invalid_payload(format!(
            "OMG IDL encapsulation options are invalid: reserved={reserved}, padding={padding}"
        )));
    }
    if !src.len().is_multiple_of(4) {
        return Err(UWireError::invalid_payload(
            "OMG IDL SerializedPayload length is not a multiple of four",
        ));
    }
    let payload_len = src
        .len()
        .checked_sub(ENCAPSULATION_HEADER_LEN)
        .ok_or_else(|| UWireError::invalid_payload("OMG IDL payload length underflow"))?;
    if padding > payload_len {
        return Err(UWireError::invalid_payload(
            "OMG IDL padding exceeds the serialized value length",
        ));
    }
    let padding_start = src
        .len()
        .checked_sub(padding)
        .ok_or_else(|| UWireError::invalid_payload("OMG IDL padding length underflow"))?;
    let padding_bytes = src
        .get(padding_start..)
        .ok_or_else(|| UWireError::invalid_payload("OMG IDL padding range is invalid"))?;
    if padding_bytes.iter().any(|byte| *byte != 0) {
        return Err(UWireError::invalid_payload(
            "OMG IDL encapsulation padding must contain only zero bytes",
        ));
    }
    Ok(())
}

fn ensure_payload_limit(len: usize) -> Result<(), UWireError> {
    if len > MAX_OMG_IDL_PAYLOAD_LEN {
        return Err(UWireError::invalid_payload(format!(
            "OMG IDL payload length {len} exceeds the {MAX_OMG_IDL_PAYLOAD_LEN}-byte limit"
        )));
    }
    Ok(())
}

fn ensure_payload_limit_for_encode(len: usize) -> Result<(), UWireError> {
    if len > MAX_OMG_IDL_PAYLOAD_LEN {
        return Err(UWireError::serialization_error(format!(
            "OMG IDL encoded payload length {len} exceeds the {MAX_OMG_IDL_PAYLOAD_LEN}-byte limit"
        )));
    }
    Ok(())
}

/// Reference payload defined normatively by `idl/vehicle_status_v1.idl`.
#[derive(dust_dds::infrastructure::type_support::DdsType, Debug, Clone, PartialEq)]
pub struct VehicleStatusV1 {
    /// Sample timestamp in nanoseconds.
    pub stamp_ns: u64,
    /// Signed speed in millimeters per second.
    pub speed_mmps: i32,
    /// Steering angle in millidegrees.
    pub steering_mdeg: i32,
    /// Gear selector code.
    pub gear: u8,
    /// Whether the parking brake is engaged.
    pub park_brake: bool,
}

impl VehicleStatusV1 {
    /// Builds a deterministic fixture using wrapping timestamp arithmetic.
    #[must_use]
    pub fn fixture(seed: u64) -> Self {
        Self {
            stamp_ns: seed.wrapping_mul(1_000_003),
            speed_mmps: ((seed % 40_000) as i32) - 20_000,
            steering_mdeg: ((seed % 90_000) as i32) - 45_000,
            gear: (seed % 8) as u8,
            park_brake: seed.is_multiple_of(2),
        }
    }
}
