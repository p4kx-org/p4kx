// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_model::{IncludedObjectKind, IncludedObjectTables, IncludedObjects, SocEntityFile};
use p4kx_testkit::{SOC_IDENTITY, TestChunkFile, TestSoc};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const INCLUDED_OBJECTS: u32 = 0x0010;
const NAME_LEN: usize = 256;

#[test]
fn preserves_per_chunk_path_slots_and_layouts() -> TestResult {
    let first = first_payload()?;
    let second = TestSoc::new()
        .layout(15)
        .model("y.cga")
        .object(0, SOC_IDENTITY);
    let bytes = TestChunkFile::crch()
        .chunk(0x0002, 5, [0; 4])
        .chunk(INCLUDED_OBJECTS, 1, first.bytes)
        .chunk(INCLUDED_OBJECTS, 1, second.chunk())
        .bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let mut chunks = file.included_objects_chunks();
    let first_chunk = chunks.next().ok_or("first physical chunk")?;
    let second = chunks.next().ok_or("second physical chunk")?;
    assert_eq!(
        (first_chunk.chunk_index(), second.chunk_index()),
        (1, 2),
        "physical positions"
    );
    assert_eq!(first_chunk.header().version(), 1, "original chunk version");
    assert_eq!(
        first_chunk.header().size(),
        first_chunk.bytes().len() as u64,
        "physical chunk length"
    );
    let (first_tables, second) = (first_chunk.tables()?, second.tables()?);
    assert_eq!(
        (first_tables.layout(), second.layout()),
        (8, 15),
        "original layout words"
    );
    verify_path_slots(&first_tables, first.material_start, &first.material)?;
    assert_eq!(
        range_length(
            second
                .rows()
                .next()
                .ok_or("second table placement")?
                .range()
        ),
        208,
        "per-layout framing"
    );
    let preview = IncludedObjects::read(&bytes)?.ok_or("object preview")?;
    assert_eq!(
        preview.objects.len(),
        2,
        "both chunks supply preview placements"
    );
    assert_eq!(
        (preview.invalid, preview.skipped),
        (1, 1),
        "original preview dispositions"
    );
    Ok(())
}

const PRECISE: [[f64; 4]; 3] = [
    [1.0, 0.0, 0.0, 1.0 + f64::EPSILON],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];
const NAN_WORD: u64 = 0x7ff8_0000_0000_001e;

struct Fixture {
    bytes: Vec<u8>,
    material_start: usize,
    material: [u8; NAME_LEN],
}

fn first_payload() -> Result<Fixture, Box<dyn std::error::Error>> {
    let mut bytes = TestSoc::new()
        .layout(8)
        .model("x.cga")
        .model("x.cga")
        .model("")
        .palette("Palette")
        .object_with(1, PRECISE, 0)
        .skipped_object()
        .object_with(9, [[f64::from_bits(NAN_WORD); 4]; 3], 255)
        .chunk();
    let material_count = 8 + NAME_LEN * 3;
    bytes
        .get_mut(material_count..material_count + 2)
        .ok_or("material count field")?
        .copy_from_slice(&1_u16.to_le_bytes());
    let material_start = material_count + 4;
    let mut material = [0; NAME_LEN];
    material
        .first_chunk_mut::<5>()
        .ok_or("material slot")?
        .copy_from_slice(b" m\xff\0!");
    bytes.splice(material_start..material_start, material);
    bytes.extend_from_slice(&[17, 32]);
    Ok(Fixture {
        bytes,
        material_start,
        material,
    })
}

fn verify_path_slots(
    tables: &IncludedObjectTables<'_>,
    material_start: usize,
    material: &[u8; NAME_LEN],
) -> TestResult {
    let models: Vec<_> = tables
        .models()
        .map(|slot| (slot.ordinal(), slot.path_bytes()))
        .collect();
    assert_eq!(
        models,
        [
            (0, b"x.cga".as_slice()),
            (1, b"x.cga".as_slice()),
            (2, b"".as_slice())
        ],
        "duplicate and empty slots keep original positions"
    );
    let slot = tables.materials().next().ok_or("material table slot")?;
    assert_eq!(slot.path_bytes(), b" m\xff", "exact lexical path bytes");
    assert_eq!(slot.bytes(), material, "post-NUL bytes remain addressable");
    let extent = material_start as u64..(material_start + NAME_LEN) as u64;
    assert_eq!(slot.range(), extent, "material slot range");
    assert_eq!(tables.material_range(), extent, "material table range");
    let palette = tables.palettes().next().ok_or("palette table slot")?;
    assert_eq!(
        palette.path_bytes(),
        b"Palette",
        "palette table start follows material slots"
    );
    assert_eq!(
        tables.trailing_bytes(),
        &[17, 32],
        "bytes beyond declared object extent"
    );
    Ok(())
}

#[test]
fn positions_interleaved_records_and_preserves_exact_matrix_words() -> TestResult {
    let fixture = first_payload()?;
    let bytes = TestChunkFile::crch()
        .chunk(INCLUDED_OBJECTS, 1, fixture.bytes)
        .bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let tables = file
        .included_objects_chunks()
        .next()
        .ok_or("physical chunk")?
        .tables()?;
    let mut rows = tables.rows();
    let first = rows.next().ok_or("first placement")?;
    let skipped = rows.next().ok_or("kind 16 row")?;
    let bad = rows.next().ok_or("invalid for preview placement")?;
    assert_eq!(
        tables.object_range().start,
        first.range().start,
        "table start"
    );
    assert_eq!(tables.object_range().end, bad.range().end, "table end");
    assert_eq!(
        range_length(first.range()),
        176,
        "layout-eight placement length"
    );
    assert_eq!(
        (first.ordinal(), skipped.ordinal(), bad.ordinal()),
        (0, 1, 2),
        "physical positions"
    );
    assert!(
        matches!(skipped.kind(), IncludedObjectKind::Kind16),
        "kind 16 is a positioned row"
    );
    verify_matrix_words(first.kind(), bad.kind())
}

fn verify_matrix_words(first: IncludedObjectKind<'_>, bad: IncludedObjectKind<'_>) -> TestResult {
    let (IncludedObjectKind::Placement(first), IncludedObjectKind::Placement(bad)) = (first, bad)
    else {
        return Err("physical placement records expected".into());
    };
    assert_eq!(
        (first.model_index(), first.palette_byte()),
        (1, 0),
        "valid selectors"
    );
    assert_eq!(
        (bad.model_index(), bad.palette_byte()),
        (9, 255),
        "invalid and sentinel selectors"
    );
    assert_eq!(
        first.transform_words(),
        PRECISE.map(|row| row.map(f64::to_bits)),
        "exact binary64 words"
    );
    assert_eq!(
        bad.transform_words(),
        [[NAN_WORD; 4]; 3],
        "exact nonfinite words"
    );
    assert_eq!(
        range_length(first.transform_range()),
        96,
        "original matrix span"
    );
    assert_eq!(
        range_length(first.model_index_range()),
        2,
        "original model selector span"
    );
    assert_eq!(
        range_length(first.palette_range()),
        1,
        "original palette selector span"
    );
    Ok(())
}

const fn range_length(range: Range<u64>) -> u64 {
    range.end - range.start
}
