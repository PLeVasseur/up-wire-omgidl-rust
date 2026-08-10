// SPDX-License-Identifier: Apache-2.0

use std::hint::black_box;
use std::io::Cursor;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use up_rust::{DecodePayload, EncodePayload, ReadDecodePayload};
use up_wire_omgidl::{OmgIdlWire, VehicleStatusV1, OMG_IDL_DECODE_LIMIT};

fn wire_criterion(c: &mut Criterion) {
    let value = VehicleStatusV1::fixture(7);
    let encoded = <OmgIdlWire as EncodePayload<VehicleStatusV1>>::encode_payload_owned(&value)
        .expect("fixture encode");

    let mut group = c.benchmark_group("omgidl_xcdr1_le");
    group.throughput(Throughput::Bytes(encoded.len() as u64));

    group.bench_function("owned_encode_one_serialization", |b| {
        b.iter(|| {
            <OmgIdlWire as EncodePayload<VehicleStatusV1>>::encode_payload_owned(black_box(&value))
                .expect("owned encode")
        });
    });

    group.bench_function("layout_probe_one_serialization", |b| {
        b.iter(|| {
            <OmgIdlWire as EncodePayload<VehicleStatusV1>>::payload_layout(black_box(&value))
                .expect("layout probe")
        });
    });

    let mut destination = vec![0_u8; encoded.len()];
    group.bench_function("direct_encode_one_serialization_plus_copy", |b| {
        b.iter(|| {
            <OmgIdlWire as EncodePayload<VehicleStatusV1>>::encode_payload(
                black_box(&value),
                black_box(&mut destination),
            )
            .expect("direct encode");
        });
    });

    group.bench_function("contiguous_decode_plus_canonical_reencode", |b| {
        b.iter(|| {
            <OmgIdlWire as DecodePayload<'_, VehicleStatusV1>>::decode_payload(black_box(&encoded))
                .expect("contiguous decode")
        });
    });

    group.bench_function("reader_exact_copy_decode_plus_canonical_reencode", |b| {
        b.iter(|| {
            <OmgIdlWire as ReadDecodePayload<VehicleStatusV1>>::decode_payload_from_reader(
                black_box(Cursor::new(encoded.as_ref())),
                encoded.len(),
                OMG_IDL_DECODE_LIMIT,
            )
            .expect("reader decode")
        });
    });
    group.finish();
}

criterion_group!(benches, wire_criterion);
criterion_main!(benches);
