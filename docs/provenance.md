# IDL And Fixture Provenance

OMG IDL 4.2 defines language- and platform-independent data types and
interfaces. DDS-XTypes 1.3 defines the DDS topic type system and XCDR
serialization used by this implementation. The product is therefore named for
its application-facing OMG IDL contract; Dust DDS is the Rust implementation
used to materialize the DDS-XTypes/XCDR1 representation.

## Normative Type

`idl/vehicle_status_v1.idl` is the normative application type definition. The
Rust `VehicleStatusV1` is its checked mapping. The integration test reads the
IDL at compile time and verifies final extensibility, declaration name, field
order, and field spellings.

The mapping was materialized with the Dust DDS 0.15.0 `#[derive(DdsType)]`
route, not copied from generated prototype output. The field mapping is:

| IDL | Rust | XCDR1 alignment |
| --- | --- | --- |
| `unsigned long long stamp_ns` | `u64` | 8 |
| `long speed_mmps` | `i32` | 4 |
| `long steering_mdeg` | `i32` | 4 |
| `octet gear` | `u8` | 1 |
| `boolean park_brake` | `bool` | 1 |

The dependency is locked to Dust DDS 0.15.0. A change to Dust DDS, the derive
route, the IDL, or the Rust field order requires regenerating and reviewing the
golden evidence.

## Independent Golden Fixture

`tests/fixtures/vehicle_status_v1_xcdr1_le.hex` was authored independently of
the Dust DDS serializer from the OMG DDSI-RTPS SerializedPayload encapsulation
and OMG XTypes XCDR1 alignment rules. Its value is:

```text
stamp_ns      = 0x0102030405060708
speed_mmps    = -2
steering_mdeg = 0x11223344
gear          = 7
park_brake    = true
```

The first four octets are the `CDR_LE` representation identifier and options;
the options report two terminal padding octets. The fixture then contains the
fields in normative IDL order and two zero padding octets. No Dust DDS API is
used to construct the expected bytes. Tests prove both that Dust-produced bytes
equal this independent fixture and that the production decoder accepts it.

This is independent wire-codec evidence, not a claim of DDS discovery, RTPS
transport, or native DDS shared-memory interoperability. Those belong to the
physical DDS transport validation phase.

## Representation Sources

- OMG IDL 4.2: <https://www.omg.org/spec/IDL/4.2/About-IDL>.
- OMG DDS-XTypes 1.3: <https://www.omg.org/spec/DDS-XTypes/1.3/About-DDS-XTypes>.
- OMG DDSI-RTPS 2.5: <https://www.omg.org/spec/DDSI-RTPS/2.5/About-DDSI-RTPS>.
- OMG DDSI-RTPS 2.5, SerializedPayload and RepresentationIdentifier tables.
- OMG XTypes 1.3, XCDR version 1 serialization and alignment rules.
- Dust DDS 0.15.0 `Cdr1LeSerializer` and `CdrDeserializer` implementation.
- up-rust revision `c1d13ca31859b9cc0086ef5d4b953907f24094ec` selected-wire API.
