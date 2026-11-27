// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_assembly::{BodyVariant, SocCaptureError};
use p4kx_extraction::{
    Archive, ArchiveWatch, CancelToken, DecimalU64, OriginDigestWriter, PhysicalSourceRef,
};
use p4kx_guard::ArchiveWriteProtection;

use crate::player_bundle_output::Output;
use crate::player_bundle_wire::{self as wire, Artifact, Manifest, Row, Stream, Totals};
use crate::soc_scene::SocSceneError;
use crate::stoppable::{self, Stoppable};
use crate::{
    PackageError, PackageResult, PackageWriteContext, PlayerDocument, PlayerDocumentSink,
    PlayerSourceCapture, PlayerSourceError, RejectedPlayerDocument,
};

type Writer = OriginDigestWriter<BufWriter<File>>;

/// Writes a shared Player source bundle to a new directory.
///
/// The final manifest publishes the records and verified original XML files.
///
/// # Errors
/// Returns source, destination, protection, cancellation or output failures.
pub fn write_player_source_bundle(
    archive: &Archive,
    bodies: &[BodyVariant],
    destination: &Path,
    cancel: &CancelToken,
) -> Result<(), PackageError> {
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context = PackageWriteContext::new(&protection, cancel.flag());
    let mut output = Output::create(destination, context)?;
    let result = assemble(archive, bodies, cancel, &mut output);
    output.finish(result)
}

fn assemble(
    archive: &Archive,
    bodies: &[BodyVariant],
    cancel: &CancelToken,
    output: &mut Output<'_>,
) -> PackageResult<()> {
    let watch = ArchiveWatch::new(archive);
    let records = output.path(wire::RECORDS);
    let writer = output.file(wire::RECORDS)?;
    let context = output.write_context();
    let mut sink = Capture {
        output,
        rows: Rows {
            writer,
            context,
            count: 0,
        },
        artifacts: BTreeMap::new(),
        bytes: 0,
    };
    sink.rows
        .push(&Row::Header {
            format: wire::FORMAT,
            schema_version: wire::VERSION,
        })
        .map_err(|e| write_error(&records, e))?;
    let capture = PlayerSourceCapture::read(archive, bodies, cancel, &mut sink)
        .map_err(|e| capture_error(&records, e))?;
    let (writer, row_count, raw_artifacts) = sink
        .finish(&capture)
        .map_err(|e| write_error(&records, e))?;
    let digest = output.complete_file(&records, writer)?;
    watch
        .check()
        .map_err(|e| PackageError::write(&records, io::Error::other(e)))?;
    output.publish(&Manifest {
        format: wire::FORMAT,
        schema_version: wire::VERSION,
        bodies: &bodies.iter().map(|body| body.code()).collect::<Vec<_>>(),
        records: Stream {
            path: wire::RECORDS,
            digest,
            row_count: DecimalU64::new(row_count),
        },
        raw_artifacts,
    })
}

struct Rows<'context> {
    writer: Writer,
    context: PackageWriteContext<'context>,
    count: u64,
}

impl Rows<'_> {
    fn push(&mut self, row: &Row<'_>) -> io::Result<()> {
        let mut writer = Stoppable::new(&mut self.writer, &self.context);
        serde_json::to_writer(&mut writer, row)?;
        writer.write_all(b"\n")?;
        self.count = checked_add(self.count, 1)?;
        Ok(())
    }
}

struct Capture<'writer, 'context> {
    output: &'writer mut Output<'context>,
    rows: Rows<'context>,
    artifacts: BTreeMap<PhysicalSourceRef, Artifact>,
    bytes: u64,
}

impl Capture<'_, '_> {
    fn retain(&mut self, source: &PhysicalSourceRef, bytes: &[u8]) -> io::Result<()> {
        let path = format!("{}/{}.bin", wire::ORIGINALS, self.artifacts.len());
        let std::collections::btree_map::Entry::Vacant(entry) =
            self.artifacts.entry(source.clone())
        else {
            return Ok(());
        };
        let mut file = self.output.file(&path).map_err(io::Error::other)?;
        let mut crc = 0;
        for block in bytes.chunks(256 * 1024) {
            self.rows.context.check().map_err(io::Error::other)?;
            file.write_all(block)?;
            crc = crc32c::crc32c_append(crc, block);
        }
        let digest = self
            .output
            .complete_file(&self.output.path(&path), file)
            .map_err(io::Error::other)?;
        self.bytes = checked_add(self.bytes, digest.byte_length())?;
        entry.insert(Artifact {
            path,
            digest,
            crc32c: format!("{crc:08x}"),
        });
        Ok(())
    }

    fn finish(mut self, capture: &PlayerSourceCapture) -> io::Result<(Writer, u64, Totals)> {
        super::player_bundle_rows::write(capture, |row| self.rows.push(row))?;
        let totals = Totals {
            count: DecimalU64::new(self.artifacts.len() as u64),
            byte_length: DecimalU64::new(self.bytes),
        };
        self.rows.push(&Row::End {
            rows: DecimalU64::new(self.rows.count),
            raw_artifacts: &totals,
        })?;
        Ok((self.rows.writer, self.rows.count, totals))
    }
}

impl PlayerDocumentSink for Capture<'_, '_> {
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()> {
        self.retain(&document.source, document.bytes)?;
        let artifact = self
            .artifacts
            .get(&document.source)
            .ok_or_else(|| io::Error::other("missing original artifact"))?;
        self.rows.push(&Row::Document {
            index: document.index,
            source: &document.source,
            origin: &document.origin,
            format: document.format,
            lossy: document.lossy,
            artifact,
        })
    }

    fn rejected_document(&mut self, document: RejectedPlayerDocument<'_>) -> io::Result<()> {
        self.retain(&document.source, document.bytes)?;
        let artifact = self
            .artifacts
            .get(&document.source)
            .ok_or_else(|| io::Error::other("missing rejected original artifact"))?;
        self.rows.push(&Row::DocumentRejection {
            source: &document.source,
            origin: &document.origin,
            reason: document.rejection,
            artifact,
        })
    }
}

fn capture_error(path: &Path, error: PlayerSourceError) -> PackageError {
    match error {
        PlayerSourceError::Cancelled
        | PlayerSourceError::Capture(SocCaptureError::Cancelled)
        | PlayerSourceError::Xml(
            SocSceneError::Cancelled | SocSceneError::Capture(SocCaptureError::Cancelled),
        ) => PackageError::Cancelled,
        PlayerSourceError::Xml(SocSceneError::DocumentSink(ref cause))
            if stoppable::is_cancelled(cause) =>
        {
            PackageError::Cancelled
        }
        _ => PackageError::write(path, io::Error::other(error)),
    }
}

fn write_error(path: &Path, error: io::Error) -> PackageError {
    if stoppable::is_cancelled(&error) {
        PackageError::Cancelled
    } else {
        PackageError::write(path, error)
    }
}

fn checked_add(left: u64, right: u64) -> io::Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| io::Error::other("Player bundle counter exceeds u64"))
}

#[cfg(test)]
#[path = "player_bundle_cancellation_tests.rs"]
mod cancellation_tests;
