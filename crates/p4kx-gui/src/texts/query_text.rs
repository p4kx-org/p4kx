// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the Query tab, looked up in the Fluent file of the current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryText {
    /// The tab that runs Queries.
    Tab,
    /// The label of the Query editor.
    Label,
    /// The placeholder of the empty Query editor.
    Hint,
    /// The action that runs the Query.
    Run,
    /// The action that stops the running Query.
    Cancel,
    /// Shown while DataForge is read.
    Loading,
    /// Shown while the References between the Records are indexed for `referencing`.
    Indexing,
    /// Shown while the Records are selected.
    Selecting,
    /// The progress; takes `done`, `total` and `percent`.
    Progress,
    /// Shown once the Query was cancelled.
    Cancelled,
    /// Shown when the Query failed; takes `reason`.
    Failed,
    /// Shown when the Query stopped without a result.
    Stopped,
    /// Shown when the Archive changed on disk.
    ArchiveChanged,
    /// The error of a Query that cannot be run; takes `line`, `column` and `reason`.
    Invalid,
    /// The title before an Archive is open.
    EmptyTitle,
    /// The hint before an Archive is open.
    EmptyHint,
    /// Shown before a Query ran; takes the run shortcut.
    Prompt,
    /// Localized run shortcut for non-Mac platforms.
    RunShortcut,
    /// Shown when no Record matches.
    NoRows,
    /// The number of rows; takes `count` and `number`.
    Rows,
    /// The number of rows and failed Records; takes `count`, `number` and `failed`.
    RowsFailed,
    /// The unresolved References of the rows, after their number; takes `count` and `number`.
    Unresolved,
    /// The action that exports the result as CSV.
    ExportCsv,
    /// The action that exports the result as JSON.
    ExportJson,
    /// The file type filter of the CSV export dialog.
    FilterCsv,
    /// The file type filter of the JSON export dialog.
    FilterJson,
    /// Shown while the result is exported; takes `path`.
    Exporting,
    /// Shown once the result was exported; takes `count`, `number` and `path`.
    Exported,
    /// Shown when the export failed; takes `path` and `reason`.
    ExportFailed,
    /// The action that stops the running export.
    ExportCancel,
    /// Shown once the export was cancelled; takes `path`.
    ExportCancelled,
    /// The page shown; takes `page` and `pages`.
    Page,
    /// The button that shows the first page.
    PageFirst,
    /// The button that shows the previous page.
    PagePrevious,
    /// The button that shows the next page.
    PageNext,
    /// The button that shows the last page.
    PageLast,
    /// The names a completion does not list; takes `count` and `number`.
    CompletionMore,
    /// The action that saves the Query of the editor.
    Save,
    /// The heading of the saved Queries.
    SavedTitle,
    /// Shown while no Query is saved.
    SavedNone,
    /// The accessible name of a saved Query that runs it; takes `name`.
    SavedRun,
    /// The action that renames a saved Query.
    SavedRename,
    /// The action that deletes a saved Query.
    SavedDelete,
    /// The title of the dialog that saves the Query.
    SaveTitle,
    /// The title of the dialog that renames a saved Query.
    RenameTitle,
    /// The title of the question before a saved Query is deleted.
    DeleteTitle,
    /// The label of the name of a saved Query.
    NameLabel,
    /// The placeholder of the empty name.
    NameHint,
    /// The action that saves under the name.
    SaveAction,
    /// The action that renames to the name.
    RenameAction,
    /// The action that deletes the saved Query.
    DeleteAction,
    /// The question before a saved Query is deleted; takes `name`.
    DeleteQuestion,
    /// Shown when saving replaces the Query saved under the name.
    NameReplaces,
    /// Shown while the name is empty.
    NameEmpty,
    /// Shown when the name is too long; takes `max`.
    NameTooLong,
    /// Shown when another saved Query has the name.
    NameTaken,
    /// Shown when the name holds a control character.
    NameControl,
    /// Shown when no more Queries can be saved; takes `max` and `size`.
    NameFull,
    /// Shown when the Query of the editor cannot be saved.
    NameBadText,
}

