// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use p4kx_extraction::{CancelToken, Exclusion};
use sha2::{Digest as _, Sha256};

use crate::artifact::StreamObserver;
use crate::checkpoint::Checkpoint;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{
    Artifact, ByteOrder, ChunkLayout, DataType, MediaError, MediaWriter, OffsetBasis, Source,
    SourceRange,
};

const FRAMES: [u8; 4] = [9, 0, 30, 0];

#[derive(Default)]
struct Observer {
    receipts: RefCell<Vec<Artifact>>,
    manifest: RefCell<Vec<u8>>,
}

impl StreamObserver for Observer {
    fn write(&self, relative: &str, bytes: &[u8]) -> io::Result<()> {
        if relative == "artifacts.jsonl" {
            self.manifest.borrow_mut().extend_from_slice(bytes);
        }
        Ok(())
    }

    fn typed_chunk(&self, artifact: &Artifact) -> io::Result<bool> {
        let mut receipts = self.receipts.borrow_mut();
        let created = !receipts.iter().any(|seen| seen.path == artifact.path);
        receipts.push(artifact.clone());
        Ok(created)
    }
}

fn layout() -> ChunkLayout {
    let mut layout = ChunkLayout::new(DataType::U16, ByteOrder::Little, 2, "original frames");
    layout.source_range = Some(SourceRange::new(OffsetBasis::Entry, 16, 20));
    layout
}

fn observed_writer<'a>(
    root: &Path,
    reference: &MediaWriter<'_>,
    checkpoint: Checkpoint<'a>,
    source: &Source,
    observer: &'a Observer,
) -> Result<MediaWriter<'a>, MediaError> {
    let mut writer = MediaWriter::observed(
        root,
        vec![reference.kinds[0].capability.clone()],
        reference.limits(),
        checkpoint,
        true,
        observer,
    )?;
    writer.begin(0, source.clone())?;
    Ok(writer)
}

#[test]
fn observed_chunks_match_file_backed_bytes_receipts_and_deduplication() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, file, checkpoint, folder| {
            let observer = Observer::default();
            let root = folder.join("no-chunk-files");
            let mut writer = observed_writer(&root, file, checkpoint, context.source, &observer)?;
            for _ in 0..2 {
                let expected = file.chunk(layout(), &mut FRAMES.as_slice())?;
                let actual = writer.chunk(layout(), &mut FRAMES.as_slice())?;
                let payload = fs::read(file.root.join(&expected.artifact.path))?;
                assert_eq!(
                    payload, FRAMES,
                    "the file contains exact little-endian frames"
                );
                assert_eq!(actual.artifact.size, u64::try_from(payload.len())?);
                assert_eq!(
                    actual.artifact.digest.hex,
                    crate::hashing::hex(Sha256::digest(&payload).into())
                );
                assert_eq!(
                    serde_json::to_value(actual)?,
                    serde_json::to_value(expected)?
                );
            }
            let manifest = observer.manifest.borrow();
            let receipts = observer
                .receipts
                .borrow()
                .iter()
                .map(serde_json::to_string)
                .collect::<Result<Vec<_>, _>>()?;
            assert!(
                std::str::from_utf8(&manifest)?
                    .lines()
                    .eq(receipts.iter().map(String::as_str)),
                "chunk receipts reach the artifact manifest unchanged"
            );
            assert_eq!(
                serde_json::to_value(writer.kinds[0].counts.converted_outputs)?,
                serde_json::to_value(file.kinds[0].counts.converted_outputs)?,
                "repeated content has the same converted-output accounting"
            );
            assert!(
                !root.exists(),
                "observing a typed chunk creates no payload files"
            );
            Ok(())
        },
    )
}

#[test]
fn cancelled_and_malformed_chunks_have_no_observed_receipt() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, file, checkpoint, folder| {
            let observer = Observer::default();
            let cancel = CancelToken::new();
            let checkpoint = Checkpoint {
                cancel: &cancel,
                ..checkpoint
            };
            let root = folder.join("no-chunk-files");
            let mut writer = observed_writer(&root, file, checkpoint, context.source, &observer)?;
            for mut invalid in [b"\0\0".as_slice(), b"\0\0\0\0\0\0".as_slice()] {
                assert!(matches!(
                    writer.chunk(layout(), &mut invalid),
                    Err(MediaError::Contract(_))
                ));
            }
            writer.limits.chunk_bytes = 2;
            assert!(matches!(
                writer.chunk(layout(), &mut FRAMES.as_slice()),
                Err(MediaError::Limit {
                    budget: "chunk_bytes",
                    ..
                })
            ));
            writer.limits.chunk_bytes = file.limits().chunk_bytes;
            let mut input = CancellingReader {
                cancel: &cancel,
                bytes: &FRAMES,
            };
            assert!(matches!(
                writer.chunk(layout(), &mut input),
                Err(MediaError::Cancelled)
            ));
            assert!(observer.receipts.borrow().is_empty());
            assert!(observer.manifest.borrow().is_empty());
            assert_eq!(writer.kinds[0].counts.converted_outputs.enumerated, 0);
            assert!(!root.exists());
            Ok(())
        },
    )
}

struct CancellingReader<'a> {
    cancel: &'a CancelToken,
    bytes: &'static [u8],
}

impl Read for CancellingReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self.bytes.read(output)?;
        self.cancel.cancel();
        Ok(count)
    }
}
