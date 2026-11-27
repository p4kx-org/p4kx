// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Extraction windows, looked up in the Fluent file of the current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtractionText {
    /// The action that extracts the checked Entries.
    ActionSelected,
    /// The action that extracts the shown Entries.
    ActionShown,
    /// The action that extracts every Entry.
    ActionAll,
    /// The title of the setup window.
    SetupTitle,
    /// The heading of the scope choice.
    ScopeHeading,
    /// The checked Entries; takes `number`.
    ScopeSelected,
    /// The shown Entries; takes `number`.
    ScopeShown,
    /// Every Entry; takes `number`.
    ScopeAll,
    /// The heading of the exclusion field.
    ExcludeHeading,
    /// The hint of the empty exclusion field.
    ExcludeHint,
    /// A Filter is too long; takes `max`.
    FilterTooLong,
    /// A Filter has too many wildcards; takes `max`.
    FilterTooManyWildcards,
    /// The exclusion has too many Filters; takes `max`.
    ExcludeTooMany,
    /// A Filter of the exclusion is empty.
    ExcludeEmpty,
    /// The exclusion leaves no Entry to extract.
    ExcludeNothingLeft,
    /// The heading of the target choice.
    TargetHeading,
    /// A new folder inside the chosen folder.
    TargetNew,
    /// The chosen folder itself.
    TargetExisting,
    /// The name of the new folder; takes `name`.
    TargetNewName,
    /// Shown when the chosen folder lies inside a Star Citizen installation; takes
    /// `installation`.
    TargetInInstallation,
    /// Shown while no folder is chosen.
    FolderNone,
    /// The action that picks the folder through a folder dialog.
    ChooseFolder,
    /// The heading of the Conflict policy choice.
    PolicyHeading,
    /// Keeps existing files.
    PolicySkip,
    /// Replaces existing files.
    PolicyOverwrite,
    /// Replaces existing files only when they differ.
    PolicyIfChanged,
    /// The switch that converts `CryXml` Entries and DataForge to XML, textures and sounds.
    Conversion,
    /// The switch that merges a split texture into one DDS file.
    TextureMerge,
    /// The switch that exports a texture as PNG.
    TexturePng,
    /// The choice of the largest mip for the PNG.
    TextureMipLargest,
    /// The choice of a smaller mip level for the PNG.
    TextureMipLevel,
    /// The switch that also writes the raw Entries of a texture.
    TextureRaw,
    /// The label of the sound formats.
    AudioLabel,
    /// The choice of Ogg for sounds, WAV where a codec has no Ogg form.
    AudioOgg,
    /// The choice of WAV for sounds.
    AudioWav,
    /// The choice of unchanged WEMs for sounds.
    AudioRaw,
    /// The note on the Ogg fallback and that soundbanks are unpacked into a folder per bank.
    AudioBanks,
    /// The action that starts the Extraction.
    Start,
    /// The action that closes a window.
    Close,
    /// The question before a whole-Archive Extraction; takes `number`, `count` and `size`.
    ConfirmAll,
    /// The action that confirms a whole-Archive Extraction.
    ConfirmAllAction,
    /// The action that returns to the choices.
    Back,
    /// The title of the progress window.
    RunningTitle,
    /// The progress; takes `done`, `total` and `percent`.
    Progress,
    /// Shown once the Extraction was asked to stop.
    Cancelling,
    /// The action that stops the Extraction.
    Cancel,
    /// The title of the summary of a complete Extraction.
    SummaryTitle,
    /// The title of the summary of a cancelled Extraction.
    SummaryCancelledTitle,
    /// The title of the summary of an Extraction stopped by a change of the Archive.
    SummaryChangedTitle,
    /// The note that the Archive changed on disk and written files may be stale.
    SummaryChanged,
    /// The counts of the summary; takes `extracted`, `skipped`, `failed` and `cancelled`.
    SummaryCounts,
    /// The counts of the summary with Conversion on; takes `extracted`, `converted`, `skipped`,
    /// `failed` and `cancelled`.
    SummaryCountsConverted,
    /// The note that the exclusion left out parts of textures that stay, which the Conversion read
    /// all the same; takes `count` and `number`.
    SummaryExcludedParts,
    /// The counts of the Records written for the DataForge Entry; takes `written`, `skipped`,
    /// `failed`, `cancelled`, `errors` and `erroneous`.
    SummaryRecords,
    /// The written Record count could not be bound; takes `reason`.
    SummaryRecordCountOriginFailed,
    /// The target folder; takes `path`.
    SummaryTarget,
    /// The heading of the failed Entries; takes `number`.
    SummaryFailed,
    /// The heading of the skipped Entries; takes `number`.
    SummarySkipped,
    /// The heading of the cancelled Entries; takes `number`.
    SummaryCancelled,
    /// The heading of the textures and sounds that were not converted; takes `number`.
    SummaryUnconverted,
    /// The list of published origin companions for merged DDS textures.
    SummaryTextureReceipts,
    /// The heading of published companions for extracted files and converted XML or audio.
    SummaryFileOriginReceipts,
    /// One failed Entry; takes `path` and `reason`.
    SummaryFailure,
    /// The heading of the Records that were not written; takes `number`.
    SummaryRecordsFailed,
    /// One Record that was not written; takes `path`, `guid` and `reason`.
    SummaryRecordFailure,
    /// The title of the window shown when the Extraction could not run.
    FailedTitle,
    /// The target could not be prepared; takes `path` and `reason`.
    Failed,
    /// The Extraction stopped without a result; takes `path`.
    Stopped,
}

