// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Export menu and the Record export windows, looked up in the Fluent file of the
/// current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordExportText {
    /// The Export menu.
    Menu,
    /// The entry of the Export menu that opens the Record export setup.
    Action,
    /// The title of the setup window.
    SetupTitle,
    /// The heading of the Record choice.
    ScopeHeading,
    /// The Records of the node selected in the Record tree; takes `name` and `number`.
    ScopeTree,
    /// The Record tree choice while no node of a loaded DataForge is selected.
    ScopeTreeNone,
    /// The Records whose record path matches the filter.
    ScopeFilter,
    /// The placeholder of the filter field.
    FilterHint,
    /// The heading of the form choice.
    FormHeading,
    /// One JSON file per Record.
    FormJson,
    /// One NDJSON file; takes `name`.
    FormNdjson,
    /// One CSV file with a row per value; takes `name`.
    FormFlatCsv,
    /// The same with the exact values.
    FormFlatCsvRaw,
    /// One NDJSON file with an object per value; takes `name`.
    FormFlatNdjson,
    /// The choice to write the tag path next to every tag Reference; offered for the forms that
    /// can carry it.
    Tags,
    /// The action that starts the Record export.
    Start,
    /// The title of the progress window.
    RunningTitle,
    /// Shown while DataForge is read.
    Reading,
    /// Shown while the Records are chosen.
    Selecting,
    /// The progress; takes `done`, `total` and `percent`.
    Progress,
    /// The title of the summary of a complete Record export.
    SummaryTitle,
    /// The title of the summary of a cancelled Record export.
    SummaryCancelledTitle,
    /// The note that the export was cancelled.
    SummaryCancelled,
    /// The counts of the summary; takes `written`, `skipped`, `failed` and `cancelled`.
    SummaryCounts,
    /// The written count in the summary provenance tooltip.
    SummaryWritten,
    /// The Records with error values; takes `erroneous` and `errors`.
    SummaryErrors,
    /// Shown when the filter matched no Record.
    SummaryNoMatch,
    /// The target folder or file; takes `path`.
    SummaryTarget,
    /// The NDJSON file could not be written; takes `path` and `reason`.
    SummaryFileFailed,
    /// The heading of companion publication failures; takes `number`.
    SummaryOriginFailures,
    /// The heading of the failed Records; takes `number`.
    SummaryFailed,
    /// The heading of the kept files; takes `number`.
    SummarySkipped,
    /// One failed Record; takes `path`, `guid` and `reason`.
    SummaryFailure,
    /// The title of the window shown when the Record export could not run.
    FailedTitle,
    /// DataForge or the target could not be prepared; takes `path` and `reason`.
    Failed,
    /// The Record export stopped without a result; takes `path`.
    Stopped,
}

impl RecordExportText {
    /// Every text, for checks of the Fluent files.
    pub const ALL: [Self; 36] = [
        Self::Menu,
        Self::Action,
        Self::SetupTitle,
        Self::ScopeHeading,
        Self::ScopeTree,
        Self::ScopeTreeNone,
        Self::ScopeFilter,
        Self::FilterHint,
        Self::FormHeading,
        Self::FormJson,
        Self::FormNdjson,
        Self::FormFlatCsv,
        Self::FormFlatCsvRaw,
        Self::FormFlatNdjson,
        Self::Tags,
        Self::Start,
        Self::RunningTitle,
        Self::Reading,
        Self::Selecting,
        Self::Progress,
        Self::SummaryTitle,
        Self::SummaryCancelledTitle,
        Self::SummaryCancelled,
        Self::SummaryCounts,
        Self::SummaryWritten,
        Self::SummaryErrors,
        Self::SummaryNoMatch,
        Self::SummaryTarget,
        Self::SummaryFileFailed,
        Self::SummaryOriginFailures,
        Self::SummaryFailed,
        Self::SummarySkipped,
        Self::SummaryFailure,
        Self::FailedTitle,
        Self::Failed,
        Self::Stopped,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Menu => "menu-export",
            Self::Action => "record-export-action",
            Self::SetupTitle => "record-export-title",
            Self::ScopeHeading => "record-export-scope-heading",
            Self::ScopeTree => "record-export-scope-tree",
            Self::ScopeTreeNone => "record-export-scope-tree-none",
            Self::ScopeFilter => "record-export-scope-filter",
            Self::FilterHint => "record-export-filter-hint",
            Self::FormHeading => "record-export-form-heading",
            Self::FormJson => "record-export-form-json",
            Self::FormNdjson => "record-export-form-ndjson",
            Self::FormFlatCsv => "record-export-form-flat-csv",
            Self::FormFlatCsvRaw => "record-export-form-flat-csv-raw",
            Self::FormFlatNdjson => "record-export-form-flat-ndjson",
            Self::Tags => "record-export-tags",
            Self::Start => "record-export-start",
            Self::RunningTitle => "record-export-running-title",
            Self::Reading => "record-export-reading",
            Self::Selecting => "record-export-selecting",
            Self::Progress => "record-export-progress",
            Self::SummaryTitle => "record-export-summary-title",
            Self::SummaryCancelledTitle => "record-export-summary-cancelled-title",
            Self::SummaryCancelled => "record-export-summary-cancelled",
            Self::SummaryCounts => "record-export-summary-counts",
            Self::SummaryWritten => "record-export-summary-written",
            Self::SummaryErrors => "record-export-summary-errors",
            Self::SummaryNoMatch => "record-export-summary-no-match",
            Self::SummaryTarget => "record-export-summary-target",
            Self::SummaryFileFailed => "record-export-summary-file-failed",
            Self::SummaryOriginFailures => "record-export-summary-origin-failures",
            Self::SummaryFailed => "record-export-summary-failed",
            Self::SummarySkipped => "record-export-summary-skipped",
            Self::SummaryFailure => "record-export-summary-failure",
            Self::FailedTitle => "record-export-failed-title",
            Self::Failed => "record-export-failed",
            Self::Stopped => "record-export-stopped",
        }
    }
}

impl From<RecordExportText> for super::Text {
    fn from(text: RecordExportText) -> Self {
        Self::RecordExport(text)
    }
}
