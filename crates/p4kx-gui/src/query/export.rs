// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use p4kx_extraction::query::{Query, ResultFormat};
use p4kx_extraction::{
    CancelToken, DataForgeEntry, QueryExport, QueryExportError, QueryExportOptions, export_query,
};

use crate::model::describe;

/// A file format a Query result is exported in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryExportFormat {
    /// CSV after RFC 4180, as `p4kx dcb query --format csv` writes it.
    Csv,
    /// One JSON array of objects, as `p4kx dcb query --format json` writes it.
    Json,
}

impl QueryExportFormat {
    /// Returns the file extension, without the dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }

    const fn service(self) -> ResultFormat {
        match self {
            Self::Csv => ResultFormat::Csv,
            Self::Json => ResultFormat::Json,
        }
    }
}

/// Where exporting the result stands.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QueryExportStatus {
    /// The result is being written to the path.
    Exporting(PathBuf),
    /// The result was written to the path: so many Records, so many failed.
    Exported {
        /// The file written.
        path: PathBuf,
        /// The rows written.
        records: u64,
        /// The Records that gave no row.
        failed: u64,
    },
    /// The result could not be written to the path, for the reason given.
    Failed(PathBuf, String),
    /// The export to the path was cancelled; the file was not created or replaced.
    Cancelled(PathBuf),
}

/// Exporting a result on a background thread, or its outcome.
#[derive(Debug)]
pub(crate) struct ExportJob {
    status: QueryExportStatus,
    cancel: CancelToken,
    result: Option<Receiver<Result<QueryExport, Stopped>>>,
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl ExportJob {
    /// Returns where exporting stands.
    pub(crate) const fn status(&self) -> &QueryExportStatus {
        &self.status
    }

    /// Asks the export to stop at its next page.
    pub(crate) fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Returns whether the export still runs.
    pub(crate) const fn is_running(&self) -> bool {
        self.result.is_some()
    }

    /// Takes the outcome once exporting ended; returns whether it ended.
    pub(crate) fn poll(&mut self) -> bool {
        let Some(result) = &self.result else {
            return false;
        };
        let outcome = match result.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err(Stopped::Failed(String::new())),
        };
        self.result = None;
        let path = path_of(&self.status).to_path_buf();
        self.status = match outcome {
            Ok(export) => QueryExportStatus::Exported {
                path,
                records: export.records,
                failed: export.failed,
            },
            Err(Stopped::Failed(reason)) => QueryExportStatus::Failed(path, reason),
            Err(Stopped::Cancelled) => QueryExportStatus::Cancelled(path),
        };
        true
    }
}

fn path_of(status: &QueryExportStatus) -> &Path {
    match status {
        QueryExportStatus::Exporting(path)
        | QueryExportStatus::Exported { path, .. }
        | QueryExportStatus::Failed(path, _)
        | QueryExportStatus::Cancelled(path) => path,
    }
}

/// Why an export ended without a file.
#[derive(Debug)]
enum Stopped {
    Cancelled,
    Failed(String),
}

/// Runs `query` again over `forge`, read from the Archive at `archive`, and writes its result to
/// `path` in `format` on a background thread, calling `wake` from it once it ended.
pub(crate) fn start(
    (forge, archive): (&Arc<DataForgeEntry>, &Path),
    query: &str,
    (path, format): (PathBuf, QueryExportFormat),
    wake: impl Fn() + Send + 'static,
) -> ExportJob {
    let (sender, result) = mpsc::channel();
    let cancel = CancelToken::new();
    let thread_forge = Arc::clone(forge);
    let thread_archive = archive.to_path_buf();
    let thread_query = query.to_owned();
    let thread_path = path.clone();
    let thread_cancel = cancel.clone();
    let spawned = thread::Builder::new()
        .name("query-export".to_owned())
        .spawn(move || {
            let written = export(
                (&thread_forge, &thread_archive),
                &thread_query,
                (&thread_path, format.service()),
                &thread_cancel,
            );
            if sender.send(written).is_ok() {
                wake();
            }
        });
    match spawned {
        Ok(_handle) => ExportJob {
            status: QueryExportStatus::Exporting(path),
            cancel,
            result: Some(result),
        },
        Err(error) => ExportJob {
            status: QueryExportStatus::Failed(path, describe(&error)),
            cancel,
            result: None,
        },
    }
}

fn export(
    (forge, archive): (&DataForgeEntry, &Path),
    query: &str,
    out: (&Path, ResultFormat),
    cancel: &CancelToken,
) -> Result<QueryExport, Stopped> {
    let failed = |reason: String| Stopped::Failed(reason);
    let forge = forge.open().map_err(|error| failed(describe(&error)))?;
    let compiled = Query::parse(query)
        .and_then(|query| query.compile(&forge))
        .map_err(|error| failed(describe(&error)))?;
    let options = QueryExportOptions {
        cancel: Some(cancel),
        ..QueryExportOptions::default()
    };
    let run = compiled
        .run_while(|| !cancel.is_cancelled())
        .map_err(|error| failed(describe(&error)))?
        .ok_or(Stopped::Cancelled)?;
    match export_query(&compiled, &run, out, archive, &options) {
        Ok(export) => Ok(export),
        Err(QueryExportError::Cancelled) => Err(Stopped::Cancelled),
        Err(error) => Err(failed(describe(&error))),
    }
}
