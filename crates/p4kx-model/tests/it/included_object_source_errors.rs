// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_model::{
    IncludedObjectKind, IncludedObjects, IncludedObjectsError, MAX_INCLUDED_MODELS, SocEntityFile,
};
use p4kx_testkit::{SOC_IDENTITY, TestChunkFile, TestSoc};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const INCLUDED_OBJECTS: u32 = 0x0010;
const NAME_LEN: usize = 256;

#[test]
fn unknown_and_truncated_bytes_are_positioned_without_guessing_the_next_row() -> TestResult {
    let unknown = TestSoc::new()
        .model("x.cga")
        .object_of_kind(7)
        .object(0, SOC_IDENTITY);
    let unmeasured = TestSoc::new().layout(11).object(0, SOC_IDENTITY);
    let mut truncated = TestSoc::new().skipped_object().chunk();
    let at = truncated.len() - 136;
    truncated.truncate(at + 6);
    truncated
        .get_mut(at - 4..at)
        .ok_or("table length field")?
        .copy_from_slice(&6_u32.to_le_bytes());
    let bytes = TestChunkFile::crch()
        .chunk(INCLUDED_OBJECTS, 1, unknown.chunk())
        .chunk(INCLUDED_OBJECTS, 1, unmeasured.chunk())
        .chunk(INCLUDED_OBJECTS, 1, truncated)
        .bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let mut chunks = file.included_objects_chunks();
    let unknown = chunks.next().ok_or("unknown kind chunk")?.tables()?;
    let layout = chunks.next().ok_or("unknown layout chunk")?.tables()?;
    let truncated = chunks.next().ok_or("partial row chunk")?.tables()?;
    let mut rows = unknown.rows();
    let tail = rows.next().ok_or("unknown kind tail")?;
    assert!(
        matches!(tail.kind(), IncludedObjectKind::UnmeasuredKind { kind: 7 }),
        "preserve unknown kind without scanning for the plausible next row"
    );
    assert_eq!(
        tail.range(),
        unknown.object_range(),
        "unknown kind bounds the whole remainder"
    );
    assert!(rows.next().is_none(), "no fictitious source rows");
    assert!(
        matches!(
            layout.rows().next().ok_or("unknown layout tail")?.kind(),
            IncludedObjectKind::UnmeasuredLayout
        ),
        "unknown layout retains its declared table as opaque"
    );
    let tail = truncated.rows().next().ok_or("partial row")?;
    assert!(
        matches!(
            tail.kind(),
            IncludedObjectKind::Truncated { kind: Some(16) }
        ),
        "readable kind survives an incomplete record"
    );
    assert_eq!(
        tail.bytes(),
        &[16, 0, 0, 0, 0, 0],
        "original truncated payload"
    );
    Ok(())
}

#[test]
fn unreadable_chunks_keep_their_original_position_and_do_not_stop_iteration() -> TestResult {
    let measured = TestSoc::new().layout(8).model("x.cga").chunk();
    let bytes = TestChunkFile::crch()
        .big_endian_chunk(INCLUDED_OBJECTS, 1, measured.clone())
        .chunk(INCLUDED_OBJECTS, 1, vec![0; 3])
        .chunk(INCLUDED_OBJECTS, 1, measured)
        .bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let mut chunks = file.included_objects_chunks();
    let first = chunks.next().ok_or("big-endian chunk")?;
    let second = chunks.next().ok_or("truncated chunk")?;
    let third = chunks.next().ok_or("readable chunk")?;
    assert_eq!(
        (
            first.chunk_index(),
            second.chunk_index(),
            third.chunk_index()
        ),
        (0, 1, 2),
        "original ordinals survive per-chunk failure"
    );
    assert!(
        matches!(first.tables(), Err(IncludedObjectsError::BigEndian)),
        "unsupported byte order reports its disposition"
    );
    assert!(
        matches!(second.tables(), Err(IncludedObjectsError::Truncated(_))),
        "short header reports its disposition"
    );
    assert_eq!(third.tables()?.layout(), 8, "later chunk remains readable");
    Ok(())
}

const fn range_length(range: Range<u64>) -> u64 {
    range.end - range.start
}

#[test]
fn declared_extent_beyond_the_chunk_preserves_the_readable_prefix() -> TestResult {
    let soc = TestSoc::new()
        .layout(8)
        .model("prefix.cga")
        .object(0, SOC_IDENTITY);
    let mut chunk = soc.chunk();
    let placed_len = 176;
    let count_offset = chunk.len() - placed_len - size_of::<u32>();
    chunk
        .get_mut(count_offset..count_offset + size_of::<u32>())
        .ok_or("object table length field")?
        .copy_from_slice(&200_u32.to_le_bytes());
    let bytes = TestChunkFile::crch()
        .chunk(INCLUDED_OBJECTS, 1, chunk)
        .bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let physical = file
        .included_objects_chunks()
        .next()
        .ok_or("physical chunk")?;
    let tables = physical.tables()?;
    assert_eq!(tables.object_len_declared(), 200, "original length word");
    assert_eq!(
        range_length(tables.object_range()),
        placed_len as u64,
        "readable extent"
    );
    assert!(
        matches!(
            tables.rows().next().ok_or("complete prefix row")?.kind(),
            IncludedObjectKind::Placement(_)
        ),
        "readable row survives an oversized declared extent"
    );
    assert!(
        matches!(
            IncludedObjects::read(&bytes),
            Err(IncludedObjectsError::ObjectTable {
                declared: 200,
                available: 176
            })
        ),
        "preview keeps its rejection behavior"
    );
    Ok(())
}

#[test]
fn the_source_reader_accepts_path_counts_beyond_the_preview_limit() -> TestResult {
    let count = MAX_INCLUDED_MODELS + 1;
    let mut chunk = TestSoc::new().layout(8).chunk();
    chunk
        .get_mut(4..8)
        .ok_or("model count")?
        .copy_from_slice(&u32::try_from(count)?.to_le_bytes());
    chunk.splice(8..8, std::iter::repeat_n(0, count * NAME_LEN));
    let bytes = TestChunkFile::crch()
        .chunk(INCLUDED_OBJECTS, 1, chunk)
        .bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let physical = file
        .included_objects_chunks()
        .next()
        .ok_or("physical chunk")?;
    let tables = physical.tables()?;
    assert_eq!(
        tables.models().len(),
        count,
        "every original model path slot"
    );
    assert_eq!(
        tables.model_range(),
        8..8 + (count * NAME_LEN) as u64,
        "complete positioned model table"
    );
    assert_eq!(
        range_length(tables.material_range()),
        0,
        "empty material table retains position"
    );
    assert_eq!(
        tables.material_range(),
        tables.palette_range(),
        "adjacent empty tables share their byte position"
    );
    assert!(
        matches!(
            IncludedObjects::read(&bytes),
            Err(IncludedObjectsError::TooMany { .. })
        ),
        "preview retains its own quota"
    );
    Ok(())
}