impl ExtractionText {
    /// Every text, for checks of the Fluent files.
    pub const ALL: [Self; 68] = [
        Self::ActionSelected,
        Self::ActionShown,
        Self::ActionAll,
        Self::SetupTitle,
        Self::ScopeHeading,
        Self::ScopeSelected,
        Self::ScopeShown,
        Self::ScopeAll,
        Self::ExcludeHeading,
        Self::ExcludeHint,
        Self::FilterTooLong,
        Self::FilterTooManyWildcards,
        Self::ExcludeTooMany,
        Self::ExcludeEmpty,
        Self::ExcludeNothingLeft,
        Self::TargetHeading,
        Self::TargetNew,
        Self::TargetExisting,
        Self::TargetNewName,
        Self::TargetInInstallation,
        Self::FolderNone,
        Self::ChooseFolder,
        Self::PolicyHeading,
        Self::PolicySkip,
        Self::PolicyOverwrite,
        Self::PolicyIfChanged,
        Self::Conversion,
        Self::TextureMerge,
        Self::TexturePng,
        Self::TextureMipLargest,
        Self::TextureMipLevel,
        Self::TextureRaw,
        Self::AudioLabel,
        Self::AudioOgg,
        Self::AudioWav,
        Self::AudioRaw,
        Self::AudioBanks,
        Self::Start,
        Self::Close,
        Self::ConfirmAll,
        Self::ConfirmAllAction,
        Self::Back,
        Self::RunningTitle,
        Self::Progress,
        Self::Cancelling,
        Self::Cancel,
        Self::SummaryTitle,
        Self::SummaryCancelledTitle,
        Self::SummaryChangedTitle,
        Self::SummaryChanged,
        Self::SummaryCounts,
        Self::SummaryCountsConverted,
        Self::SummaryExcludedParts,
        Self::SummaryRecords,
        Self::SummaryRecordCountOriginFailed,
        Self::SummaryTarget,
        Self::SummaryFailed,
        Self::SummarySkipped,
        Self::SummaryCancelled,
        Self::SummaryUnconverted,
        Self::SummaryTextureReceipts,
        Self::SummaryFileOriginReceipts,
        Self::SummaryFailure,
        Self::SummaryRecordsFailed,
        Self::SummaryRecordFailure,
        Self::FailedTitle,
        Self::Failed,
        Self::Stopped,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one arm per text")]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ActionSelected => "extract-selected",
            Self::ActionShown => "extract-shown",
            Self::ActionAll => "extract-all",
            Self::SetupTitle => "extraction-setup-title",
            Self::ScopeHeading => "extraction-scope-heading",
            Self::ScopeSelected => "extraction-scope-selected",
            Self::ScopeShown => "extraction-scope-shown",
            Self::ScopeAll => "extraction-scope-all",
            Self::ExcludeHeading => "extraction-exclude-heading",
            Self::ExcludeHint => "extraction-exclude-hint",
            Self::FilterTooLong => "filter-too-long",
            Self::FilterTooManyWildcards => "filter-too-many-wildcards",
            Self::ExcludeTooMany => "extraction-exclude-too-many",
            Self::ExcludeEmpty => "extraction-exclude-empty",
            Self::ExcludeNothingLeft => "extraction-exclude-nothing-left",
            Self::TargetHeading => "extraction-target-heading",
            Self::TargetNew => "extraction-target-new",
            Self::TargetExisting => "extraction-target-existing",
            Self::TargetNewName => "extraction-target-new-name",
            Self::TargetInInstallation => "extraction-target-in-installation",
            Self::FolderNone => "extraction-folder-none",
            Self::ChooseFolder => "extraction-choose-folder",
            Self::PolicyHeading => "extraction-policy-heading",
            Self::PolicySkip => "extraction-policy-skip",
            Self::PolicyOverwrite => "extraction-policy-overwrite",
            Self::PolicyIfChanged => "extraction-policy-if-changed",
            Self::Conversion => "extraction-conversion",
            Self::TextureMerge => "extraction-texture-merge",
            Self::TexturePng => "extraction-texture-png",
            Self::TextureMipLargest => "extraction-texture-mip-largest",
            Self::TextureMipLevel => "extraction-texture-mip-level",
            Self::TextureRaw => "extraction-texture-raw",
            Self::AudioLabel => "extraction-audio-label",
            Self::AudioOgg => "extraction-audio-ogg",
            Self::AudioWav => "extraction-audio-wav",
            Self::AudioRaw => "extraction-audio-raw",
            Self::AudioBanks => "extraction-audio-banks",
            Self::Start => "extraction-start",
            Self::Close => "extraction-close",
            Self::ConfirmAll => "extraction-confirm-all",
            Self::ConfirmAllAction => "extraction-confirm-all-action",
            Self::Back => "extraction-back",
            Self::RunningTitle => "extraction-running-title",
            Self::Progress => "extraction-progress",
            Self::Cancelling => "extraction-cancelling",
            Self::Cancel => "extraction-cancel",
            Self::SummaryTitle => "extraction-summary-title",
            Self::SummaryCancelledTitle => "extraction-summary-cancelled-title",
            Self::SummaryChangedTitle => "extraction-summary-changed-title",
            Self::SummaryChanged => "extraction-summary-changed",
            Self::SummaryCounts => "extraction-summary-counts",
            Self::SummaryCountsConverted => "extraction-summary-counts-converted",
            Self::SummaryExcludedParts => "extraction-summary-excluded-parts",
            Self::SummaryRecords => "extraction-summary-records",
            Self::SummaryRecordCountOriginFailed => "extraction-summary-record-count-origin-failed",
            Self::SummaryTarget => "extraction-summary-target",
            Self::SummaryFailed => "extraction-summary-failed",
            Self::SummarySkipped => "extraction-summary-skipped",
            Self::SummaryCancelled => "extraction-summary-cancelled",
            Self::SummaryUnconverted => "extraction-summary-unconverted",
            Self::SummaryTextureReceipts => "extraction-summary-texture-receipts",
            Self::SummaryFileOriginReceipts => "extraction-summary-file-origin-receipts",
            Self::SummaryFailure => "extraction-summary-failure",
            Self::SummaryRecordsFailed => "extraction-summary-records-failed",
            Self::SummaryRecordFailure => "extraction-summary-record-failure",
            Self::FailedTitle => "extraction-failed-title",
            Self::Failed => "extraction-failed",
            Self::Stopped => "extraction-stopped",
        }
    }
}
