// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;

use p4kx_extraction::is_same_file;

use super::Model;
use crate::catalog_export::{is_gzip, normalised_catalog_file, refused, refused_gzip, start};
#[cfg(feature = "test-hooks")]
use crate::hold::Pause;
use crate::{CatalogExport, CatalogExportOptions, RecordTreeStatus};

/// The Basetool catalog of the window: the running or finished catalog.
#[derive(Debug, Default)]
pub(crate) struct CatalogState {
    run: Option<CatalogExport>,
    #[cfg(feature = "test-hooks")]
    gate: Option<Arc<crate::hold::Gate>>,
}

impl Model {
    /// Returns whether a Basetool catalog can be created: the open Archive has a DataForge that
    /// did not fail to load, no catalog is being created, and the DataForge is not a file on
    /// its own, since the catalog reads Entries too.
    #[must_use]
    pub fn catalog_available(&self) -> bool {
        self.has_dataforge()
            && !self.is_dataforge_file()
            && !self
                .catalog
                .run
                .as_ref()
                .is_some_and(CatalogExport::is_running)
    }

    /// Starts a normal catalog job for the open Archive and calls `wake` when it ends.
    /// Normalizes the filename and refuses invalid targets or incomplete publication.
    pub fn start_catalog(&mut self, file: PathBuf, wake: impl Fn() + Send + 'static) -> bool {
        self.start_catalog_with_options(file, CatalogExportOptions::default(), wake)
    }

    /// Starts a catalog job with explicit source selection and publication policy.
    /// Refusals write nothing; diagnostic permission applies only to this job.
    pub fn start_catalog_with_options(
        &mut self,
        file: PathBuf,
        options: CatalogExportOptions,
        wake: impl Fn() + Send + 'static,
    ) -> bool {
        if self.check_archive() || !self.catalog_available() {
            return false;
        }
        let Some(browser) = &self.browser else {
            return false;
        };
        let file = normalised_catalog_file(file);
        if is_gzip(&file) {
            self.catalog.run = Some(refused_gzip(file, options.publication_mode));
            return false;
        }
        if is_same_file(&file, browser.archive_index().path()) {
            self.catalog.run = Some(refused(file, options.publication_mode));
            return false;
        }
        let index = Arc::clone(browser.shared_index());
        #[cfg(feature = "test-hooks")]
        let run = start(
            &index,
            file,
            options,
            wake,
            self.catalog
                .gate
                .take()
                .map_or_else(Pause::default, Pause::at),
        );
        #[cfg(not(feature = "test-hooks"))]
        let run = start(&index, file, options, wake);
        self.catalog.run = Some(run);
        true
    }

    /// Makes the next Basetool catalog wait once it started building, until the returned hold is
    /// dropped or the catalog is cancelled.
    #[cfg(feature = "test-hooks")]
    pub fn hold_catalog(&mut self) -> crate::Hold {
        let (hold, gate) = crate::hold::Hold::new();
        self.catalog.gate = Some(gate);
        hold
    }

    /// Returns whether the open Archive has a DataForge that did not fail to load.
    pub(super) fn has_dataforge(&self) -> bool {
        self.browser
            .as_ref()
            .and_then(crate::Browser::records)
            .is_some_and(|tree| {
                !matches!(
                    tree.status(),
                    RecordTreeStatus::Failed(_) | RecordTreeStatus::ArchiveChanged
                )
            })
    }

    /// Returns the running or finished Basetool catalog, or `None` when there is none.
    #[must_use]
    pub const fn catalog(&self) -> Option<&CatalogExport> {
        self.catalog.run.as_ref()
    }

    /// Takes the outcome of a Basetool catalog that ended; returns whether one ended.
    pub fn poll_catalog(&mut self) -> bool {
        self.catalog.run.as_mut().is_some_and(CatalogExport::poll)
    }

    /// Asks the running Basetool catalog to stop.
    pub fn cancel_catalog(&self) {
        if let Some(run) = &self.catalog.run {
            run.cancel();
        }
    }

    /// Cancels the running Basetool catalog and waits until it ended.
    pub fn stop_catalog(&mut self) {
        if let Some(run) = &mut self.catalog.run {
            run.stop();
        }
    }

    /// Dismisses the outcome of a Basetool catalog that ended.
    pub fn close_catalog(&mut self) {
        if !self
            .catalog
            .run
            .as_ref()
            .is_some_and(CatalogExport::is_running)
        {
            self.catalog.run = None;
        }
    }
}
