// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Extraction service of P4KX and the Archive, `CryXml`, DataForge, localisation,
//! texture, audio, chunk file, model and UI movie APIs front-ends use.

mod animation_listing;
mod animation_load;
mod animation_select;
mod animation_set;
mod animation_sources;
mod archive_cause;
mod bank;
mod budget;
mod buffers;
mod chunk_source;
mod conversion_error;
mod convert;
mod counting;
mod coverage;
mod coverage_entries;
mod coverage_forge;
mod coverage_objects;
mod coverage_run;
mod dataforge;
mod dataforge_convert;
mod dataforge_file;
mod dataforge_pick;
mod dataforge_source;
mod dependent;
mod disk_case;
mod entry_content;
mod entry_path;
mod entry_tree;
mod exclusion;
mod extract;
mod filter;
mod folder_cache;
mod folder_convert;
mod folder_source;
pub mod hirc_select;
pub mod hirc_view;
mod hirc_view_build;
mod loaded_dataforge;
mod localization;
mod localization_set;
mod model_companions;
pub mod model_info;
mod model_info_animations;
mod model_info_materials;
mod model_observed;
mod model_source;
mod model_source_glb;
mod model_textures;
mod options;
mod output_file;
mod part_escape;
mod path_index;
mod portable_name;
mod query_export;
mod record_combined;
mod record_export;
mod record_files;
mod record_format;
mod record_plan;
mod record_report;
mod report;
mod resolved_texts;
mod selection;
mod sound;
mod sound_convert;
mod sound_job;
mod sound_output;
mod sound_workers;
mod soundbank_convert;
mod source;
mod target;
pub mod text_line;
mod text_window;
mod texture;
mod texture_convert;
mod texture_exr;
mod texture_grey;
mod texture_groups;
mod texture_images;
mod texture_job;
mod texture_names;
mod texture_outputs;
mod texture_parts;
mod texture_png;
mod texture_preview;
mod texture_preview_png;
mod texture_siblings;
mod tracker;
mod ui_disk_path;
mod ui_external;
mod ui_extract;
mod ui_images;
mod ui_import_match;
mod ui_imports;
mod ui_jobs;
mod ui_movie;
mod ui_render;
mod ui_render_bitmaps;
mod ui_screen_texts;
mod ui_svg;
mod verify;
mod watch;
mod write;
mod write_compare;
mod written_paths;

