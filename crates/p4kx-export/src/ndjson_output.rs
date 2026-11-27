// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_extraction::{
    CancelToken, ConflictPolicy, ExtractionError, OriginDigestWriter, OwnedFileToken,
    OwnedOutputError, RecordExportOptions, RecordStreamCommit, RecordStreamInput,
    RecordStreamReceipt, RecordStreamRows, RecordStreamSpool, create_output_file,
};
use tempfile::TempPath;

const WRITE_BUFFER_LEN: usize = 64 << 10;

/// The active descriptor and creation proof for one streamed Record file.
pub(crate) struct Output<'input> {
    file: Option<BufWriter<File>>,
    temporary: Option<TempPath>,
    ownership: OwnedFileToken,
    pub(crate) len: u64,
    pub(crate) crc: u32,
    observed: Option<Observed<'input>>,
}

impl<'input> Output<'input> {
    pub(crate) fn create(
        target: &Path,
        options: &RecordExportOptions<'_>,
        original: Option<RecordStreamInput<'input, '_>>,
    ) -> Result<Self, ExtractionError> {
        let observed = original
            .map(Observed::new)
            .transpose()
            .map_err(|source| write_error(target, source))?;
        let file = tempfile::Builder::new()
            .prefix(".p4kx-")
            .suffix(".part")
            .disable_cleanup(true)
            .make_in(target, |path| {
                create_output_file(path, options.protection, cancel_flag(options.cancel))
                    .map_err(OwnedOutputError::into_io_error)
            })
            .map_err(|source| {
                if let Some(OwnedOutputError::Protection(error)) =
                    p4kx_extraction::owned_output_failure(&source)
                {
                    return ExtractionError::Installation(error.clone());
                }
                write_error(target, source)
            })?;
        let (created, temporary) = file.into_parts();
        let (file, ownership) = created.into_parts();
        Ok(Self {
            file: Some(BufWriter::with_capacity(WRITE_BUFFER_LEN, file)),
            temporary: Some(temporary),
            ownership,
            len: 0,
            crc: 0,
            observed,
        })
    }

    pub(crate) fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("Record output is closed"))?
            .write_all(bytes)?;
        if let Some(observed) = &mut self.observed {
            observed.digest.write_all(bytes)?;
        }
        self.len = self.len.saturating_add(bytes.len() as u64);
        self.crc = crc32c::crc32c_append(self.crc, bytes);
        Ok(())
    }

    pub(crate) fn append_part(
        &mut self,
        bytes: &[u8],
        rows: Option<Vec<RecordStreamRows<'input, '_>>>,
    ) -> io::Result<()> {
        let part_start = self.len;
        self.append(bytes)?;
        match (&mut self.observed, rows) {
            (Some(observed), Some(rows)) => observed.spool.append_part(rows, part_start),
            (None, None) => Ok(()),
            _ => Err(io::Error::other(
                "Record stream output and origin mode differ",
            )),
        }
    }

    pub(crate) fn persist_record_stream(
        &mut self,
        destination: &Path,
        replace: bool,
        options: &RecordExportOptions<'_>,
    ) -> Result<Option<RecordStreamCommit>, ExtractionError> {
        self.sync(destination)?;
        let path = self.temporary.as_ref().ok_or_else(|| {
            write_error(destination, io::Error::other("Record temporary is closed"))
        })?;
        let proof = self
            .observed
            .take()
            .map(Observed::finish)
            .transpose()
            .map_err(|source| write_error(destination, source))?;
        let options = RecordExportOptions {
            policy: if replace {
                ConflictPolicy::Overwrite
            } else {
                ConflictPolicy::Skip
            },
            ..*options
        };
        let mut committed = p4kx_extraction::publish_record_stream(
            path,
            destination,
            &self.ownership,
            &options,
            proof,
        )?;
        if let Some(commit) = &mut committed {
            if !replace
                && let Err(cleanup) = self.ownership.remove(path, cancel_flag(options.cancel))
            {
                commit.origin_failure = Some(ExtractionError::CleanupFailed {
                    primary: commit
                        .origin_failure
                        .take()
                        .map(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>),
                    cleanup: Box::new(cleanup),
                });
            }
            self.temporary = None;
        }
        Ok(committed)
    }

    fn sync(&mut self, destination: &Path) -> Result<(), ExtractionError> {
        if let Some(writer) = self.file.take() {
            writer
                .into_inner()
                .map_err(|error| write_error(destination, error.into_error()))?
                .sync_all()
                .map_err(|error| write_error(destination, error))?;
        }
        Ok(())
    }

    pub(crate) fn complete<T>(
        mut self,
        result: Result<T, ExtractionError>,
        cancel: Option<&CancelToken>,
    ) -> Result<T, ExtractionError> {
        self.file = None;
        let Some(temporary) = self.temporary else {
            return result;
        };
        match self.ownership.remove(&temporary, cancel_flag(cancel)) {
            Ok(()) => result,
            Err(cleanup) => Err(ExtractionError::CleanupFailed {
                primary: result
                    .err()
                    .map(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>),
                cleanup: Box::new(cleanup),
            }),
        }
    }
}

struct Observed<'input> {
    digest: OriginDigestWriter<io::Sink>,
    spool: RecordStreamSpool<'input>,
}

impl<'input> Observed<'input> {
    fn new(input: RecordStreamInput<'input, '_>) -> io::Result<Self> {
        let spool = RecordStreamSpool::new(input)?;
        Ok(Self {
            digest: OriginDigestWriter::new(io::sink()),
            spool,
        })
    }

    fn finish(self) -> io::Result<RecordStreamReceipt> {
        Ok(RecordStreamReceipt::new(
            self.digest.finish()?,
            self.spool.finish()?,
        ))
    }
}

fn cancel_flag(cancel: Option<&CancelToken>) -> &std::sync::atomic::AtomicBool {
    static NEVER_CANCELLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    cancel.map_or(&NEVER_CANCELLED, CancelToken::flag)
}

fn write_error(path: &Path, source: io::Error) -> ExtractionError {
    ExtractionError::Write {
        path: path.to_path_buf(),
        source,
    }
}
