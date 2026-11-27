// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::OriginLines;
use crate::checkpoint::{Checkpoint, IO_BLOCK_BYTES};
use crate::{Algorithm, Artifact, ByteScope, Digest, MediaError, hashing, origin_inventory_wire};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn buffered_and_spanning_rows_share_the_same_digest_and_row_policy() -> TestResult {
    let large = "é".repeat(IO_BLOCK_BYTES);
    let bytes = format!("{}\n{}\n{}\n", json!(1), json!(large), json!(2));
    with_origin_lines(bytes.as_bytes(), bytes.len(), |mut lines| {
        let seed = origin_inventory_wire::seed::<Value>;
        assert_eq!(lines.next(seed())?, Some(json!(1)));
        assert_eq!(lines.next(seed())?, Some(json!(large)));
        assert_eq!(lines.next(seed())?, Some(json!(2)));
        assert_eq!(lines.next(seed())?, None);
        lines.finish()?;
        Ok(())
    })?;
    with_origin_lines(bytes.as_bytes(), IO_BLOCK_BYTES, |mut lines| {
        let seed = origin_inventory_wire::seed::<Value>;
        assert_eq!(lines.next(seed())?, Some(json!(1)));
        assert!(
            matches!(lines.next(seed()), Err(MediaError::Destination(_))),
            "a spanning row observes the same policy"
        );
        Ok(())
    })
}

#[test]
fn origin_rows_require_a_single_value_and_a_final_newline() -> TestResult {
    for bytes in [b"1 2\n".as_slice(), b"1".as_slice(), b"\n".as_slice()] {
        with_origin_lines(bytes, bytes.len(), |mut lines| {
            assert!(
                lines.next(origin_inventory_wire::seed::<Value>()).is_err(),
                "a physical line supplies exactly one JSON value and its terminator"
            );
            Ok(())
        })?;
    }
    with_origin_lines(b"1\n", 1, |mut lines| {
        assert!(
            matches!(
                lines.next(origin_inventory_wire::seed::<Value>()),
                Err(MediaError::Destination(_))
            ),
            "the buffered byte check includes the newline"
        );
        Ok(())
    })
}

fn with_origin_lines(
    bytes: &[u8],
    maximum: usize,
    check: impl FnOnce(OriginLines<'_, '_>) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("fixture.bin", b"fixture"))
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let watch = ArchiveWatch::new(&archive);
    let cancel = CancelToken::default();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &cancel,
        watch: &watch,
        protection: &protection,
    };
    let receipt = Artifact {
        path: "test.jsonl".to_owned(),
        size: u64::try_from(bytes.len())?,
        digest: Digest {
            algorithm: Algorithm::Sha256,
            scope: ByteScope::ArtifactFile,
            hex: hashing::hex(Sha256::digest(bytes).into()),
        },
    };
    fs::write(scratch.path().join(&receipt.path), bytes)?;
    check(OriginLines::open(
        scratch.path(),
        &receipt,
        checkpoint,
        u64::try_from(maximum)?,
    )?)
}
