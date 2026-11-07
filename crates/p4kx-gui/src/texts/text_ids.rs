// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{
    AboutText, ArchiveKeyText, AssemblyExportText, AudioText, CatalogText, ChangeExtractionText,
    CharacterText, ComparisonText, ContainerText, ContentSearchText, DataForgeText, DiffText,
    EntityText, ExtractionText, FolderConversionText, GroupText, ImageText, IncomingText,
    LocalizationText, ModelExportText, ModelInfoText, ModelText, ProtectionText, QueryText,
    RecordChangeText, RecordExportText, RecordLanguageText, RecordText, RenameText, SchemaText,
    ScrollText, SoundsText, TagsText, Text, UiText, XmlText,
};
use crate::Language;

impl Text {
    /// Every text outside the Extraction windows.
    const BASE: [Self; 73] = [
        Self::AppTitle,
        Self::MenuArchive,
        Self::ActionOpen,
        Self::ActionOpenDataForge,
        Self::ActionAddFolder,
        Self::ActionAddArchive,
        Self::MenuRecent,
        Self::RecentNone,
        Self::ChannelsHeading,
        Self::ChannelsNone,
        Self::SidebarShow,
        Self::SidebarHide,
        Self::MenuLanguage,
        Self::LanguageName(Language::German),
        Self::LanguageName(Language::BritishEnglish),
        Self::MenuHelp,
        Self::FontLicence,
        Self::UpdateCheck,
        Self::UpdateAvailable,
        Self::DialogArchiveFilter,
        Self::DialogDataForgeFilter,
        Self::ColumnName,
        Self::ColumnCompression,
        Self::ColumnEncryption,
        Self::ColumnSize,
        Self::SelectAll,
        Self::SelectEntry,
        Self::CompressionStore,
        Self::CompressionDeflate,
        Self::CompressionZstd,
        Self::CompressionOther,
        Self::EncryptedYes,
        Self::EncryptedNo,
        Self::ExpandFolder,
        Self::CollapseFolder,
        Self::SearchLabel,
        Self::SearchHint,
        Self::DropHint,
        Self::EmptyTitle,
        Self::DropRelease,
        Self::QuickFilters,
        Self::CloseDialog,
        Self::ShortcutReload,
        Self::ShortcutExtractSelected,
        Self::StatusEmpty,
        Self::StatusOpening,
        Self::StatusFailed,
        Self::StatusOpeningStopped,
        Self::StatusEntries,
        Self::StatusShown,
        Self::StatusSelected,
        Self::StatusSelectedHidden,
        Self::StatusSearching,
        Self::StatusBuild,
        Self::StatusBuildUnknown,
        Self::PreviewTabText,
        Self::PreviewTabHex,
        Self::PreviewCopy,
        Self::PreviewNone,
        Self::PreviewLoading,
        Self::PreviewTooLarge,
        Self::PreviewTruncated,
        Self::PreviewFailed,
        Self::PreviewStopped,
        Self::PreviewArchiveChanged,
        Self::ArchiveChanged,
        Self::ArchiveIssues,
        Self::GameRunning,
        Self::ArchiveTestBuild,
        Self::ArchiveTestBuildNda,
        Self::ActionReload,
        Self::PreviewKeyRequired,
        Self::PreviewKeyRejected,
    ];

