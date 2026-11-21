// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process;

use p4kx_dataforge::query::{CompiledQuery, QueryRun, ResultFormat, ResultWriter, Row};
use thiserror::Error;

use crate::CancelToken;

/// The number of rows built and written at a time.
pub const QUERY_PAGE_SIZE: NonZeroUsize = match NonZeroUsize::new(1024) {
    Some(size) => size,
    None => NonZeroUsize::MIN,
};

const WRITE_BUFFER_LEN: usize = 256 * 1024;

/// How [`export_query`] is controlled while it runs.
#[derive(Clone, Copy, Default)]
pub struct QueryExportOptions<'a> {
    /// Checked before every page; once cancelled, the export stops and writes nothing.
    pub cancel: Option<&'a CancelToken>,
    /// Called after every page with the number of rows written so far.
    pub progress: Option<&'a (dyn Fn(u64) + Sync)>,
    /// Called with each row whose Record's values cannot be read; it gives no row.
    pub failed: Option<&'a (dyn Fn(&Row<'_>) + Sync)>,
}

impl std::fmt::Debug for QueryExportOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryExportOptions")
            .field("cancel", &self.cancel)
            .field("progress", &self.progress.is_some())
            .field("failed", &self.failed.is_some())
            .finish()
    }
}

/// What [`export_query`] wrote.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QueryExport {
    /// The rows written, one per Record.
    pub records: u64,
    /// The Records whose layout or instance cannot be read; they have no row.
    pub failed: u64,
    /// The null or dangling links `->` met in the rows written, each counted once per row.
    pub unresolved_references: u64,
}

/// A Query result cannot be exported.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum QueryExportError {
    /// Ownership capture or verified cleanup failed without losing its typed cause.
    #[error(transparent)]
    OwnedOutput(Box<p4kx_guard::OwnedOutputError>),
    /// An unpublished output could not be removed after completion or failure.
    #[error("owned output cleanup failed: {cleanup}")]
    CleanupFailed {
        /// The original failure when one preceded cleanup.
        #[source]
        primary: Option<Box<dyn std::error::Error + Send + Sync>>,
        /// The separate cleanup failure.
        cleanup: Box<p4kx_guard::OwnedOutputCleanupError>,
    },
    /// The export was cancelled; the target is unchanged.
    #[error("the export was cancelled")]
    Cancelled,
    /// The target is the Archive the Query reads; nothing is written.
    #[error("{} is the Archive the Query reads", .0.display())]
    OutputIsArchive(PathBuf),
    /// The file cannot be written; the target is unchanged.
    #[error("cannot write {}", path.display())]
    Write {
        /// The target file.
        path: PathBuf,
        /// The failed write.
        #[source]
        source: io::Error,
    },
    /// Source or target protection refused the write.
    #[error(transparent)]
    Installation(p4kx_guard::WriteProtectionError),
}

