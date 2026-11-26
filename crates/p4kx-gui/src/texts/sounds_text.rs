// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Sounds tab, looked up in the Fluent file of the current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundsText {
    /// The tab with the sounds of Entities.
    Tab,
    /// The label of the Entity field.
    EntityLabel,
    /// The placeholder of the empty Entity field.
    EntityHint,
    /// The action that lists the audio triggers of the Entity in the field.
    Show,
    /// The action that lists the audio triggers of the Record selected in the Archive tab.
    UseRecord,
    /// The label of the search field.
    SearchLabel,
    /// The placeholder of the empty search field.
    SearchHint,
    /// The action that opens the sound export.
    Export,
    /// The title before an Archive is open.
    EmptyTitle,
    /// The hint before an Archive is open.
    EmptyHint,
    /// Shown while the audio of the Archive is indexed.
    Loading,
    /// Shown when indexing failed; takes `reason`.
    Failed,
    /// Shown when indexing stopped without a result.
    Stopped,
    /// Shown when the Archive changed on disk.
    ArchiveChanged,
    /// The remark that audio files could not be read; takes `count` and `number`.
    Issues,
    /// Shown while no list is asked for.
    ListHint,
    /// Shown while the list is made.
    ListLoading,
    /// Shown when the Record asked for is no Entity.
    NotAnEntity,
    /// Shown when the list failed; takes `reason`.
    ListFailed,
    /// Shown when the Archive changed while the list was made.
    ListArchiveChanged,
    /// Shown when nothing matches the search.
    NoMatch,
    /// Shown when the Entity names no audio trigger.
    NoTriggers,
    /// The head of the trigger column.
    ColumnTrigger,
    /// The head of the status column.
    ColumnState,
    /// The head of the Wwise event column.
    ColumnEvents,
    /// The status of a trigger that leads to a soundbank.
    StateOk,
    /// The status of a trigger no ATL definition has.
    StateNoDefinition,
    /// The status of a trigger whose definition posts no Wwise event.
    StateNoEvent,
    /// The status of a trigger whose events no soundbank holds.
    StateNoSoundbank,
    /// The number of triggers of an Entity; takes `name`, `count`, `number` and `resolved`.
    CountEntity,
    /// The number of triggers a search found; takes `count`, `number`.
    CountSearch,
    /// The number of triggers a search found when the list shows only the first; takes `number`
    /// and `total`.
    CountSearchMore,
    /// The accessible name of the trigger list.
    ScrollTriggers,
    /// The accessible name of the list of sounds.
    ScrollVariants,
    /// Shown while no trigger is selected.
    SelectHint,
    /// Shown while the sounds of a trigger are resolved.
    VariantsLoading,
    /// Shown when no ATL definition has the selected trigger.
    Undefined,
    /// Shown when the selected trigger leads to no sound.
    NoVariants,
    /// The number of sounds of a trigger; takes `name`, `count` and `number`.
    VariantsCount,
    /// The heading of the broken links of a trigger; takes `count` and `number`.
    BreaksHeading,
    /// A broken link of a trigger; takes `name` and `reason`.
    BreakLine,
    /// A broken link of a trigger in a soundbank; takes `name`, `bank` and `reason`.
    BreakLineIn,
    /// The head of the event column.
    ColumnEvent,
    /// The head of the WEM ID column.
    ColumnWem,
    /// The head of the source column.
    ColumnSource,
    /// The head of the language column.
    ColumnLanguage,
    /// The head of the soundbank column.
    ColumnSoundbank,
    /// A source embedded in a soundbank.
    SourceEmbedded,
    /// A source whose start is embedded and whose rest is streamed.
    SourcePrefetch,
    /// A source streamed from a loose WEM.
    SourceStreamed,
    /// A source a plug-in generates.
    SourcePlugin,
    /// A source the game hands in at runtime.
    SourceExternal,
    /// A source that is a MIDI file.
    SourceMidi,
    /// A source of a kind this reader does not know.
    SourceOther,
    /// A language that is the fallback; takes `language`.
    LanguageFallback,
    /// The accessible name of a sound row; takes `name`, `bank` and `id`.
    VariantRow,
    /// The line of the selected sound; takes `path`, `id` and `source`.
    Detail,
    /// Shown while no sound is selected.
    SelectVariant,
    /// Shown when only the prefetched start of a sound is embedded.
    NotePrefetch,
    /// Shown when a source plug-in generates the sound.
    NotePlugin,
    /// Shown when an external source supplies the sound at runtime.
    NoteExternal,
    /// Shown when the sound is a MIDI file.
    NoteMidi,
    /// Shown when no soundbank or WEM Entry holds the WEM.
    NoteMissing,
    /// Shown when the Archive has no Entry for the file that holds a sound that should play.
    NoteNoEntry,
    /// The label of the language choice.
    LanguageLabel,
    /// The title of the sound export setup.
    ExportTitle,
    /// The heading of the choice of triggers.
    ExportScope,
    /// The choice of the selected trigger; takes `name`.
    ExportSelected,
    /// The choice of the selected trigger while none is selected.
    ExportSelectedNone,
    /// The choice of all triggers of the list; takes `count` and `number`.
    ExportAll,
    /// The heading of the choice of the format.
    ExportFormat,
    /// The action that starts the export.
    ExportStart,
    /// The title of the running export.
    ExportRunning,
    /// Shown while the export is planned.
    ExportPlanning,
    /// The progress of the export; takes `done`, `total` and `percent`.
    ExportProgress,
    /// The title of the summary of an export.
    SummaryTitle,
    /// The title of the summary of an export that was stopped.
    SummaryCancelledTitle,
    /// The title of an export that failed.
    FailedTitle,
    /// Shown when the export failed; takes `path` and `reason`.
    ExportFailed,
    /// Shown when the export stopped without a result; takes `path`.
    ExportStopped,
    /// The note that the export was stopped.
    SummaryCancelled,
    /// The counts of an export; takes `written`, `skipped` and `failed`.
    SummaryCounts,
    /// The folder of an export; takes `path`.
    SummaryTarget,
    /// The heading of the sounds written unchanged; takes `count` and `number`.
    SummaryUnconverted,
    /// The heading for origin receipt failures, with `count` and `number`.
    SummaryReceiptFailed,
    /// The heading of the sounds not exported; takes `count` and `number`.
    SummaryFailed,
    /// A sound written unchanged; takes `name` and `reason`.
    SummaryFile,
    /// A sound not exported; takes `name`, `id` and `reason`.
    SummaryFailure,
    /// A sound not exported that has no WEM; takes `name` and `reason`.
    SummaryFailurePlain,
}

