// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_model::{AnimationError, CorruptAnimation, DbaAnimationSources, DbaMetadataRecords};
use p4kx_testkit::{TestAnimation, TestChunkFile, TestDba};

type TestResult = Result<(), Box<dyn Error>>;

const METADATA: u32 = 0xF735_1608;
const DATA: u32 = 0x194F_BC50;
const RECORD_BYTES: usize = 48;

#[test]
fn metadata_keeps_all_physical_names_when_the_first_controller_block_is_corrupt() -> TestResult {
    let database = TestDba::new()
        .animation(TestAnimation::new("same.caf", 15))
        .animation(TestAnimation::new("same.caf", 60))
        .animation(TestAnimation::new("other.caf", 30))
        .bytes();
    let original = ChunkFile::parse(database.as_slice())?;
    let (chunk_index, metadata) = original
        .chunks_of(ivo::DBA_METADATA)
        .next()
        .ok_or("metadata")?;
    let mut metadata_bytes = original
        .chunk_data(chunk_index)
        .ok_or("metadata bytes")?
        .to_vec();
    let invalid = 8 + 3 * RECORD_BYTES + 2 * b"same.caf\0".len();
    *metadata_bytes.get_mut(invalid).ok_or("third raw name")? = 0xff;
    let bytes = TestChunkFile::ivo()
        .chunk(METADATA, metadata.version(), metadata_bytes.clone())
        .chunk(DATA, 0x900, b"\0\0\0\0bad block".to_vec())
        .bytes();
    let file = ChunkFile::parse(bytes.as_slice())?;
    let mut clips = DbaAnimationSources::read("broken.dba", &file)?;
    assert!(
        matches!(
            clips.next(),
            Some(Err(AnimationError::Corrupt {
                chunk: "DbaData",
                ..
            }))
        ),
        "broken data block is still diagnosed"
    );
    assert!(
        clips.next().is_none(),
        "the source cursor does not guess a new block boundary"
    );
    verify_metadata_rows(&file, &metadata_bytes)
}

fn verify_metadata_rows(file: &ChunkFile<&[u8]>, metadata_bytes: &[u8]) -> TestResult {
    let (chunk_index, chunk) = file
        .chunks_of(ivo::DBA_METADATA)
        .next()
        .ok_or("metadata chunk")?;
    let rows: Vec<_> = DbaMetadataRecords::read("broken.dba", file)?.collect::<Result<_, _>>()?;
    let expected: [(&[u8], u16); 3] = [(b"same.caf", 15), (b"same.caf", 60), (b"\xffther.caf", 30)];
    assert_eq!(rows.len(), expected.len(), "metadata remains complete");
    let mut name_at = chunk.offset() + (8 + expected.len() * RECORD_BYTES) as u64;
    for (ordinal, (row, (name, fps))) in rows.iter().zip(expected).enumerate() {
        assert_eq!(row.ordinal, ordinal, "physical metadata ordinal");
        assert_eq!(row.name.bytes(), name, "exact name bytes at {ordinal}");
        let next = name_at + name.len() as u64 + 1;
        assert_eq!(
            row.name.range(),
            name_at..next,
            "name includes its NUL at {ordinal}"
        );
        name_at = next;
        assert_eq!(
            row.metadata.chunk_index(),
            chunk_index,
            "original chunk index"
        );
        let first = 8 + ordinal * RECORD_BYTES;
        let end = first + RECORD_BYTES;
        assert_eq!(
            row.metadata.range(),
            chunk.offset() + first as u64..chunk.offset() + end as u64,
            "exact record range"
        );
        assert_eq!(
            Some(row.metadata.bytes().as_slice()),
            metadata_bytes.get(first..end),
            "exact record bytes"
        );
        assert_eq!(
            row.metadata.timing()?.frames_per_second,
            fps,
            "same-name clips retain separate timing"
        );
    }
    Ok(())
}

#[test]
fn an_unterminated_name_keeps_the_proven_prefix_in_a_metadata_only_database() -> TestResult {
    let mut metadata = vec![0; 8 + 2 * RECORD_BYTES];
    *metadata.first_mut().ok_or("record count")? = 2;
    metadata.extend_from_slice(b"first.caf\0unterminated");
    let bytes = TestChunkFile::ivo()
        .chunk(METADATA, 0x902, metadata)
        .bytes();
    let file = ChunkFile::parse(bytes.as_slice())?;
    let mut rows = DbaMetadataRecords::read("metadata-only.dba", &file)?;
    let first = rows.next().ok_or("first metadata record")??;
    assert_eq!(first.ordinal, 0, "first physical record remains reachable");
    assert_eq!(first.name.bytes(), b"first.caf", "terminated prefix name");
    assert!(
        matches!(
            rows.next(),
            Some(Err(AnimationError::Corrupt {
                chunk: "DbaMetadata",
                source: CorruptAnimation::Names { animations: 2 },
                ..
            }))
        ),
        "unterminated tail is diagnosed"
    );
    assert!(
        rows.next().is_none(),
        "the unread name is never reported as a record"
    );
    assert!(
        matches!(
            DbaAnimationSources::read("metadata-only.dba", &file),
            Err(AnimationError::MissingChunk {
                chunk: "DbaData",
                ..
            })
        ),
        "the controller inventory still requires a data chunk"
    );
    Ok(())
}