/// Writes the result of `run`, selected by `query` over the Archive at `archive`, to the file
/// `out` in `format`, page by page, as `p4kx dcb query --out` does (`docs/query.md`).
///
/// The rows go to `<file>.p4kx-<process id>.part` next to `out`, which replaces `out` once
/// complete, so `out` is never half written. A Record whose values cannot be read gives no row
/// and is counted as failed.
///
/// # Errors
///
/// Returns [`QueryExportError::OutputIsArchive`] when `out` is the file at `archive`,
/// [`QueryExportError::Cancelled`] once `options.cancel` is set and [`QueryExportError::Write`]
/// when the file cannot be written; the temporary file is removed in every case. A target inside
/// a Star Citizen installation gives [`QueryExportError::Installation`].
pub fn export_query(
    query: &CompiledQuery<'_>,
    run: &QueryRun<'_, '_>,
    (out, format): (&Path, ResultFormat),
    archive: &Path,
    options: &QueryExportOptions<'_>,
) -> Result<QueryExport, QueryExportError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(QueryExportError::Cancelled);
    }
    if is_same_file(out, archive) {
        return Err(QueryExportError::OutputIsArchive(out.to_path_buf()));
    }
    let protection = crate::ArchiveWriteProtection::new(
        archive,
        crate::output_file::cancel_flag(options.cancel),
    )?;
    crate::check_protected_output(out, Some(&protection), options.cancel)?;
    let write_error = |source| QueryExportError::Write {
        path: out.to_path_buf(),
        source,
    };
    let temporary = temporary_path(out).map_err(write_error)?;
    crate::check_protected_output(&temporary, Some(&protection), options.cancel)?;
    let (file, ownership) = crate::create_output_file(
        &temporary,
        Some(&protection),
        crate::output_file::cancel_flag(options.cancel),
    )
    .map_err(|error| match error {
        crate::OwnedOutputError::Io { source, .. } => write_error(source),
        other => other.into(),
    })?
    .into_parts();
    let result = write_rows(query, run, file, format, options).and_then(|(file, export)| {
        file.sync_all()?;
        ownership
            .ensure_writable(&temporary, crate::output_file::cancel_flag(options.cancel))
            .map_err(Stop::Protection)?;
        protection
            .ensure_writable(out, crate::output_file::cancel_flag(options.cancel))
            .map_err(Stop::Protection)?;
        fs::rename(&temporary, out)?;
        Ok(export)
    });
    let result = result.map_err(|stop| match stop {
        Stop::Cancelled => QueryExportError::Cancelled,
        Stop::Io(source) => write_error(source),
        Stop::Protection(error) if error.is_cancelled() => QueryExportError::Cancelled,
        Stop::Protection(error) => QueryExportError::Installation(error),
    });
    if result.is_err() {
        crate::output_cleanup::query(result, &ownership, &temporary, options.cancel)
    } else {
        result
    }
}

/// Why writing the rows stopped early.
enum Stop {
    Protection(p4kx_guard::WriteProtectionError),
    Cancelled,
    Io(io::Error),
}

impl From<io::Error> for Stop {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn write_rows(
    query: &CompiledQuery<'_>,
    run: &QueryRun<'_, '_>,
    file: File,
    format: ResultFormat,
    options: &QueryExportOptions<'_>,
) -> Result<(File, QueryExport), Stop> {
    let cancelled = || options.cancel.is_some_and(CancelToken::is_cancelled);
    let buffered = BufWriter::with_capacity(WRITE_BUFFER_LEN, file);
    let mut writer = ResultWriter::new(buffered, format, query.columns())?;
    let mut export = QueryExport::default();
    for page in run.pages(QUERY_PAGE_SIZE) {
        if cancelled() {
            return Err(Stop::Cancelled);
        }
        for row in &page {
            if let Ok(cells) = row.cells() {
                writer.write_row(row.record_path(), cells)?;
                export.records = export.records.saturating_add(1);
                export.unresolved_references = export
                    .unresolved_references
                    .saturating_add(u64::try_from(row.unresolved_references()).unwrap_or(u64::MAX));
            } else {
                export.failed = export.failed.saturating_add(1);
                if let Some(failed) = options.failed {
                    failed(row);
                }
            }
        }
        if let Some(progress) = options.progress {
            progress(export.records);
        }
    }
    if cancelled() {
        return Err(Stop::Cancelled);
    }
    let file = writer
        .finish()?
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?;
    Ok((file, export))
}

/// Returns `<name>.p4kx-<process id>.part` next to `out`.
fn temporary_path(out: &Path) -> io::Result<PathBuf> {
    let name = out
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the output names no file"))?;
    let mut name = name.to_os_string();
    name.push(format!(".p4kx-{}.part", process::id()));
    Ok(out.with_file_name(name))
}

/// Returns whether `path` names an existing file that is the file at `other`, after resolving
/// links and relative parts.
#[must_use]
pub fn is_same_file(path: &Path, other: &Path) -> bool {
    match (fs::canonicalize(path), fs::canonicalize(other)) {
        (Ok(path), Ok(other)) => path == other,
        _ => false,
    }
}
