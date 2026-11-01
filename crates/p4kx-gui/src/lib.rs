// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The desktop window of P4KX.
//!
//! The window state lives in [`Model`], separate from the egui code in [`ui`] that draws it, so
//! tests can drive it headless.

mod app;
mod assembly_export;
mod audio;
mod backends;
mod browser;
mod catalog_export;
mod channel_list;
mod characters;
mod checks;
mod comparison;
mod containers;
mod content_search;
#[cfg(windows)]
pub mod dwm;
mod entities;
mod entry_groups;
mod entry_index;
mod exit_guard;
mod extraction;
mod forge_cache;
mod format;
pub mod frame;
#[cfg(feature = "test-hooks")]
mod hold;
mod installation;
mod language;
mod localization;
mod mismatch;
mod model;
mod model_export;
mod model_info;
mod preview;
mod public_patches;
mod query;
mod read_gate;
mod record_export;
mod record_index;
mod record_language;
mod record_preview;
mod record_search;
mod record_tree;
mod search;
mod shown_line;
pub mod smoke;
mod sort;
mod sounds;
mod tags;
mod text_cache;
mod texts;
pub mod theme;
pub mod ui;
mod ui_file_dialog;
mod ui_tab;
mod viewer;
mod widgets;
pub mod window;

pub use app::App;
pub use assembly_export::{
    ASSEMBLY_LODS, ASSEMBLY_MATERIALS, AssemblyExport, AssemblyExportForm, AssemblyExportProgress,
    AssemblyExportSetup, AssemblyExportStage, AssemblyExportStatus, AssemblyExportSummary,
    InteriorFacts, PackageProblem, PartIssue,
};
pub use audio::{
    AudioOutput, DeviceOutput, FailureReporter, MAX_PREVIEW_PCM, Pcm, Player, SoundSource,
    StreamGuard,
};
pub use browser::{Browser, EntryRow, EntryRowKind, HitLine, TreeNode, TreeRow};
pub use catalog_export::{
    CATALOG_FILE, CatalogExport, CatalogExportProgress, CatalogExportStage, CatalogExportStatus,
    CatalogExportSummary,
};
pub use channel_list::ChannelList;
pub use characters::{
    CHARACTER_LODS, CharacterExport, CharacterExportSetup, CharacterExportStatus,
    CharacterExportSummary, CharactersTab, Chooser, ChooserKind, Expression, HeadInfo, HeadSource,
    Item, Key, Pick, Sources,
};
pub use comparison::{
    ChangeColumn, ChangeSort, ComparisonLevels, ComparisonList, ComparisonSide, ComparisonStage,
    ComparisonStatus, ComparisonTab, ComparisonView, DiffCell, DiffDocument, DiffHunk, DiffRow,
    DiffStatus, DirectorySummary, EntryDiff, FieldName, FieldRow, FieldValue, RecordChangeColumn,
    RecordChangeList, RecordChangeRow, RecordChangeSort, RecordChangeView, RecordDetail,
    RecordDetailStatus, RecordFields, ReportStatus, SchemaRow, SchemaRowKind, SchemaView,
    SideError, Summary,
};
pub use containers::{
    ContainerFailure, ContainerId, ContainerStatus, OPEN_CONTAINERS_BUDGET, Place,
};
pub use content_search::{ContentResult, ContentSearchStatus, ContentSearchTab};
pub use entities::{AssemblyView, VIEW_LOD, ViewFailure, ViewProgress, ViewStage};
pub use entities::{
    EntitiesStatus, EntitiesTab, EntityFilter, EntityList, EntityRole, EntityRow, LoadoutRow,
    LoadoutSession, LoadoutView, ReplaceChooser, ResolveFailure, RowChange,
};
pub use entry_index::{EntryIndex, NoEntries};
pub use extraction::{
    Confirmation, Extraction, ExtractionScope, ExtractionSetup, ExtractionStatus,
    ExtractionSummary, FailedEntry, RecordCounts, TargetKind,
};
#[cfg(feature = "test-hooks")]
pub use hold::Hold;
pub use language::Language;
pub use localization::{LocalizationStatus, LocalizationTab, LocalizationTable};
pub use mismatch::PortMismatch;
pub use model::{Failure, GAME_CHECK_INTERVAL, KeyDialog, KeyProblem, KeyReason, Model, WorkTab};
pub use model_export::{
    AnimationNote, AnimationNoteKind, MODEL_MATERIALS, MODEL_TEXTURE_MIPS, MaterialProblem,
    MissingTexture, ModelExport, ModelExportFailure, ModelExportProgress, ModelExportSetup,
    ModelExportStage, ModelExportStatus, ModelExportSummary,
};
pub use model_info::{
    AnimationCount, ChunkCounts, ChunkFile, ChunkNote, LodInfo, MAX_LISTED_ANIMATIONS,
    MAX_LISTED_CHUNKS, MaterialsInfo, ModelInfo,
};
pub use p4kx_assembly::audio::SoundFormat;
pub use p4kx_assembly::audio::{SourceKind, Unmatched};
pub use p4kx_export::RecordForm;
pub use p4kx_extraction::Language as TextLanguage;
pub use p4kx_extraction::{ArchiveKey, Folder, FolderId};
pub use p4kx_update::{Release, ReleaseCheck};
pub use preview::{
    AudioPreview, AudioStatus, BankSound, ImagePreview, ImageStatus, MAX_PREVIEW_PIXELS, MipStatus,
    PREVIEW_READ_LIMIT, PREVIEW_SIZE_LIMIT, Preview, PreviewContents, PreviewStatus, PreviewTab,
    SoundInfo, SoundStatus, SoundbankPreview, SoundbankStatus, TextureInfo, XML_SIZE_LIMIT,
    XML_TEXT_LIMIT, XmlDocument, XmlPreview, XmlStatus, is_bank_path, is_wem_path,
};
pub use query::{
    CellLook, NameNote, QUERY_PAGE_ROWS, QueryExportFormat, QueryExportStatus, QueryInvalid,
    QueryResult, QueryStage, QueryStatus, QueryTab, ResultColumn, SavedDialog,
};
pub use record_export::{
    FailedRecord, RECORD_FORMS, RecordExport, RecordExportProgress, RecordExportSetup,
    RecordExportStage, RecordExportStatus, RecordExportSummary, RecordScope, RenamedRecord,
    TreeSelection, form_annotates_tags,
};
pub use record_index::{
    DataForgeSummary, RecordId, RecordInfo, RecordNodeId, RecordNodeKind, SUMMARY_RECORDS,
};
pub use record_language::RecordTextChoice;
pub use record_preview::{
    FieldLine, IncomingPreview, IncomingStatus, IncomingView, KeyText, LineValue,
    MAX_INCOMING_LINES, RecordLink, RecordPreview, RecordPreviewStatus, RecordView,
};
pub use record_tree::{DataForgeFailure, RecordNode, RecordRow, RecordTree, RecordTreeStatus};
pub use sort::{Sort, SortColumn, SortDirection};
pub use sounds::{
    BreakLine, EntityInfo, EventLine, IndexInfo, ListSource, ListStatus, MAX_SOUND_HITS, Playable,
    SoundList, SoundsExport, SoundsExportSetup, SoundsExportStatus, SoundsListStatus, SoundsScope,
    SoundsStatus, SoundsTab, TriggerList, TriggerRow, Variant,
};
pub use tags::{TagRow, TagsStatus, TagsTab, TagsView};
pub use texts::{
    AboutText, ArchiveKeyText, AssemblyExportText, AudioText, CatalogText, ChangeExtractionText,
    CharacterText, ComparisonText, ContainerText, ContentSearchText, DataForgeText, DiffText,
    EntityText, ExtractionText, FolderConversionText, GroupText, ImageText, IncomingText,
    LocalizationText, ModelExportText, ModelInfoText, ModelText, QueryText, RecordChangeText,
    RecordExportText, RecordLanguageText, RecordSearchText, RecordText, RenameText, SchemaText,
    ScrollText, SoundsText, TagsText, Text, Texts, UiText, XmlText,
};
pub use ui_tab::{
    ContentRow, Fact, FileList, MAX_CONTENT_ROWS, Note, ReportLine, RowDetail, ShipList, ShipPick,
    UiBackground, UiContents, UiDetail, UiExport, UiExportStatus, UiExportSummary, UiFilesStatus,
    UiFormat, UiMode, UiPower, UiRenderFailure, UiRendered, UiReport, UiScreensStatus, UiSettings,
    UiSize, UiSource, UiTab,
};
pub use viewer::{
    AssemblyScene, BuildError, BuildOptions, DrawList, GroupDraw, Hooks, Instance, Left,
    MAX_VIEW_ITEMS, MAX_VIEW_TEXTURE_BYTES, MAX_VIEW_TRIANGLES, MAX_VIEW_VERTICES, MIN_PART_PIXELS,
    PartInfo, Progress, VIEW_TEXTURE_MIP,
};
pub use viewer::{
    Bounds, Camera, Control, GpuReport, MAX_SCENE_TRIANGLES, MAX_SCENE_VERTICES,
    MAX_SKELETON_BONES, MAX_TEXTURE_SIDE, MODEL_FILE_LIMIT, Matrix, ModelPreview, ModelStatus,
    MorphSpan, PREVIEW_ANIMATION_BOUNDS, Playback, Pose, Problem, Rejected, SKIN_STRIDE, Scene,
    SceneError, Skeleton, SkinVertex, Step, Unavailable, Uniforms, Vertex, ViewerSupport,
    check_buffer, check_texture, install as install_viewer, is_model_path, target_extent,
};