pub use animation_listing::AnimationListing;
pub use animation_set::{
    AnimationBounds, AnimationChoice, AnimationSet, MAX_ANIMATION_FILE_LEN, UnreadAnimation,
};
pub use archive_cause::archive_error_in;
pub use bank::{Bank, BankExtraction, MAX_SOUNDBANK_LEN, WemFailure, WemFormat, WrittenWem};
pub use chunk_source::{ChunkSource, ChunkTextError, MAX_CHUNK_FILE_LEN};
pub use conversion_error::ConversionError;
pub use coverage::{
    ComponentCoverage, CoverageReport, EntryCoverage, ObjectCoverage, ReferenceError,
    ReferenceItem, ReferenceKind, ReferenceList, SampleExport, TypeCoverage,
};
pub use dataforge::{DATAFORGE_ENTRIES, DataForgeEntry, DataForgeLoadError, MAX_DATAFORGE_SIZE};
pub use dataforge_file::is_dataforge_file;
pub use dataforge_source::DataForgeSource;
pub use entry_content::{
    EntryBytes, EntryReadError, EntryText, MIN_TEXT_LEN, TextEncoding, find_entry, read_bytes,
    read_text,
};
pub use entry_tree::{EntryTree, Folder, FolderId};
pub use exclusion::{Exclusion, LeftOut, MAX_EXCLUSIONS};
pub use extract::extract;
pub use filter::{Filter, FilterError, MAX_FILTER_LEN, MAX_FILTER_WILDCARDS};
pub use folder_convert::convert_folder;
pub use folder_source::{
    FolderError, FolderFileError, FolderSelection, FolderSource, MAX_FOLDER_DEPTH,
    MAX_FOLDER_ENTRIES,
};
pub use loaded_dataforge::{DataForgeStopped, LoadedDataForge};
pub use localization::{LocalizationEntry, LocalizationLoadError};
pub use localization_set::{LocalizationNote, LocalizationSet};
pub use model_info::{BoneInfo, ChunkInfo, ChunkJson, LodInfo, MeshInfo, ModelInfo, NodeInfo};
pub use model_info_animations::{AnimationInfo, UnreadInfo};
pub use model_info_materials::{LibraryInfo, MaterialInfo};
pub use model_source::ModelSource;
pub use model_textures::{ArchiveTextures, MAX_MODEL_TEXTURE_PIXELS};
pub use options::{
    AudioConversion, CancelToken, ConflictPolicy, ExtractOptions, Progress, TextureConversion,
};
pub use output_file::{Replaced, write_bytes_replacing, write_replacing, write_replacing_counted};
pub use p4kx_archive::{
    Archive, ArchiveError, Build, BuildError, Checksum, Compression, Entry, EntryIssue,
    EntryReader, KeyCheck, MAX_COMPRESSION_RATIO, MAX_CONTAINER_DEPTH, MAX_CONTAINER_ENTRIES,
    MAX_CONTAINER_LEN, MAX_RECOVERED_ENTRIES, MAX_RECOVERY_SCAN, OpenOptions,
};
pub use p4kx_audio::{
    ActionTarget, BankError, BankHeader, BankWarning, Container, ContainerKind, EmbeddedWem,
    HircKind, HircLayout, HircObject, HircValue, MediaStorage, MusicClip, MusicGroupKind,
    MusicMarker, MusicMeter, MusicNode, MusicPlayMode, MusicPlaylist, MusicPlaylistItem,
    MusicSegment, MusicStinger, MusicSwitch, MusicTrack, MusicTrackKind, MusicTrackSwitch,
    PlayBank, Section, SectionId, SoundSource, Soundbank,
};
pub use p4kx_audio::{
    AudioError, ByteOrder, Codebooks, Codec, MediaClass, OggReport, OpusError, VorbisError,
    WavReport, Wem, WemChunk,
};
pub use p4kx_chunks as chunks;
pub use p4kx_cryxml as cryxml;
pub use p4kx_dataforge::query;
pub use p4kx_dataforge::{
    Annotations, CSV_BYTE_ORDER_MARK, DataForge, DataForgeError, DataForgeIssue, DataType, Enum,
    FLAT_COLUMNS, FLAT_TAG_COLUMN, FLAT_TEXT_COLUMN, Field, FieldKind, FieldStat, FieldStatistics,
    FieldStatisticsError, FileGraph, FlatFormat, FlatValues, FormatSurvey, Guid, IndexedReference,
    InvalidGuid, JsonLayout, MAX_DATAFORGE_ISSUES, MAX_DISTINCT_VALUES, MAX_FIELD_PATHS,
    MAX_INDEXED_REFERENCES, MAX_NESTING_DEPTH, MAX_RECORD_VALUES, MAX_TAG_DEPTH,
    MAX_TAG_NAME_BYTES, MAX_TAG_PATH_BYTES, MAX_TAG_TEXT_BYTES, Number, PointerProbe,
    PointerSurvey, Property, RECORD_XML_NAMESPACE, Record, RecordJson, RecordTree, RecordXml,
    Reference, ReferenceIndex, Resolution, Struct, StructValue, TAG_DATABASE_TYPE,
    TAG_PATH_SEPARATOR, TAG_TYPE, Tag, TagResolver, TagTree, TagTreeIssues, TextResolver,
    TrailingSurvey, TreeBuilder, Value, ValueError, ValueKind, write_csv_cell,
};
pub use p4kx_foundation::{ArchiveKey, ArchiveKeyError};
pub use p4kx_gfx as gfx;
pub use p4kx_gltf::{
    AnimationSummary, GlbOptions, GlbSummary, GltfError, LoadedTexture, MAX_TEXTURE_IMAGES,
    MAX_TEXTURE_PIXELS, MaterialMode, TextureFallback, TextureLoadError, TextureLoader,
    TextureRole, base_colour_factor, diffuse_texture_path, draws_layer_diffuse, layered_base_key,
    layered_base_texture, layered_base_textures,
};
pub use p4kx_guard::{
    Installation, WriteRefused, ensure_writable, game_is_running, protect, protect_archive_folder,
    protect_libraries, protect_manual,
};
pub use p4kx_localization::{
    GlobalIni, IniEncoding, IniText, Language, LocalizationError, Lookup, MAX_GLOBAL_INI_KEYS,
    MAX_GLOBAL_INI_SIZE, OwnedTextSet, PLACEHOLDER_KEYS, ParseLanguageError, TextSet,
    decode_line_breaks, display_text, is_placeholder_key,
};
pub use p4kx_model as model;
pub use p4kx_texture::{
    BlockLayout, DdsHeader, DxgiFormat, HdrImage, MergeReport, Mip, RgbaImage, TextureError,
    TextureFormat,
};
pub use part_escape::escape_part;
pub use path_index::{PathIndex, PathKeys};
pub use portable_name::{
    MAX_PART_BYTES, NameRule, PathLimit, check_part, check_path_length, fold_name,
};
pub use query_export::{
    QUERY_PAGE_SIZE, QueryExport, QueryExportError, QueryExportOptions, export_query, is_same_file,
};
pub use record_combined::export_combined;
pub use record_export::{export_planned_record_files, export_record_files, export_records};
pub use record_format::RecordFileFormat;
pub use record_plan::RecordFilePlan;
pub use record_report::{
    RecordExportError, RecordExportOptions, RecordExportReport, RecordFailure, RecordRenamed,
    RenameCause,
};
pub use report::{EntryFailure, ExtractionError, ExtractionReport};
pub use resolved_texts::LanguageTexts;
pub use selection::{UnknownEntries, select_entries};
pub use sound::{MAX_SOUND_LEN, Sound};
pub use target::{create_default_target, default_target_name, utc_rfc3339};
pub use texture::Texture;
pub use texture_exr::ExrReport;
pub use texture_grey::{TextureVerdicts, probe_saturation};
pub use texture_groups::{TextureGroup, TextureGroups};
pub use texture_png::{CUBE_FACE_NAMES, CubemapLayout, PngReport};
pub use texture_preview_png::PreviewPng;
pub use ui_extract::{UiAssetFailure, UiAssetKind, UiExtraction, WrittenUiAsset};
pub use ui_images::{ArchiveImages, MAX_SCREEN_TEXTURE_PIXELS};
pub use ui_imports::{
    ImportIssue, ImportReport, MAX_IMPORT_BYTES, MAX_IMPORT_DEPTH, MAX_IMPORTED_FONTS,
    MAX_IMPORTED_MOVIES, UnresolvedImport,
};
pub use ui_movie::{MAX_UI_MOVIE_LEN, UiMovie};
pub use ui_render::{MAX_UI_RENDER_BITMAP_PIXELS, UiBitmapFailure, UiRender};
pub use ui_screen_texts::ScreenTexts;
pub use ui_svg::UiSvg;
pub use verify::{VerificationReport, VerifyOptions, verify};
pub use watch::{ArchiveChanged, ArchiveWatch, DEFAULT_WATCH_INTERVAL, FileStamp};
