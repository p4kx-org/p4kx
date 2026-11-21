// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Comparison tab, looked up in the Fluent file of the current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComparisonText {
    /// The tab that browses the open Archive.
    TabArchive,
    /// The tab that compares two Archives.
    TabComparison,
    /// The label of the old Archive's picker.
    OldArchive,
    /// The label of the new Archive's picker.
    NewArchive,
    /// The picker before an Archive is picked.
    PickArchive,
    /// The picker entry that picks a file through a file dialog.
    ChooseFile,
    /// The label of the level switches.
    Levels,
    /// The Entry level.
    LevelEntries,
    /// The Record level.
    LevelRecords,
    /// Shown in the list when the Entry level was not compared.
    EntriesSkipped,
    /// The action that starts the Comparison.
    Start,
    /// The action that stops the Comparison.
    Cancel,
    /// Shown while both Archives open.
    Opening,
    /// The progress; takes `done`, `total` and `percent`.
    Progress,
    /// Shown while the list is prepared.
    Preparing,
    /// Shown once the Comparison was cancelled.
    Cancelled,
    /// Shown when the Comparison failed; takes `reason`.
    Failed,
    /// Shown when the Comparison stopped without a result.
    Stopped,
    /// The title before a Comparison ran.
    EmptyTitle,
    /// The hint before a Comparison ran.
    EmptyHint,
    /// The key of the old side.
    FactOld,
    /// The key of the new side.
    FactNew,
    /// One side; takes `channel` and `build`.
    Side,
    /// A side outside a Channel folder.
    ChannelUnknown,
    /// The added Entries.
    Added,
    /// The removed Entries.
    Removed,
    /// The changed Entries.
    Changed,
    /// Incomparable matched Entry pairs; takes `number`.
    Incomparable,
    /// Shown when the list is empty and some Entry pairs are incomparable.
    NoConfirmedDifferences,
    /// The section of the counts per top-level directory; takes `number`.
    Directories,
    /// The Entries at the root of the Archive.
    Root,
    /// The directory column.
    ColumnDirectory,
    /// The change column.
    ColumnChange,
    /// The path column.
    ColumnPath,
    /// The old size column.
    ColumnOldSize,
    /// The new size column.
    ColumnNewSize,
    /// A size the side does not have.
    NoSize,
    /// The action that saves the JSON report.
    SaveReport,
    /// The file type filter of the save dialog.
    ReportFilter,
    /// Shown while the report is written; takes `path`.
    ReportSaving,
    /// Shown once the report was written; takes `path`.
    ReportSaved,
    /// Shown when the report could not be written; takes `path` and `reason`.
    ReportFailed,
    /// Shown in the list when the Archives do not differ.
    NoDifferences,
    /// Shown in the list when no change matches the Filter.
    NoMatches,
    /// The number of shown changes; takes `number`.
    Shown,
    /// The accessible name of a change row; takes `change` and `path`.
    Row,
}

impl ComparisonText {
    /// Every text of the Comparison tab.
    pub const ALL: [Self; 46] = [
        Self::TabArchive,
        Self::TabComparison,
        Self::OldArchive,
        Self::NewArchive,
        Self::PickArchive,
        Self::ChooseFile,
        Self::Levels,
        Self::LevelEntries,
        Self::LevelRecords,
        Self::EntriesSkipped,
        Self::Start,
        Self::Cancel,
        Self::Opening,
        Self::Progress,
        Self::Preparing,
        Self::Cancelled,
        Self::Failed,
        Self::Stopped,
        Self::EmptyTitle,
        Self::EmptyHint,
        Self::FactOld,
        Self::FactNew,
        Self::Side,
        Self::ChannelUnknown,
        Self::Added,
        Self::Removed,
        Self::Changed,
        Self::Incomparable,
        Self::NoConfirmedDifferences,
        Self::Directories,
        Self::Root,
        Self::ColumnDirectory,
        Self::ColumnChange,
        Self::ColumnPath,
        Self::ColumnOldSize,
        Self::ColumnNewSize,
        Self::NoSize,
        Self::SaveReport,
        Self::ReportFilter,
        Self::ReportSaving,
        Self::ReportSaved,
        Self::ReportFailed,
        Self::NoDifferences,
        Self::NoMatches,
        Self::Shown,
        Self::Row,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TabArchive => "comparison-tab-archive",
            Self::TabComparison => "comparison-tab-comparison",
            Self::OldArchive => "comparison-old-archive",
            Self::NewArchive => "comparison-new-archive",
            Self::PickArchive => "comparison-pick-archive",
            Self::ChooseFile => "comparison-choose-file",
            Self::Levels => "comparison-levels",
            Self::LevelEntries => "comparison-level-entries",
            Self::LevelRecords => "comparison-level-records",
            Self::EntriesSkipped => "comparison-entries-skipped",
            Self::Start => "comparison-start",
            Self::Cancel => "comparison-cancel",
            Self::Opening => "comparison-opening",
            Self::Progress => "comparison-progress",
            Self::Preparing => "comparison-preparing",
            Self::Cancelled => "comparison-cancelled",
            Self::Failed => "comparison-failed",
            Self::Stopped => "comparison-stopped",
            Self::EmptyTitle => "comparison-empty-title",
            Self::EmptyHint => "comparison-empty-hint",
            Self::FactOld => "comparison-fact-old",
            Self::FactNew => "comparison-fact-new",
            Self::Side => "comparison-side",
            Self::ChannelUnknown => "comparison-channel-unknown",
            Self::Added => "comparison-added",
            Self::Removed => "comparison-removed",
            Self::Changed => "comparison-changed",
            Self::Incomparable => "comparison-incomparable",
            Self::NoConfirmedDifferences => "comparison-no-confirmed-differences",
            Self::Directories => "comparison-directories",
            Self::Root => "comparison-root",
            Self::ColumnDirectory => "comparison-column-directory",
            Self::ColumnChange => "comparison-column-change",
            Self::ColumnPath => "comparison-column-path",
            Self::ColumnOldSize => "comparison-column-old-size",
            Self::ColumnNewSize => "comparison-column-new-size",
            Self::NoSize => "comparison-no-size",
            Self::SaveReport => "comparison-save-report",
            Self::ReportFilter => "comparison-report-filter",
            Self::ReportSaving => "comparison-report-saving",
            Self::ReportSaved => "comparison-report-saved",
            Self::ReportFailed => "comparison-report-failed",
            Self::NoDifferences => "comparison-no-differences",
            Self::NoMatches => "comparison-no-matches",
            Self::Shown => "comparison-shown",
            Self::Row => "comparison-row",
        }
    }
}
