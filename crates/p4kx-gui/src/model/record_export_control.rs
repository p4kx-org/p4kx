// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use super::Model;
use crate::record_export::{TreeSelection, records_below, start};
use crate::{RecordExport, RecordExportSetup, RecordTree, RecordTreeStatus, Texts};

/// The Record export of the window: the open setup and the running or finished export.
#[derive(Debug, Default)]
pub(crate) struct RecordExportState {
    setup: Option<RecordExportSetup>,
    run: Option<RecordExport>,
    #[cfg(feature = "test-hooks")]
    write_gate: Option<Arc<crate::hold::Gate>>,
}

impl RecordExportState {
    /// Closes the setup, because the Archive changed on disk; a running export goes on with what
    /// it read.
    pub(super) fn archive_changed(&mut self) {
        self.setup = None;
    }
}

impl Model {
    /// Returns whether a Record export can be set up: the open Archive has a DataForge that did
    /// not fail to load, and no Record export runs.
    #[must_use]
    pub fn record_export_available(&self) -> bool {
        self.has_dataforge()
            && !self
                .record_export
                .run
                .as_ref()
                .is_some_and(RecordExport::is_running)
    }

    /// Opens the Record export setup with the node selected in the Record tree, unless no
    /// Record export is available; a finished Record export is dismissed.
    pub fn begin_record_export(&mut self) {
        if !self.record_export_available() {
            return;
        }
        let tree = self
            .browser
            .as_ref()
            .and_then(crate::Browser::records)
            .and_then(tree_selection);
        self.record_export.run = None;
        self.record_export.setup =
            Some(RecordExportSetup::new(tree, self.extraction_folder.clone()));
    }

    /// Returns the open Record export setup, or `None` when none is open.
    #[must_use]
    pub const fn record_export_setup(&self) -> Option<&RecordExportSetup> {
        self.record_export.setup.as_ref()
    }

    /// Returns the open Record export setup for changes.
    pub const fn record_export_setup_mut(&mut self) -> Option<&mut RecordExportSetup> {
        self.record_export.setup.as_mut()
    }

    /// Returns the open Record export setup for changes together with the interface texts.
    pub(crate) const fn record_export_setup_with_texts(
        &mut self,
    ) -> Option<(&mut RecordExportSetup, &Texts)> {
        match &mut self.record_export.setup {
            Some(setup) => Some((setup, &self.texts)),
            None => None,
        }
    }

    /// Closes the Record export setup without exporting.
    pub fn close_record_export_setup(&mut self) {
        self.record_export.setup = None;
    }

    /// Starts the Record export of the open setup on a background thread, calling `wake` from it
    /// once it ended; returns whether it started.
    ///
    /// Nothing starts while the setup is not ready, while another Record export runs, or once
    /// the Archive changed on disk. The chosen folder becomes the folder the next Extraction and
    /// Record export start with.
    pub fn submit_record_export(&mut self, wake: impl Fn() + Send + 'static) -> bool {
        if self.check_archive() || !self.record_export_available() {
            return false;
        }
        let (Some(setup), Some(browser)) = (&self.record_export.setup, &self.browser) else {
            return false;
        };
        let Some(request) = setup.request() else {
            return false;
        };
        let index = Arc::clone(browser.shared_index());
        self.extraction_folder = Some(request.folder.clone());
        #[cfg(feature = "test-hooks")]
        let run = start(
            &index,
            request,
            wake,
            self.record_export
                .write_gate
                .take()
                .map_or_else(crate::hold::Pause::default, crate::hold::Pause::at),
        );
        #[cfg(not(feature = "test-hooks"))]
        let run = start(&index, request, wake);
        self.record_export.run = Some(run);
        self.record_export.setup = None;
        true
    }

    /// Holds the next Record export after source and target preparation until release or cancellation.
    #[cfg(feature = "test-hooks")]
    pub fn hold_record_export_writing(&mut self) -> crate::Hold {
        let (hold, gate) = crate::hold::Hold::new();
        self.record_export.write_gate = Some(gate);
        hold
    }

    /// Returns the running or finished Record export, or `None` when there is none.
    #[must_use]
    pub const fn record_export(&self) -> Option<&RecordExport> {
        self.record_export.run.as_ref()
    }

    /// Returns whether DataForge of the open Archive was read and is kept for the next Record
    /// export.
    #[must_use]
    pub fn record_export_keeps_dataforge(&self) -> bool {
        self.browser
            .as_ref()
            .is_some_and(|browser| browser.archive_index().forge().is_read())
    }

    /// Takes the outcome of a Record export that ended; returns whether one ended.
    pub fn poll_record_export(&mut self) -> bool {
        self.record_export
            .run
            .as_mut()
            .is_some_and(RecordExport::poll)
    }

    /// Asks the running Record export to stop.
    pub fn cancel_record_export(&self) {
        if let Some(run) = &self.record_export.run {
            run.cancel();
        }
    }

    /// Cancels the running Record export and waits until it ended, partial files removed.
    pub fn stop_record_export(&mut self) {
        if let Some(run) = &mut self.record_export.run {
            run.stop();
        }
    }

    /// Dismisses the outcome of a Record export that ended.
    pub fn close_record_export(&mut self) {
        if !self
            .record_export
            .run
            .as_ref()
            .is_some_and(RecordExport::is_running)
        {
            self.record_export.run = None;
        }
    }
}

/// Returns the Records of the node selected in `tree`, or `None` when no node is selected or
/// DataForge is not loaded.
fn tree_selection(tree: &RecordTree) -> Option<TreeSelection> {
    let selected = tree.selected()?;
    if !matches!(tree.status(), RecordTreeStatus::Ready(_)) {
        return None;
    }
    let records = records_below(tree, selected);
    Some(TreeSelection::new(
        tree.node(selected).name().to_owned(),
        records.into(),
    ))
}
