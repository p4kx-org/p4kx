// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Basetool catalog entry of the Export menu and its windows, looked up in the
/// Fluent file of the current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogText {
    /// The entry of the Export menu that asks for the catalog file.
    Action,
    /// Explicitly requests a catalog intended for diagnosis rather than normal publication.
    DiagnosticAction,
    /// The progress title of an explicitly requested diagnostic catalog.
    DiagnosticRunningTitle,
    /// The summary title of an explicitly requested diagnostic catalog.
    DiagnosticSummaryTitle,
    /// Explains the consumer rule of a diagnostic catalog.
    DiagnosticNotice,
    /// Explains that incomplete normal publication wrote no file.
    Incomplete,
    /// The selected sources were read and projected successfully.
    HealthComplete,
    /// Some selected sources could not be read or projected successfully.
    HealthIncomplete,
    /// The affected kind, reason and count of a failed original Entry operation.
    HealthEntryDiagnostic,
    /// The file type of the save dialog.
    FileFilter,
    /// The file type of the save dialog for the zstd-compressed catalog.
    FileFilterCompressed,
    /// The title of the progress window.
    RunningTitle,
    /// Shown while DataForge is read.
    Reading,
    /// The progress while Records are checked; takes `done`, `total` and `percent`.
    Progress,
    /// The progress while Records are read for the crafting data; takes `done`, `total` and
    /// `percent`.
    CraftingProgress,
    /// A warning of the game data that makes the catalog incomplete; takes `kind` (the name
    /// `CraftingWarning::kind` gives), `count` and `message`, the English sentence for a kind
    /// this version does not know.
    Warning,
    /// The files a warning names beyond those listed; takes `number`.
    WarningMore,
    /// Shown while the file is written; takes `path`.
    Writing,
    /// The title of the summary.
    SummaryTitle,
    /// The heading of the counts per kind.
    KindsHeading,
    /// The counts of one kind; takes `kind`, `written`, `skipped` and `complete` (`yes` or `no`).
    SummaryKind,
    /// What the loadouts hold; takes `ports`, `unresolved`, `docked` and `outside`.
    SummaryLoadouts,
    /// The Records left out as development leftovers over every kind; takes `number`.
    SummarySkipped,
    /// The heading of the playability counts.
    PlayabilityHeading,
    /// The playability counts of the items or the ships; takes `kind` (`items` or `ships`),
    /// `player`, `ai`, `mission`, `special` and `unclassified`.
    SummaryPlayability,
    /// The heading of the Records that could not be read; takes `number`.
    SummaryFailed,
    /// One Record that could not be read; takes `path`, `guid` and `reason`.
    SummaryFailure,
    /// The heading of the texts per language.
    TextsHeading,
    /// How the texts resolved in one language; takes `language`, `resolved`, `missing` and
    /// `placeholders`.
    SummaryTexts,
    /// A language whose `global.ini` the Archive lacks; takes `language` and `missing`.
    SummaryTextsUnavailable,
    /// The missing texts over every language; takes `number`.
    SummaryMissing,
    /// The size of the catalog; takes `size` and `limit`.
    SummarySize,
    /// The size of a compressed catalog; takes `size`, `compressed` and `limit`.
    SummarySizeCompressed,
    /// The catalog is close to the upload limit; takes `limit`.
    NearLimit,
    /// The catalog is over the upload limit; takes `limit`.
    OverLimit,
    /// The plain JSON catalog is close to the upload limit; takes `limit`.
    NearLimitPlain,
    /// The plain JSON catalog is over the upload limit; takes `limit`.
    OverLimitPlain,
    /// The decompressed catalog is close to the Basetool limit for decompressed content; takes `limit`.
    DecompressedNearLimit,
    /// The decompressed catalog is over that limit; takes `limit`.
    DecompressedOverLimit,
    /// The catalog file; takes `path`.
    SummaryFile,
    /// The title of the window of a cancelled catalog.
    CancelledTitle,
    /// The note that the catalog was cancelled.
    Cancelled,
    /// The title of the window shown when the catalog could not be created.
    FailedTitle,
    /// The catalog could not be created; takes `path` and `reason`.
    Failed,
    /// The catalog stopped without a result; takes `path`.
    Stopped,
    /// The chosen file is the open Archive; takes `path`.
    IsArchive,
    /// The chosen file ends in `.gz`, which is not written; takes `path`.
    GzipNotSupported,
}

