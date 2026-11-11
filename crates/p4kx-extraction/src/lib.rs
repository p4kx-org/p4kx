// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Extraction service of P4KX and the Archive, `CryXml`, DataForge, localisation,
//! texture, audio, chunk file, model and UI movie APIs front-ends use.

mod animation_listing;
mod animation_load;
mod animation_select;
mod animation_set;
mod animation_sources;
mod archive_byte_range;
mod archive_cause;
mod archive_layout_info;
#[cfg(test)]
mod archive_layout_info_tests;
mod archive_layout_v2;
mod archive_layout_zip;
mod archive_material_reference;
mod archive_metadata_version;
mod archive_region_info;
mod archive_table_declaration;
pub mod bank;
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
mod dataforge_file_origin;
mod dataforge_origin;
mod dataforge_pick;
mod dataforge_source;
mod dds_raw_info;
mod dds_source_info;
mod decimal_u64;
mod dependent;
mod disk_case;
mod disk_parts;
mod disk_textures;
mod entry_content;
mod entry_metadata_info;
mod entry_path;
mod entry_payload;
mod entry_record_format_info;
mod entry_tree;
mod exclusion;
mod extract;
mod extract_outcome;
mod field_origin;
mod field_origin_seed;
mod field_origins;
mod field_origins_seed;
#[cfg(test)]
mod field_origins_tests;
mod field_path;
mod field_path_seed;
mod file_receipt;
mod filter;
mod folder_cache;
mod folder_convert;
mod folder_source;
mod guard_cancellation;
pub mod hirc_select;
pub mod hirc_view;
mod loaded_dataforge;
mod localization;
mod localization_set;
mod material_source_info;
mod material_source_mapping;
mod material_source_owned;
mod material_xml_info;
mod model_chunk_source_info;
mod model_companions;
pub mod model_info;
mod model_info_animations;
mod model_info_materials;
mod model_observed;
mod model_source;
mod model_source_glb;
mod model_textures;
mod observed_file_origin;
mod options;
mod origin_artifact;
mod origin_companion;
mod origin_file_io;
#[cfg(test)]
mod origin_reader_admission_tests;
mod origin_reader_limits;
#[cfg(test)]
mod origin_reader_tests;
mod output_cleanup;
mod output_creation;
mod output_file;
mod owned_output_failure;
mod part_escape;
mod path_index;
mod path_index_build_error;
mod path_index_error;
mod path_key_sort;
mod portable_name;
mod projection_schema;
mod query_export;
mod record_combined;
mod record_existing;
mod record_export;
mod record_files;
mod record_format;
mod record_plan;
mod record_report;
mod report;
mod resolved_texts;
mod selection;
pub mod sound;
mod sound_convert;
mod sound_job;
mod sound_output;
mod sound_workers;
mod soundbank_convert;
mod source;
#[cfg(test)]
mod source_ancestor_tests;
mod source_archive;
mod source_archive_binding;
mod source_details;
mod source_encryption_evidence;
mod source_evidence_error;
mod source_file_input;
mod source_inventory;
mod source_protection;
mod source_provenance;
mod source_registration;
mod source_registry;
mod source_scope;
mod source_use;
mod source_use_wire;
mod source_uses;
mod source_value;
mod source_value_schema;
mod target;
pub mod text_line;
mod text_window;
mod texture_conversion_error;
mod texture_convert;
mod texture_exr;
mod texture_grey;
mod texture_groups;
mod texture_header;
mod texture_image_plan;
mod texture_images;
mod texture_job;
mod texture_member_keys;
mod texture_names;
mod texture_outputs;
mod texture_parts;
mod texture_png;
pub mod texture_preview;
mod texture_preview_png;
mod texture_siblings;
mod textures;
mod tracker;
mod ui_disk_path;
mod ui_external;
mod ui_images;
mod ui_import_match;
mod ui_imports;
mod ui_render;
mod ui_render_bitmaps;
mod ui_screen_texts;
mod ui_service;
mod ui_svg;
mod verify;
mod watch;
mod write;
mod write_compare;
mod written_paths;
mod zip_header_info;