    /// Returns every text, for checks of the Fluent files.
    pub fn all() -> impl Iterator<Item = Self> {
        Self::BASE
            .into_iter()
            .chain(ExtractionText::ALL.map(Self::Extraction))
            .chain(ProtectionText::ALL.map(Self::Protection))
            .chain(ComparisonText::ALL.map(Self::Comparison))
            .chain(DataForgeText::ALL.map(Self::DataForge))
            .chain(ChangeExtractionText::ALL.map(Self::ChangeExtraction))
            .chain(DiffText::ALL.map(Self::Diff))
            .chain(RecordChangeText::ALL.map(Self::RecordChange))
            .chain(SchemaText::ALL.map(Self::Schema))
            .chain(GroupText::ALL.map(Self::Group))
            .chain(ImageText::ALL.map(Self::Image))
            .chain(AudioText::ALL.map(Self::Audio))
            .chain(XmlText::ALL.map(Self::Xml))
            .chain(ScrollText::ALL.map(Self::Scroll))
            .chain(RenameText::ALL.map(Self::Rename))
            .chain(QueryText::ALL.map(Self::Query))
            .chain(ContainerText::ALL.map(Self::Container))
            .chain(RecordExportText::ALL.map(Self::RecordExport))
            .chain(CatalogText::ALL.map(Self::Catalog))
            .chain(RecordText::ALL.map(Self::Record))
            .chain(IncomingText::ALL.map(Self::Incoming))
            .chain(RecordLanguageText::ALL.map(Self::RecordLanguage))
            .chain(LocalizationText::ALL.map(Self::Localization))
            .chain(TagsText::ALL.map(Self::Tags))
            .chain(UiText::ALL.map(Self::Ui))
            .chain(SoundsText::ALL.map(Self::Sounds))
            .chain(ContentSearchText::ALL.map(Self::ContentSearch))
            .chain(ModelText::ALL.map(Self::Model))
            .chain(ModelInfoText::ALL.map(Self::ModelInfo))
            .chain(ModelExportText::ALL.map(Self::ModelExport))
            .chain(FolderConversionText::ALL.map(Self::FolderConversion))
            .chain(EntityText::ALL.map(Self::Entity))
            .chain(CharacterText::ALL.map(Self::Character))
            .chain(AssemblyExportText::ALL.map(Self::AssemblyExport))
            .chain(ArchiveKeyText::ALL.map(Self::Key))
            .chain(AboutText::ALL.map(Self::About))
    }

    /// Returns the Fluent message id.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one arm per text")]
    pub const fn id(self) -> &'static str {
        match self {
            Self::AppTitle => "app-title",
            Self::MenuArchive => "menu-archive",
            Self::ActionOpen => "action-open",
            Self::ActionOpenDataForge => "action-open-dataforge",
            Self::ActionAddFolder => "action-add-folder",
            Self::ActionAddArchive => "action-add-archive",
            Self::MenuRecent => "menu-recent",
            Self::RecentNone => "recent-none",
            Self::ChannelsHeading => "channels-heading",
            Self::ChannelsNone => "channels-none",
            Self::SidebarShow => "sidebar-show",
            Self::SidebarHide => "sidebar-hide",
            Self::MenuLanguage => "menu-language",
            Self::LanguageName(Language::German) => "language-de-DE",
            Self::LanguageName(Language::BritishEnglish) => "language-en-GB",
            Self::MenuHelp => "menu-help",
            Self::FontLicence => "font-licence",
            Self::UpdateCheck => "update-check",
            Self::UpdateAvailable => "update-available",
            Self::DialogArchiveFilter => "dialog-archive-filter",
            Self::DialogDataForgeFilter => "dialog-dataforge-filter",
            Self::ColumnName => "column-name",
            Self::ColumnCompression => "column-compression",
            Self::ColumnEncryption => "column-encryption",
            Self::ColumnSize => "column-size",
            Self::SelectAll => "select-all",
            Self::SelectEntry => "select-entry",
            Self::CompressionStore => "compression-store",
            Self::CompressionDeflate => "compression-deflate",
            Self::CompressionZstd => "compression-zstd",
            Self::CompressionOther => "compression-other",
            Self::EncryptedYes => "encrypted-yes",
            Self::EncryptedNo => "encrypted-no",
            Self::ExpandFolder => "expand-folder",
            Self::CollapseFolder => "collapse-folder",
            Self::SearchLabel => "search-label",
            Self::SearchHint => "search-hint",
            Self::DropHint => "drop-hint",
            Self::EmptyTitle => "empty-title",
            Self::DropRelease => "drop-release",
            Self::QuickFilters => "quick-filters",
            Self::CloseDialog => "close-dialog",
            Self::ShortcutReload => "shortcut-reload",
            Self::ShortcutExtractSelected => "shortcut-extract-selected",
            Self::StatusEmpty => "status-empty",
            Self::StatusOpening => "status-opening",
            Self::StatusFailed => "status-failed",
            Self::StatusOpeningStopped => "status-opening-stopped",
            Self::StatusEntries => "status-entries",
            Self::StatusShown => "status-shown",
            Self::StatusSelected => "status-selected",
            Self::StatusSelectedHidden => "status-selected-hidden",
            Self::StatusSearching => "status-searching",
            Self::StatusBuild => "status-build",
            Self::StatusBuildUnknown => "status-build-unknown",
            Self::PreviewTabText => "preview-tab-text",
            Self::PreviewTabHex => "preview-tab-hex",
            Self::PreviewCopy => "preview-copy",
            Self::PreviewNone => "preview-none",
            Self::PreviewLoading => "preview-loading",
            Self::PreviewTooLarge => "preview-too-large",
            Self::PreviewTruncated => "preview-truncated",
            Self::PreviewFailed => "preview-failed",
            Self::PreviewStopped => "preview-stopped",
            Self::PreviewArchiveChanged => "preview-archive-changed",
            Self::ArchiveChanged => "archive-changed",
            Self::ArchiveIssues => "archive-issues",
            Self::GameRunning => "game-running",
            Self::ArchiveTestBuild => "archive-test-build",
            Self::ArchiveTestBuildNda => "archive-test-build-nda",
            Self::ActionReload => "action-reload",
            Self::PreviewKeyRequired => "preview-key-required",
            Self::PreviewKeyRejected => "preview-key-rejected",
            Self::Extraction(text) => text.id(),
            Self::Protection(text) => text.id(),
            Self::Comparison(text) => text.id(),
            Self::DataForge(text) => text.id(),
            Self::ChangeExtraction(text) => text.id(),
            Self::Diff(text) => text.id(),
            Self::RecordChange(text) => text.id(),
            Self::Schema(text) => text.id(),
            Self::Image(text) => text.id(),
            Self::Audio(text) => text.id(),
            Self::Group(text) => text.id(),
            Self::Xml(text) => text.id(),
            Self::Scroll(text) => text.id(),
            Self::Rename(text) => text.id(),
            Self::Incoming(text) => text.id(),
            Self::Query(text) => text.id(),
            Self::Container(text) => text.id(),
            Self::RecordExport(text) => text.id(),
            Self::Catalog(text) => text.id(),
            Self::Record(text) => text.id(),
            Self::RecordLanguage(text) => text.id(),
            Self::Localization(text) => text.id(),
            Self::Tags(text) => text.id(),
            Self::Ui(text) => text.id(),
            Self::Sounds(text) => text.id(),
            Self::ContentSearch(text) => text.id(),
            Self::Model(text) => text.id(),
            Self::ModelInfo(text) => text.id(),
            Self::ModelExport(text) => text.id(),
            Self::FolderConversion(text) => text.id(),
            Self::Entity(text) => text.id(),
            Self::Character(text) => text.id(),
            Self::AssemblyExport(text) => text.id(),
            Self::Key(text) => text.id(),
            Self::About(text) => text.id(),
        }
    }
}