impl SoundsText {
    /// Every text of the Sounds tab.
    pub(crate) const ALL: [Self; 89] = [
        Self::Tab,
        Self::EntityLabel,
        Self::EntityHint,
        Self::Show,
        Self::UseRecord,
        Self::SearchLabel,
        Self::SearchHint,
        Self::Export,
        Self::EmptyTitle,
        Self::EmptyHint,
        Self::Loading,
        Self::Failed,
        Self::Stopped,
        Self::ArchiveChanged,
        Self::Issues,
        Self::ListHint,
        Self::ListLoading,
        Self::NotAnEntity,
        Self::ListFailed,
        Self::ListArchiveChanged,
        Self::NoMatch,
        Self::NoTriggers,
        Self::ColumnTrigger,
        Self::ColumnState,
        Self::ColumnEvents,
        Self::StateOk,
        Self::StateNoDefinition,
        Self::StateNoEvent,
        Self::StateNoSoundbank,
        Self::CountEntity,
        Self::CountSearch,
        Self::CountSearchMore,
        Self::ScrollTriggers,
        Self::ScrollVariants,
        Self::SelectHint,
        Self::VariantsLoading,
        Self::Undefined,
        Self::NoVariants,
        Self::VariantsCount,
        Self::BreaksHeading,
        Self::BreakLine,
        Self::BreakLineIn,
        Self::ColumnEvent,
        Self::ColumnWem,
        Self::ColumnSource,
        Self::ColumnLanguage,
        Self::ColumnSoundbank,
        Self::SourceEmbedded,
        Self::SourcePrefetch,
        Self::SourceStreamed,
        Self::SourcePlugin,
        Self::SourceExternal,
        Self::SourceMidi,
        Self::SourceOther,
        Self::LanguageFallback,
        Self::VariantRow,
        Self::Detail,
        Self::SelectVariant,
        Self::NotePrefetch,
        Self::NotePlugin,
        Self::NoteExternal,
        Self::NoteMidi,
        Self::NoteMissing,
        Self::NoteNoEntry,
        Self::LanguageLabel,
        Self::ExportTitle,
        Self::ExportScope,
        Self::ExportSelected,
        Self::ExportSelectedNone,
        Self::ExportAll,
        Self::ExportFormat,
        Self::ExportStart,
        Self::ExportRunning,
        Self::ExportPlanning,
        Self::ExportProgress,
        Self::SummaryTitle,
        Self::SummaryCancelledTitle,
        Self::FailedTitle,
        Self::ExportFailed,
        Self::ExportStopped,
        Self::SummaryCancelled,
        Self::SummaryCounts,
        Self::SummaryTarget,
        Self::SummaryUnconverted,
        Self::SummaryReceiptFailed,
        Self::SummaryFailed,
        Self::SummaryFile,
        Self::SummaryFailure,
        Self::SummaryFailurePlain,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one arm per text")]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tab => "sounds-tab",
            Self::EntityLabel => "sounds-entity-label",
            Self::EntityHint => "sounds-entity-hint",
            Self::Show => "sounds-show",
            Self::UseRecord => "sounds-use-record",
            Self::SearchLabel => "sounds-search-label",
            Self::SearchHint => "sounds-search-hint",
            Self::Export => "sounds-export",
            Self::EmptyTitle => "sounds-empty-title",
            Self::EmptyHint => "sounds-empty-hint",
            Self::Loading => "sounds-loading",
            Self::Failed => "sounds-failed",
            Self::Stopped => "sounds-stopped",
            Self::ArchiveChanged => "sounds-archive-changed",
            Self::Issues => "sounds-issues",
            Self::ListHint => "sounds-list-hint",
            Self::ListLoading => "sounds-list-loading",
            Self::NotAnEntity => "sounds-not-an-entity",
            Self::ListFailed => "sounds-list-failed",
            Self::ListArchiveChanged => "sounds-list-archive-changed",
            Self::NoMatch => "sounds-no-match",
            Self::NoTriggers => "sounds-no-triggers",
            Self::ColumnTrigger => "sounds-column-trigger",
            Self::ColumnState => "sounds-column-state",
            Self::ColumnEvents => "sounds-column-events",
            Self::StateOk => "sounds-state-ok",
            Self::StateNoDefinition => "sounds-state-no-definition",
            Self::StateNoEvent => "sounds-state-no-event",
            Self::StateNoSoundbank => "sounds-state-no-soundbank",
            Self::CountEntity => "sounds-count-entity",
            Self::CountSearch => "sounds-count-search",
            Self::CountSearchMore => "sounds-count-search-more",
            Self::ScrollTriggers => "sounds-scroll-triggers",
            Self::ScrollVariants => "sounds-scroll-variants",
            Self::SelectHint => "sounds-select-hint",
            Self::VariantsLoading => "sounds-variants-loading",
            Self::Undefined => "sounds-undefined",
            Self::NoVariants => "sounds-no-variants",
            Self::VariantsCount => "sounds-variants-count",
            Self::BreaksHeading => "sounds-breaks-heading",
            Self::BreakLine => "sounds-break-line",
            Self::BreakLineIn => "sounds-break-line-in",
            Self::ColumnEvent => "sounds-column-event",
            Self::ColumnWem => "sounds-column-wem",
            Self::ColumnSource => "sounds-column-source",
            Self::ColumnLanguage => "sounds-column-language",
            Self::ColumnSoundbank => "sounds-column-soundbank",
            Self::SourceEmbedded => "sounds-source-embedded",
            Self::SourcePrefetch => "sounds-source-prefetch",
            Self::SourceStreamed => "sounds-source-streamed",
            Self::SourcePlugin => "sounds-source-plugin",
            Self::SourceExternal => "sounds-source-external",
            Self::SourceMidi => "sounds-source-midi",
            Self::SourceOther => "sounds-source-other",
            Self::LanguageFallback => "sounds-language-fallback",
            Self::VariantRow => "sounds-variant-row",
            Self::Detail => "sounds-detail",
            Self::SelectVariant => "sounds-select-variant",
            Self::NotePrefetch => "sounds-note-prefetch",
            Self::NotePlugin => "sounds-note-plugin",
            Self::NoteExternal => "sounds-note-external",
            Self::NoteMidi => "sounds-note-midi",
            Self::NoteMissing => "sounds-note-missing",
            Self::NoteNoEntry => "sounds-note-no-entry",
            Self::LanguageLabel => "sounds-language-label",
            Self::ExportTitle => "sounds-export-title",
            Self::ExportScope => "sounds-export-scope",
            Self::ExportSelected => "sounds-export-selected",
            Self::ExportSelectedNone => "sounds-export-selected-none",
            Self::ExportAll => "sounds-export-all",
            Self::ExportFormat => "sounds-export-format",
            Self::ExportStart => "sounds-export-start",
            Self::ExportRunning => "sounds-export-running",
            Self::ExportPlanning => "sounds-export-planning",
            Self::ExportProgress => "sounds-export-progress",
            Self::SummaryTitle => "sounds-summary-title",
            Self::SummaryCancelledTitle => "sounds-summary-cancelled-title",
            Self::FailedTitle => "sounds-failed-title",
            Self::ExportFailed => "sounds-export-failed",
            Self::ExportStopped => "sounds-export-stopped",
            Self::SummaryCancelled => "sounds-summary-cancelled",
            Self::SummaryCounts => "sounds-summary-counts",
            Self::SummaryTarget => "sounds-summary-target",
            Self::SummaryUnconverted => "sounds-summary-unconverted",
            Self::SummaryReceiptFailed => "sounds-summary-receipt-failed",
            Self::SummaryFailed => "sounds-summary-failed",
            Self::SummaryFile => "sounds-summary-file",
            Self::SummaryFailure => "sounds-summary-failure",
            Self::SummaryFailurePlain => "sounds-summary-failure-plain",
        }
    }
}

impl From<SoundsText> for crate::Text {
    fn from(text: SoundsText) -> Self {
        Self::Sounds(text)
    }
}
