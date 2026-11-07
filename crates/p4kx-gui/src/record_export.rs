// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod job;
mod progress;
mod selection;
mod summary;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use p4kx_export::RecordForm;
use p4kx_extraction::{ConflictPolicy, Filter, FilterError, WriteProtectionError};

pub use job::{RecordExport, RecordExportStatus};
pub(crate) use job::{Request, Selection, start};
pub use progress::{RecordExportProgress, RecordExportStage};
pub(crate) use selection::records_below;
pub use summary::{FailedRecord, RecordExportSummary, RenamedRecord};

use crate::TargetKind;
use crate::installation::refusal_of;

/// The forms the Record export dialog offers.
pub const RECORD_FORMS: [RecordForm; 5] = [
    RecordForm::Json,
    RecordForm::Ndjson,
    RecordForm::FlatCsv,
    RecordForm::FlatCsvRaw,
    RecordForm::FlatNdjson,
];

/// Which Records a Record export writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordScope {
    /// The Records of the node selected in the Record tree and below it.
    Tree,
    /// The Records whose record path matches the filter text.
    Filter,
}

/// The node of the Record tree a Record export can take its Records from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSelection {
    name: String,
    records: Arc<[u32]>,
}

impl TreeSelection {
    pub(crate) const fn new(name: String, records: Arc<[u32]>) -> Self {
        Self { name, records }
    }

    /// Returns the name of the selected node, empty for the DataForge node.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the number of Records at and below the selected node.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether no Record is at or below the selected node.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// The choices of a Record export before it starts: the Records, the form and the target.
#[derive(Debug, Clone)]
pub struct RecordExportSetup {
    scope: RecordScope,
    tree: Option<TreeSelection>,
    filter: String,
    form: RecordForm,
    tags: bool,
    kind: TargetKind,
    folder: Option<PathBuf>,
    refusal: Option<WriteProtectionError>,
    policy: ConflictPolicy,
}

/// Returns whether `form` can carry the tag path of a tag Reference.
#[must_use]
pub const fn form_annotates_tags(form: RecordForm) -> bool {
    matches!(
        form,
        RecordForm::Xml | RecordForm::Json | RecordForm::Ndjson
    )
}

impl RecordExportSetup {
    pub(crate) fn new(tree: Option<TreeSelection>, folder: Option<PathBuf>) -> Self {
        let scope = if tree.as_ref().is_some_and(|tree| !tree.is_empty()) {
            RecordScope::Tree
        } else {
            RecordScope::Filter
        };
        Self {
            scope,
            tree,
            filter: String::new(),
            form: RecordForm::Json,
            tags: false,
            kind: TargetKind::NewFolder,
            refusal: refusal_of(folder.as_deref()),
            folder,
            policy: ConflictPolicy::Skip,
        }
    }

    /// Returns which Records are exported.
    #[must_use]
    pub const fn scope(&self) -> RecordScope {
        self.scope
    }

    /// Chooses which Records are exported.
    pub const fn set_scope(&mut self, scope: RecordScope) {
        self.scope = scope;
    }

    /// Returns the selection of the Record tree, or `None` when no node of a loaded DataForge was
    /// selected as the setup opened.
    #[must_use]
    pub const fn tree(&self) -> Option<&TreeSelection> {
        self.tree.as_ref()
    }

    /// Returns the record-path filter: a glob with `*` or `?`, otherwise any part of the path.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Replaces the record-path filter and exports by it.
    pub fn set_filter(&mut self, filter: &str) {
        filter.clone_into(&mut self.filter);
        self.scope = RecordScope::Filter;
    }

    pub(crate) const fn filter_mut(&mut self) -> &mut String {
        &mut self.filter
    }

    /// Returns the form the Records are written in.
    #[must_use]
    pub const fn form(&self) -> RecordForm {
        self.form
    }

    /// Chooses the form the Records are written in.
    pub const fn set_form(&mut self, form: RecordForm) {
        self.form = form;
    }

    /// Returns whether the tag path of every tag Reference is written next to it.
    #[must_use]
    pub const fn tags(&self) -> bool {
        self.tags
    }

    /// Chooses whether the tag path of every tag Reference is written next to it; the flat forms
    /// ignore the choice, see [`form_annotates_tags`].
    pub const fn set_tags(&mut self, tags: bool) {
        self.tags = tags;
    }

    /// Returns where the Record export writes.
    #[must_use]
    pub const fn kind(&self) -> TargetKind {
        self.kind
    }

    /// Chooses where the Record export writes.
    pub const fn set_kind(&mut self, kind: TargetKind) {
        self.kind = kind;
    }

    /// Returns the chosen folder, or `None` while none is chosen.
    #[must_use]
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// Chooses the folder the new folder is created in, or the existing folder written into.
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

    /// Returns the Conflict policy for files already in an existing folder.
    #[must_use]
    pub const fn policy(&self) -> ConflictPolicy {
        self.policy
    }

    /// Chooses the Conflict policy for files already in an existing folder.
    pub const fn set_policy(&mut self, policy: ConflictPolicy) {
        self.policy = policy;
    }

    /// Returns whether the setup can start: a folder is chosen and the scope names Records.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.folder.is_some()
            && self.refusal.is_none()
            && match self.scope {
                RecordScope::Tree => self.tree.as_ref().is_some_and(|tree| !tree.is_empty()),
                RecordScope::Filter => !self.filter.is_empty() && self.filter_error().is_none(),
            }
    }

    /// Returns why the record-path filter is refused, when its text is too long or has too many
    /// wildcards.
    #[must_use]
    pub fn filter_error(&self) -> Option<FilterError> {
        Filter::check(&self.filter).err()
    }

    pub(crate) fn request(&self) -> Option<Request> {
        if !self.is_ready() {
            return None;
        }
        let selection = match self.scope {
            RecordScope::Tree => Selection::Records(Arc::clone(&self.tree.as_ref()?.records)),
            RecordScope::Filter => Selection::Filter(Filter::parse(&self.filter).ok()?),
        };
        Some(Request {
            selection,
            form: self.form,
            tags: self.tags && form_annotates_tags(self.form),
            kind: self.kind,
            folder: self.folder.clone()?,
            policy: match self.kind {
                TargetKind::NewFolder => ConflictPolicy::Skip,
                TargetKind::ExistingFolder => self.policy,
            },
        })
    }
}
