// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use crate::file_receipt::{
    OrdinaryReplacement, StagedReceiptPublication, companion_path, finish_staged_receipt,
};
use crate::output_file::StagedOutput;
use crate::portable_name::check_path_length;
use crate::record_export::replace_record;
use crate::write::write_error;
use crate::{
    ExtractionError, FileReceipt, OriginArtifactDigest, OwnedFileToken, RecordExportOptions,
    StagedOriginCompanion,
};

/// Observed provenance of one measured Record stream.
pub struct RecordStreamReceipt {
    digest: OriginArtifactDigest,
    companion: StagedOriginCompanion,
}

impl RecordStreamReceipt {
    /// Pairs a finished companion with the exact measured output bytes.
    #[must_use]
    pub const fn new(digest: OriginArtifactDigest, companion: StagedOriginCompanion) -> Self {
        Self { digest, companion }
    }
}

/// A published Record stream and the separate outcome of its companion.
#[non_exhaustive]
pub struct RecordStreamCommit {
    /// Path of the published companion when available.
    pub companion: Option<PathBuf>,
    /// Failure to publish the companion or retire its lock after commit.
    pub origin_failure: Option<ExtractionError>,
}

/// Publishes an owned staged Record stream and its receipt after the caller's content check.
///
/// An unobserved replacement retires the existing owned receipt. Returns `None` for a kept file.
/// The caller removes any remaining temporary through its ownership token.
///
/// # Errors
/// Returns a precommit protection, write, cancellation or ownership error.
pub fn publish_record_stream(
    temporary: &Path,
    output: &Path,
    owner: &OwnedFileToken,
    options: &RecordExportOptions<'_>,
    origin: Option<RecordStreamReceipt>,
) -> Result<Option<RecordStreamCommit>, ExtractionError> {
    let staged = StagedOutput::new(temporary, output, owner, options.protection, options.cancel);
    publish_staged(&staged, options, origin)
}

pub(crate) fn publish_staged(
    staged: &StagedOutput<'_>,
    options: &RecordExportOptions<'_>,
    origin: Option<RecordStreamReceipt>,
) -> Result<Option<RecordStreamCommit>, ExtractionError> {
    staged.check()?;
    let publication = Publication { staged, options };
    match origin.map(|origin| publication.companion().map(|path| (path, origin))) {
        Some(Ok((path, origin))) => publication.publish(origin, path),
        partial => publication.replace(partial.and_then(Result::err)),
    }
}

struct Publication<'a, 'target, 'job> {
    staged: &'a StagedOutput<'target>,
    options: &'a RecordExportOptions<'job>,
}

impl Publication<'_, '_, '_> {
    fn companion(&self) -> Result<PathBuf, ExtractionError> {
        let path =
            companion_path(self.staged.output()).map_err(|source| self.write_error(source))?;
        check_path_length(&path)?;
        Ok(path)
    }

    fn publish(
        &self,
        mut origin: RecordStreamReceipt,
        path: PathBuf,
    ) -> Result<Option<RecordStreamCommit>, ExtractionError> {
        let receipt = FileReceipt::for_native_staged_replacement(
            self.staged.output(),
            self.options.protection,
            self.options.cancel.cloned().unwrap_or_default(),
        )
        .map_err(|source| self.write_error(source))?;
        let publication = receipt.publish_staged_with_policy(
            self.staged,
            origin.digest,
            self.options.policy,
            || Ok(()),
            || {},
            |out, binding| origin.companion.write(out, binding),
        );
        let (receipt, retirement) = match publication {
            None => return Ok(None),
            Some(StagedReceiptPublication::Uncommitted(error)) => {
                return Err(self.write_error(error));
            }
            Some(StagedReceiptPublication::Committed {
                receipt,
                retirement,
            }) => (receipt, retirement),
        };
        let written = receipt.as_ref().is_some_and(Result::is_ok);
        let foreign = receipt.is_none().then(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the companion path contains foreign data",
            )
        });
        let failure = finish_staged_receipt(receipt, retirement)
            .err()
            .or(foreign)
            .map(|error| write_error(path.clone(), error));
        Ok(Some(RecordStreamCommit {
            companion: written.then_some(path),
            origin_failure: failure,
        }))
    }

    fn replace(
        &self,
        origin_failure: Option<ExtractionError>,
    ) -> Result<Option<RecordStreamCommit>, ExtractionError> {
        let held = OrdinaryReplacement::new(
            self.staged.output(),
            self.options.protection,
            self.options.cancel,
        )
        .map_err(|source| self.write_error(source))?;
        let replaced = replace_record(&held, self.staged, self.options.policy);
        let committed = matches!(replaced, Ok(true));
        let mut result = RecordStreamCommit {
            companion: None,
            origin_failure,
        };
        match held.finish_io(replaced) {
            Ok(false) => Ok(None),
            Ok(true) => Ok(Some(result)),
            Err(error) if committed => {
                result.origin_failure = Some(self.write_error(error));
                Ok(Some(result))
            }
            Err(error) => Err(self.write_error(error)),
        }
    }

    fn write_error(&self, source: io::Error) -> ExtractionError {
        if let Some(protection) = source
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<crate::WriteProtectionError>())
        {
            return protection.clone().into();
        }
        if source.kind() == io::ErrorKind::Interrupted
            && let Err(protection) = self.staged.check()
            && protection.is_cancelled()
        {
            return protection.into();
        }
        write_error(self.staged.output().to_path_buf(), source)
    }
}