impl CatalogText {
    /// Every text, for checks of the Fluent files.
    pub const ALL: [Self; 47] = [
        Self::Action,
        Self::DiagnosticAction,
        Self::DiagnosticRunningTitle,
        Self::DiagnosticSummaryTitle,
        Self::DiagnosticNotice,
        Self::Incomplete,
        Self::HealthComplete,
        Self::HealthIncomplete,
        Self::HealthEntryDiagnostic,
        Self::FileFilter,
        Self::FileFilterCompressed,
        Self::RunningTitle,
        Self::Reading,
        Self::Progress,
        Self::CraftingProgress,
        Self::Warning,
        Self::WarningMore,
        Self::Writing,
        Self::SummaryTitle,
        Self::KindsHeading,
        Self::SummaryKind,
        Self::SummaryLoadouts,
        Self::SummarySkipped,
        Self::PlayabilityHeading,
        Self::SummaryPlayability,
        Self::SummaryFailed,
        Self::SummaryFailure,
        Self::TextsHeading,
        Self::SummaryTexts,
        Self::SummaryTextsUnavailable,
        Self::SummaryMissing,
        Self::SummarySize,
        Self::SummarySizeCompressed,
        Self::NearLimit,
        Self::OverLimit,
        Self::NearLimitPlain,
        Self::OverLimitPlain,
        Self::DecompressedNearLimit,
        Self::DecompressedOverLimit,
        Self::SummaryFile,
        Self::CancelledTitle,
        Self::Cancelled,
        Self::FailedTitle,
        Self::Failed,
        Self::Stopped,
        Self::IsArchive,
        Self::GzipNotSupported,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Action => "catalog-action",
            Self::DiagnosticAction => "catalog-diagnostic-action",
            Self::DiagnosticRunningTitle => "catalog-diagnostic-running-title",
            Self::DiagnosticSummaryTitle => "catalog-diagnostic-summary-title",
            Self::DiagnosticNotice => "catalog-diagnostic-notice",
            Self::Incomplete => "catalog-incomplete",
            Self::HealthComplete => "catalog-health-complete",
            Self::HealthIncomplete => "catalog-health-incomplete",
            Self::HealthEntryDiagnostic => "catalog-health-entry-diagnostic",
            Self::FileFilter => "catalog-file-filter",
            Self::FileFilterCompressed => "catalog-file-filter-compressed",
            Self::RunningTitle => "catalog-running-title",
            Self::Reading => "catalog-reading",
            Self::Progress => "catalog-progress",
            Self::CraftingProgress => "catalog-crafting-progress",
            Self::Warning => "catalog-warning",
            Self::WarningMore => "catalog-warning-more",
            Self::Writing => "catalog-writing",
            Self::SummaryTitle => "catalog-summary-title",
            Self::KindsHeading => "catalog-kinds-heading",
            Self::SummaryKind => "catalog-summary-kind",
            Self::SummaryLoadouts => "catalog-summary-loadouts",
            Self::SummarySkipped => "catalog-summary-skipped",
            Self::PlayabilityHeading => "catalog-playability-heading",
            Self::SummaryPlayability => "catalog-summary-playability",
            Self::SummaryFailed => "catalog-summary-failed",
            Self::SummaryFailure => "catalog-summary-failure",
            Self::TextsHeading => "catalog-texts-heading",
            Self::SummaryTexts => "catalog-summary-texts",
            Self::SummaryTextsUnavailable => "catalog-summary-texts-unavailable",
            Self::SummaryMissing => "catalog-summary-missing",
            Self::SummarySize => "catalog-summary-size",
            Self::SummarySizeCompressed => "catalog-summary-size-compressed",
            Self::NearLimit => "catalog-near-limit",
            Self::OverLimit => "catalog-over-limit",
            Self::NearLimitPlain => "catalog-near-limit-plain",
            Self::OverLimitPlain => "catalog-over-limit-plain",
            Self::DecompressedNearLimit => "catalog-decompressed-near-limit",
            Self::DecompressedOverLimit => "catalog-decompressed-over-limit",
            Self::SummaryFile => "catalog-summary-file",
            Self::CancelledTitle => "catalog-cancelled-title",
            Self::Cancelled => "catalog-cancelled",
            Self::FailedTitle => "catalog-failed-title",
            Self::Failed => "catalog-failed",
            Self::Stopped => "catalog-stopped",
            Self::IsArchive => "catalog-output-is-archive",
            Self::GzipNotSupported => "catalog-output-gzip",
        }
    }
}

impl From<CatalogText> for super::Text {
    fn from(text: CatalogText) -> Self {
        Self::Catalog(text)
    }
}
