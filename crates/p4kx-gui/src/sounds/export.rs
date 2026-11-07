// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;

use p4kx_assembly::audio::{ExportOptions, ExportReport, ExportWatch, SoundFormat};
use p4kx_extraction::{ConflictPolicy, WriteProtectionError};

use super::worker::{ExportRequest, Request, Worker, Written};
use crate::installation::refusal_of;

/// Which sounds a sound export writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundsScope {
    /// The sounds of the selected trigger.
    Selected,
    /// The sounds of every trigger of the list that leads to a soundbank.
    All,
}

/// The choices of a sound export before it starts: the triggers, the format and the folder.
#[derive(Debug, Clone)]
pub struct SoundsExportSetup {
    scope: SoundsScope,
    selected: Option<String>,
    all: Vec<String>,
    format: SoundFormat,
    folder: Option<PathBuf>,
    refusal: Option<WriteProtectionError>,
    policy: ConflictPolicy,
}

impl SoundsExportSetup {
    pub(super) fn new(
        selected: Option<String>,
        all: Vec<String>,
        format: SoundFormat,
        folder: Option<PathBuf>,
    ) -> Self {
        let scope = if selected.is_some() {
            SoundsScope::Selected
        } else {
            SoundsScope::All
        };
        Self {
            scope,
            selected,
            all,
            format,
            refusal: refusal_of(folder.as_deref()),
            folder,
            policy: ConflictPolicy::Skip,
        }
    }

    /// Returns which triggers are exported.
    #[must_use]
    pub const fn scope(&self) -> SoundsScope {
        self.scope
    }

    /// Chooses which triggers are exported.
    pub const fn set_scope(&mut self, scope: SoundsScope) {
        self.scope = scope;
    }

    /// Returns the name of the selected trigger, or `None` when none is selected.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Returns how many triggers of the list lead to a soundbank.
    #[must_use]
    pub const fn all_len(&self) -> usize {
        self.all.len()
    }

    /// Returns the format the sounds are written in; one that does not fit a sound falls back to
    /// WAV and then to the WEM unchanged.
    #[must_use]
    pub const fn format(&self) -> SoundFormat {
        self.format
    }

    /// Chooses the format the sounds are written in.
    pub const fn set_format(&mut self, format: SoundFormat) {
        self.format = format;
    }

    /// Returns the folder the sounds are written into.
    #[must_use]
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// Chooses the folder the sounds are written into.
    pub fn set_folder(&mut self, folder: PathBuf) {
        self.refusal = refusal_of(Some(&folder));
        self.folder = Some(folder);
    }

    /// Returns why the chosen folder cannot be written to, when it lies inside a Star Citizen
    /// installation.
    #[must_use]
    pub const fn refusal(&self) -> Option<&WriteProtectionError> {
        self.refusal.as_ref()
    }

    /// Returns the Conflict policy for files already in the folder.
    #[must_use]
    pub const fn policy(&self) -> ConflictPolicy {
        self.policy
    }

    /// Chooses the Conflict policy for files already in the folder.
    pub const fn set_policy(&mut self, policy: ConflictPolicy) {
        self.policy = policy;
    }

    fn triggers(&self) -> Vec<String> {
        match self.scope {
            SoundsScope::Selected => self.selected.iter().cloned().collect(),
            SoundsScope::All => self.all.clone(),
        }
    }

    /// Returns whether the export can start: a folder outside every installation is chosen and
    /// the scope names a trigger.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.folder.is_some()
            && self.refusal.is_none()
            && match self.scope {
                SoundsScope::Selected => self.selected.is_some(),
                SoundsScope::All => !self.all.is_empty(),
            }
    }
}

/// Where a sound export stands.
#[derive(Debug, Clone, Copy)]
pub enum SoundsExportStatus<'a> {
    /// The sounds are being written.
    Running {
        /// How many sounds are handled.
        done: usize,
        /// How many sounds the export handles, zero while it is planned.
        total: usize,
        /// Whether the export was asked to stop.
        cancelling: bool,
    },
    /// The export ended; the report lists the files and the failures.
    Finished {
        /// What was written.
        report: &'a ExportReport,
        /// The triggers no ATL definition has, which were not exported.
        undefined: &'a [String],
        /// Whether the export was stopped before it handled every sound.
        cancelled: bool,
    },
    /// The export could not start, for the reason given, or stopped without a result.
    Failed(Option<&'a str>),
}

#[derive(Debug)]
enum Outcome {
    Running,
    Finished(Box<ExportReport>, Vec<String>),
    Failed(Option<String>),
}

/// A sound export on its background thread, or its outcome.
#[derive(Debug)]
pub struct SoundsExport {
    id: u64,
    folder: PathBuf,
    watch: Arc<ExportWatch>,
    outcome: Outcome,
}

impl SoundsExport {
    /// Returns the number of the export, distinct among those of this window.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Returns the folder the sounds are written into.
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Returns whether the sounds are being written.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.outcome, Outcome::Running)
    }

    /// Asks the export to stop after the sounds it is writing.
    pub fn cancel(&self) {
        self.watch.cancel();
    }

    /// Returns where the export stands.
    #[must_use]
    pub fn status(&self) -> SoundsExportStatus<'_> {
        match &self.outcome {
            Outcome::Running => SoundsExportStatus::Running {
                done: self.watch.done(),
                total: self.watch.total(),
                cancelling: self.watch.is_cancelled(),
            },
            Outcome::Finished(report, undefined) => SoundsExportStatus::Finished {
                report,
                undefined,
                cancelled: self.watch.is_cancelled(),
            },
            Outcome::Failed(reason) => SoundsExportStatus::Failed(reason.as_deref()),
        }
    }

    pub(super) fn finish(&mut self, result: Result<Written, String>) {
        self.outcome = match result {
            Ok((report, undefined)) => Outcome::Finished(Box::new(report), undefined),
            Err(reason) => Outcome::Failed(Some(reason)),
        };
    }

    pub(super) fn lost(&mut self) {
        if self.is_running() {
            self.outcome = Outcome::Failed(None);
        }
    }
}

/// Starts the export of `setup` on `worker` as export `id`.
pub(super) fn start(worker: &Worker, id: u64, setup: &SoundsExportSetup) -> Option<SoundsExport> {
    let folder = setup.folder.clone()?;
    let watch = Arc::new(ExportWatch::default());
    let request = ExportRequest {
        id,
        triggers: setup.triggers(),
        folder: folder.clone(),
        options: ExportOptions {
            format: setup.format,
            policy: setup.policy,
        },
        watch: Arc::clone(&watch),
    };
    let mut export = SoundsExport {
        id,
        folder,
        watch,
        outcome: Outcome::Running,
    };
    if !worker.request(Request::Export(request)) {
        export.outcome = Outcome::Failed(None);
    }
    Some(export)
}