impl QueryText {
    /// Every Query text.
    pub(crate) const ALL: [Self; 59] = [
        Self::Tab,
        Self::Label,
        Self::Hint,
        Self::Run,
        Self::Cancel,
        Self::Loading,
        Self::Indexing,
        Self::Selecting,
        Self::Progress,
        Self::Cancelled,
        Self::Failed,
        Self::Stopped,
        Self::ArchiveChanged,
        Self::Invalid,
        Self::EmptyTitle,
        Self::EmptyHint,
        Self::Prompt,
        Self::RunShortcut,
        Self::NoRows,
        Self::Rows,
        Self::RowsFailed,
        Self::Unresolved,
        Self::ExportCsv,
        Self::ExportJson,
        Self::FilterCsv,
        Self::FilterJson,
        Self::Exporting,
        Self::Exported,
        Self::ExportFailed,
        Self::ExportCancel,
        Self::ExportCancelled,
        Self::Page,
        Self::PageFirst,
        Self::PagePrevious,
        Self::PageNext,
        Self::PageLast,
        Self::CompletionMore,
        Self::Save,
        Self::SavedTitle,
        Self::SavedNone,
        Self::SavedRun,
        Self::SavedRename,
        Self::SavedDelete,
        Self::SaveTitle,
        Self::RenameTitle,
        Self::DeleteTitle,
        Self::NameLabel,
        Self::NameHint,
        Self::SaveAction,
        Self::RenameAction,
        Self::DeleteAction,
        Self::DeleteQuestion,
        Self::NameReplaces,
        Self::NameEmpty,
        Self::NameTooLong,
        Self::NameTaken,
        Self::NameControl,
        Self::NameFull,
        Self::NameBadText,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one arm per text")]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tab => "query-tab",
            Self::Label => "query-label",
            Self::Hint => "query-hint",
            Self::Run => "query-run",
            Self::Cancel => "query-cancel",
            Self::Loading => "query-loading",
            Self::Indexing => "query-indexing",
            Self::Selecting => "query-selecting",
            Self::Progress => "query-progress",
            Self::Cancelled => "query-cancelled",
            Self::Failed => "query-failed",
            Self::Stopped => "query-stopped",
            Self::ArchiveChanged => "query-archive-changed",
            Self::Invalid => "query-invalid",
            Self::EmptyTitle => "query-empty-title",
            Self::EmptyHint => "query-empty-hint",
            Self::Prompt => "query-prompt",
            Self::RunShortcut => "query-run-shortcut",
            Self::NoRows => "query-no-rows",
            Self::Rows => "query-rows",
            Self::RowsFailed => "query-rows-failed",
            Self::Unresolved => "query-unresolved",
            Self::ExportCsv => "query-export-csv",
            Self::ExportJson => "query-export-json",
            Self::FilterCsv => "query-filter-csv",
            Self::FilterJson => "query-filter-json",
            Self::Exporting => "query-exporting",
            Self::Exported => "query-exported",
            Self::ExportFailed => "query-export-failed",
            Self::ExportCancel => "query-export-cancel",
            Self::ExportCancelled => "query-export-cancelled",
            Self::Page => "query-page",
            Self::PageFirst => "query-page-first",
            Self::PagePrevious => "query-page-previous",
            Self::PageNext => "query-page-next",
            Self::PageLast => "query-page-last",
            Self::CompletionMore => "query-completion-more",
            Self::Save => "query-save",
            Self::SavedTitle => "query-saved-title",
            Self::SavedNone => "query-saved-none",
            Self::SavedRun => "query-saved-run",
            Self::SavedRename => "query-saved-rename",
            Self::SavedDelete => "query-saved-delete",
            Self::SaveTitle => "query-save-title",
            Self::RenameTitle => "query-rename-title",
            Self::DeleteTitle => "query-delete-title",
            Self::NameLabel => "query-name-label",
            Self::NameHint => "query-name-hint",
            Self::SaveAction => "query-save-action",
            Self::RenameAction => "query-rename-action",
            Self::DeleteAction => "query-delete-action",
            Self::DeleteQuestion => "query-delete-question",
            Self::NameReplaces => "query-name-replaces",
            Self::NameEmpty => "query-name-empty",
            Self::NameTooLong => "query-name-too-long",
            Self::NameTaken => "query-name-taken",
            Self::NameControl => "query-name-control",
            Self::NameFull => "query-name-full",
            Self::NameBadText => "query-name-bad-text",
        }
    }
}

impl From<QueryText> for crate::Text {
    fn from(text: QueryText) -> Self {
        Self::Query(text)
    }
}