impl From<RecordLanguageText> for Text {
    fn from(text: RecordLanguageText) -> Self {
        Self::RecordLanguage(text)
    }
}

impl From<RecordText> for Text {
    fn from(text: RecordText) -> Self {
        Self::Record(text)
    }
}

impl From<RecordChangeText> for Text {
    fn from(text: RecordChangeText) -> Self {
        Self::RecordChange(text)
    }
}

impl From<SchemaText> for Text {
    fn from(text: SchemaText) -> Self {
        Self::Schema(text)
    }
}

impl From<ComparisonText> for Text {
    fn from(text: ComparisonText) -> Self {
        Self::Comparison(text)
    }
}

impl From<ChangeExtractionText> for Text {
    fn from(text: ChangeExtractionText) -> Self {
        Self::ChangeExtraction(text)
    }
}

impl From<DataForgeText> for Text {
    fn from(text: DataForgeText) -> Self {
        Self::DataForge(text)
    }
}

impl From<AudioText> for Text {
    fn from(text: AudioText) -> Self {
        Self::Audio(text)
    }
}

impl From<ModelText> for Text {
    fn from(text: ModelText) -> Self {
        Self::Model(text)
    }
}

impl From<ImageText> for Text {
    fn from(text: ImageText) -> Self {
        Self::Image(text)
    }
}

impl From<ExtractionText> for Text {
    fn from(text: ExtractionText) -> Self {
        Self::Extraction(text)
    }
}

impl From<DiffText> for Text {
    fn from(text: DiffText) -> Self {
        Self::Diff(text)
    }
}

impl From<IncomingText> for Text {
    fn from(text: IncomingText) -> Self {
        Self::Incoming(text)
    }
}

impl From<ScrollText> for Text {
    fn from(text: ScrollText) -> Self {
        Self::Scroll(text)
    }
}

impl From<GroupText> for Text {
    fn from(text: GroupText) -> Self {
        Self::Group(text)
    }
}

impl From<XmlText> for Text {
    fn from(text: XmlText) -> Self {
        Self::Xml(text)
    }
}

impl From<ContentSearchText> for Text {
    fn from(text: ContentSearchText) -> Self {
        Self::ContentSearch(text)
    }
}