pub use animation_listing::AnimationListing;
pub use animation_set::{
    AnimationBounds, AnimationChoice, AnimationSet, MAX_ANIMATION_FILE_LEN, UnreadAnimation,
};
pub use archive_byte_range::ArchiveByteRange;
pub use archive_cause::archive_error_in;
pub use archive_layout_info::{ArchiveDeclarationState, ArchiveLayout, ArchiveLayoutInfo};
pub use archive_layout_v2::ArchiveV2Layout;
pub use archive_layout_zip::{
    ArchiveCigInstallation, ArchiveZip64End, ArchiveZip64Locator, ArchiveZipEnd, ArchiveZipLayout,
};
pub use archive_metadata_version::ArchiveMetadataVersion;
pub use archive_region_info::{ArchiveRegion, ArchiveRegionState};
pub use archive_table_declaration::ArchiveTableDeclaration;
pub use bank::{Bank, BankExtraction, MAX_SOUNDBANK_LEN, WemFailure, WemFormat, WrittenWem};
pub use chunk_source::{ChunkSource, ChunkTextError, MAX_CHUNK_FILE_LEN};
pub use conversion_error::ConversionError;
pub use coverage::{
    ComponentCoverage, CoverageReport, EntryCoverage, ObjectCoverage, ReferenceError,
    ReferenceItem, ReferenceKind, ReferenceList, SampleExport, TypeCoverage,
};
pub use dataforge::{DATAFORGE_ENTRIES, DataForgeEntry, DataForgeLoadError, MAX_DATAFORGE_SIZE};
pub use dataforge_file::is_dataforge_file;
pub use dataforge_file_origin::DataForgeFileOrigin;
pub use dataforge_origin::DataForgeOrigin;
pub use dataforge_source::DataForgeSource;
pub use dds_raw_info::{DdsDx10Info, DdsLegacyInfo, DdsPixelFormatInfo};
pub use dds_source_info::{
    DDS_SOURCE_INFO_VERSION, DdsBlockLayoutInfo, DdsNormalizedInfo, DdsRawHeaderInfo, DdsSourceInfo,
};
pub use decimal_u64::DecimalU64;
pub use disk_textures::DiskTextures;
pub use entry_content::{
    EntryBytes, EntryReadError, EntryText, MIN_TEXT_LEN, TextEncoding, find_entry, read_bytes,
    read_text,
};
pub use entry_metadata_info::{DosCalendarInfo, DosTimestampInfo, EntryMetadataInfo};
pub use entry_payload::extract_entry_payload;
pub use entry_record_format_info::EntryRecordFormatInfo;
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
pub use material_source_info::{
    MaterialAuthoredInfo, MaterialEncodingInfo, MaterialEntryDispositionInfo,
    MaterialLayerSourceInfo, MaterialLibrarySourceInfo, MaterialSourceEntryInfo,
    MaterialSourceLossInfo, MaterialSourceLossInfoKind, MaterialTextureSourceInfo,
};
pub use material_xml_info::{
    MaterialAttributeInfo, MaterialAttributeKindInfo, MaterialContentInfo, MaterialElementInfo,
    MaterialElementSyntaxInfo, MaterialParameterInfo,
};
pub use model_chunk_source_info::{
    MODEL_CHUNK_SOURCE_INFO_VERSION, ModelChunkLayoutInfo, ModelChunkSourceInfo, ModelChunkSpanInfo,
};
pub use model_info::{BoneInfo, ChunkInfo, ChunkJson, LodInfo, MeshInfo, ModelInfo, NodeInfo};
pub use model_info_animations::{AnimationInfo, UnreadInfo};
pub use model_info_materials::{LibraryInfo, MaterialInfo};
pub use model_source::ModelSource;
pub use model_textures::{ArchiveTextures, MAX_MODEL_TEXTURE_PIXELS};
pub use observed_file_origin::{ObservedFileOrigin, ObservedFileRead, read_observed_file};
pub use options::{
    AudioConversion, CancelToken, ConflictPolicy, ExtractOptions, Progress, TextureConversion,
};
pub use output_file::{
    Replaced, check_protected_output, write_bytes_replacing, write_replacing,
    write_replacing_counted, write_replacing_counted_protected, write_replacing_protected,
};
pub use owned_output_failure::owned_output_failure;
pub use p4kx_archive::{
    Archive, ArchiveError, ArchiveInputOrigin, Build, BuildError, BuildManifestObservation,
    BuildManifestOrigin, BuildManifestOutcome, Checksum, Compression, DemandScope, DosCalendar,
    DosTimestamp, EffectiveEncoding, Entry, EntryIssue, EntryMetadata, EntryOrigin,
    EntryOriginStep, EntryReader, EntryRecordFormat, InstallationStatus, KeyCheck, KeyDemand,
    KeyDemandSnapshot, MAX_COMPRESSION_RATIO, MAX_CONTAINER_DEPTH, MAX_CONTAINER_ENTRIES,
    MAX_CONTAINER_LEN, MAX_RECOVERED_ENTRIES, MAX_RECOVERY_SCAN, ObservedEntry,
    ObservedEntryBinding, OpenOptions, SourceEncryption, UnreadEntries, ZipHeaderMetadata,
};
pub use p4kx_archive::{
    ArchiveLayoutKind, ArchiveMetadata, ArchiveMetadataStatus, AvailableMetadataSpan,
    CigInstallMetadata, MAX_METADATA_CHUNK, MetadataAccess, MetadataCost, MetadataExtra,
    MetadataExtraStatus, MetadataFreeStatus, MetadataFreeValue, MetadataInstallValue,
    MetadataProgressBound, MetadataRange, MetadataReadError, MetadataRecord, MetadataRegion,
    MetadataRegionStatus, MetadataReservedWord, MetadataSection, MetadataSpan,
    MetadataTableProblem, MetadataTableRow, MetadataTableValue, MetadataTables, MetadataView,
    V2DirectoryMetadata, Zip64EndMetadata, Zip64LocatorMetadata, ZipDirectoryMetadata,
    ZipEndMetadata,
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
    Annotations, CSV_BYTE_ORDER_MARK, CanonicalReadObserver, DataForge, DataForgeError,
    DataForgeIssue, DataType, Enum, FLAT_COLUMNS, FLAT_TAG_COLUMN, FLAT_TEXT_COLUMN, Field,
    FieldKind, FieldStat, FieldStatistics, FieldStatisticsError, FieldStorage, FileGraph,
    FlatFormat, FlatValues, FormatSurvey, Guid, IndexedReference, IndexedReferenceOrigin,
    InvalidGuid, JsonLayout, Location, MAX_DATAFORGE_ISSUES, MAX_DISTINCT_VALUES, MAX_FIELD_PATHS,
    MAX_INDEXED_REFERENCES, Number, ObservedFileEdge, ObservedFileGraph, ObservedReferenceIndex,
    PointerProbe, PointerSurvey, Property, RECORD_XML_NAMESPACE, Record, RecordJson,
    RecordMetadata, RecordText, RecordTree, RecordXml, Reference, ReferenceIndex,
    ReferenceIndexError, ReferenceOrigin, ReferenceScanGap, Resolution, Step, Struct, StructValue,
    TAG_DATABASE_TYPE, TAG_PATH_SEPARATOR, TAG_TYPE, Tag, TagBuildEvent, TagPath, TagPathEdgeKind,
    TagPathIncoming, TagRelationIssue, TagResolver, TagTree, TagTreeIssues, TextResolver,
    TrailingSurvey, TreeBuilder, Value, ValueError, ValueKind, write_csv_cell,
};
#[expect(
    deprecated,
    reason = "Public compatibility for the released Tag constants"
)]
pub use p4kx_dataforge::{
    MAX_TAG_DEPTH, MAX_TAG_NAME_BYTES, MAX_TAG_PATH_BYTES, MAX_TAG_TEXT_BYTES,
};
pub use p4kx_foundation::{ArchiveKey, ArchiveKeyError};
pub use p4kx_gfx as gfx;
pub use p4kx_gltf::{
    AnimationSummary, GlbOptions, GlbSummary, GltfError, LoadedTexture, MAX_TEXTURE_IMAGES,
    MAX_TEXTURE_PIXELS, MaterialMode, TextureFallback, TextureLoadError, TextureLoader,
    TextureRole, base_colour_factor, diffuse_texture_path, draws_layer_diffuse, layered_base_key,
    layered_base_texture, layered_base_textures, texture_path,
};
pub use p4kx_guard::{
    ArchiveWriteProtection, CreatedOutputFile, Installation, OwnedDirectoryPermit, OwnedFileToken,
    OwnedOutputCleanupError, OwnedOutputError, RuntimeEnvironment, RuntimePrerequisiteError,
    WriteProtectionError, WriteRefused, check_runtime_environment, check_runtime_prerequisites,
    configure_library_discovery, create_output_directory, create_output_file, ensure_writable,
    game_is_running, protect, protect_archive_folder, protect_archive_folder_report,
    protect_libraries, protect_libraries_report, protect_manual, remove_owned_output,
    remove_owned_output_target_only, validated_output_directory,
};
pub use p4kx_localization::{
    GlobalIni, IniEncoding, IniText, Language, LocalizationError, Lookup, MAX_GLOBAL_INI_KEYS,
    MAX_GLOBAL_INI_SIZE, OwnedTextSet, PLACEHOLDER_KEYS, ParseLanguageError, TextSet,
    decode_line_breaks, display_text, is_placeholder_key,
};
pub use p4kx_model as model;
pub use p4kx_texture::MAX_HEADER_LEN as MAX_DDS_HEADER_LEN;
pub use p4kx_texture::{
    BlockLayout, DdsDx10Fields, DdsHeader, DdsHeaderField, DdsLegacyFields, DdsPixelFormatFields,
    DdsRawHeader, DxgiFormat, HdrImage, MergeReport, Mip, RgbaImage, TextureError, TextureFormat,
};
pub use part_escape::escape_part;
pub use path_index::{PathIndex, PathKeys};
pub use path_index_build_error::PathIndexBuildError;
pub use path_index_error::PathIndexError;
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
pub use source_provenance::*;
pub use target::{
    create_default_target, create_default_target_protected, default_target_name, utc_rfc3339,
};
pub use texture_exr::ExrReport;
pub use texture_grey::{TextureVerdicts, probe_saturation};
pub use texture_groups::{TextureGroup, TextureGroups};
pub use texture_image_plan::{
    TextureImage, TextureImageFace, TextureImageFormat, TextureImagePlan,
};
pub use texture_job::{Member as TextureMember, Role as TextureMemberRole, role as texture_role};
pub use texture_member_keys::{
    TextureMemberCandidate, TextureMemberIndex, TextureMemberKeys, TextureMemberRelation,
};
pub use texture_png::{CUBE_FACE_NAMES, CubemapLayout, PngReport};
pub use textures::{
    ObservedPreviewPng, PreviewPng, PreviewPngEvidence, PreviewTextureEvidence, Texture,
};
use ui_service::{ui_jobs, ui_movie};

pub use ui_images::{ArchiveImages, MAX_SCREEN_TEXTURE_PIXELS};
pub use ui_imports::{
    ImportIssue, ImportReport, MAX_IMPORT_BYTES, MAX_IMPORT_DEPTH, MAX_IMPORTED_FONTS,
    MAX_IMPORTED_MOVIES, UnresolvedImport,
};
pub use ui_render::{MAX_UI_RENDER_BITMAP_PIXELS, UiBitmapFailure, UiRender};
pub use ui_screen_texts::ScreenTexts;
pub use ui_service::*;
pub use ui_svg::UiSvg;
pub use verify::{VerificationReport, VerifyOptions, verify};
pub use watch::{ArchiveChanged, ArchiveWatch, DEFAULT_WATCH_INTERVAL, FileStamp};
pub use zip_header_info::ZipHeaderInfo;
