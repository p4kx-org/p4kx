// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded media export with physical identities and atomic generation publication.

mod archive_extras_read;
mod archive_layout_consistency_schema;
mod archive_layout_projection;
mod archive_layout_schema;
mod archive_layout_status_schema;
mod archive_metadata_diagnostic;
mod archive_metadata_schema;
mod archive_provider;
mod archive_raw;
mod archive_record;
mod archive_records_read;
mod archive_retention;
mod archive_section;
mod archive_sections_read;
mod archive_table;
mod archive_tables_read;
mod artifact;
mod artifact_extension;
mod artifact_placement;
mod audio_bank_failure;
mod audio_bank_hirc;
mod audio_bank_hirc_action;
mod audio_bank_hirc_container;
mod audio_bank_hirc_event;
mod audio_bank_hirc_origins;
mod audio_bank_hirc_sound;
mod audio_bank_inventory;
mod audio_bank_media;
mod audio_bank_media_origins;
mod audio_bank_music_playlist;
mod audio_bank_music_segment;
mod audio_bank_music_switch;
mod audio_bank_music_track;
mod audio_bank_music_transition_origins;
mod audio_bank_music_value;
mod audio_bank_origins;
mod audio_bank_payload;
mod audio_bank_projection;
mod audio_bank_sound_node_origins;
mod audio_bank_sound_origins;
mod audio_bank_sound_payload;
mod audio_bank_source;
mod audio_bank_stid;
mod audio_bank_stid_origins;
mod audio_bank_stid_payload;
mod audio_chunk_origins;
mod audio_payload;
mod audio_provider;
mod audio_record_origins;
mod audio_schema;
mod audio_schema_status;
mod audio_source_language;
mod audio_wem_annotation;
mod audio_wem_annotation_origins;
mod audio_wem_annotation_payload;
mod audio_wem_cue;
mod audio_wem_cue_origins;
mod audio_wem_diagnostic;
mod audio_wem_list;
mod audio_wem_list_origins;
mod audio_wem_list_payload;
mod audio_wem_origins;
mod audio_wem_projection;
mod audio_wem_text_origins;
mod build_projection;
mod build_value_read;
mod checkpoint;
mod chunk;
mod chunk_copy;
mod contract;
mod converted_artifact;
mod converted_sink;
mod directory_context;
mod directory_index;
mod directory_inputs;
mod directory_issue;
mod directory_lookup;
mod directory_run;
mod error;
mod export;
mod export_run;
mod finite_serializer;
mod generation;
mod generation_lock;
#[cfg(test)]
mod generation_tests;
mod generation_validation;
mod hashing;
mod hirc_object_index;
mod identifier;
mod index_destination;
mod index_file;
mod input_budget;
mod input_lookup;
mod input_receipt;
mod integrity;
mod json_output;
mod kind;
mod limits;
mod lock_namespace;
mod lookup_work;
#[cfg(test)]
mod material_observer_support;
#[cfg(test)]
mod material_observer_tests;
mod material_payload;
mod material_projection;
mod material_provider;
mod material_read_origins;
mod material_reference;
mod material_reference_origins;
mod material_schema;
mod material_shader_origins;
mod material_shader_reference;
mod material_shader_tokens;
mod material_surface;
mod media_record;
mod media_writer;
mod metadata_input;
#[cfg(test)]
mod metadata_input_tests;
mod numeric;
mod observed_write;
mod origin_addresses;
mod origin_aggregate;
#[cfg(test)]
mod origin_aggregate_support;
mod origin_archive;
#[cfg(test)]
mod origin_atomic_tests;
mod origin_build;
mod origin_build_map;
mod origin_capture;
#[cfg(test)]
mod origin_capture_tests;
mod origin_contract;
mod origin_contributions;
mod origin_coverage_read;
mod origin_data;
mod origin_decimal;
#[cfg(test)]
mod origin_deep_tests;
#[cfg(test)]
mod origin_depth_tests;
mod origin_error;
mod origin_fact_read;
mod origin_index;
mod origin_index_build_read;
#[cfg(test)]
mod origin_index_build_tests;
#[cfg(test)]
mod origin_index_test_support;
mod origin_input;
mod origin_inventory_read;
#[cfg(test)]
mod origin_inventory_tests;
mod origin_inventory_wire;
mod origin_json;
mod origin_line_reader;
mod origin_lookup;
mod origin_material;
mod origin_material_profile;
#[cfg(test)]
mod origin_material_tests;
mod origin_observer;
mod origin_payload_read;
#[cfg(test)]
mod origin_pipeline_tests;
mod origin_prepared;
mod origin_prepared_source;
#[cfg(test)]
mod origin_profile_tests;
mod origin_read_policy;
#[cfg(test)]
mod origin_read_support;
#[cfg(test)]
mod origin_record_aggregate_tests;
#[cfg(test)]
mod origin_record_budget_tests;
mod origin_reference;
mod origin_resolver;
mod origin_rows_read;
#[cfg(test)]
mod origin_shader_failure_tests;
mod origin_source;
#[cfg(test)]
mod origin_source_aggregate_tests;
mod origin_source_facts;
mod origin_source_identity;
mod origin_source_shape;
mod origin_state;
mod origin_stream;
mod origin_subject;
#[cfg(test)]
mod origin_tamper_tests;
#[cfg(test)]
mod origin_test_support;
mod origin_value_read;
mod origin_verify;
mod output_operation;
mod output_path;
#[cfg(test)]
mod owned_output_tests;
mod provider;
mod publication;
mod raw_output;
mod reference;
mod reference_resolve;
mod required_input;
#[cfg(test)]
mod required_input_fault_support;
mod required_reader;
mod retained_payload;
mod retained_source;
mod root_source_read;
mod schema;
mod schema_merge;
#[cfg(test)]
mod schema_merge_tests;
mod shader_declaration;
mod shader_diagnostic;
mod shader_flags;
mod shader_index;
mod shader_input;
mod shader_lookup;
mod shader_lookup_flags;
mod shader_mask;
mod shader_origins;
mod shader_payload;
mod shader_projection;
mod shader_provider;
mod shader_read_origins;
mod shader_schema;
mod shader_source;
mod shader_span;
mod shader_values;
mod shader_word;
mod source;
mod source_selection;
mod staged_directory;
mod temporary_file;
mod traversal;
mod traversal_container;
mod traversal_projection;
mod traversal_visit;
#[cfg(test)]
mod write_context_tests;
mod writer_build_origins;
mod writer_manifest;
mod writer_origins;
mod writer_record;

pub use archive_metadata_diagnostic::{ArchiveMetadataDiagnostic, ArchiveMetadataFailure};
pub use archive_provider::ArchiveProvider;
pub use archive_raw::{ArchiveRawChunk, ArchiveRawKey, ArchiveRawSpan};
pub use archive_record::{ArchiveDirectoryRecord, ArchiveExtraData, ArchiveExtraState};
pub use archive_section::{ArchiveSection, ArchiveSectionData, ArchiveSectionPresence};
pub use archive_table::{
    ArchiveFreeRange, ArchiveFreeState, ArchiveInstallation, ArchiveProgressBound,
    ArchiveReservedState,
};
pub use artifact_extension::ArtifactExtension;
pub use chunk::{
    ByteOrder, ChunkLayout, ChunkReference, DataOrder, DataType, OffsetBasis, SourceRange,
};
pub use contract::{
    Artifact, Counts, Index, KindSummary, MediaIssue, MediaRecord, Severity, Status, Tally,
};
pub use directory_context::DirectoryContext;
pub use directory_issue::DirectoryIssue;
pub use directory_lookup::DirectoryLookup;
pub use error::{ExtractionCounts, MediaError};
pub use export::{AudioOutput, ExportOptions, ExportOutcome, export, export_with};
pub use identifier::{IntegerWidth, OriginalIdentifier};
pub use input_lookup::{EntryInput, InputLookup};
pub use input_receipt::{InputOmission, InputReceipt};
pub use integrity::{Algorithm, ByteScope, Verification};
pub use kind::MediaKind;
pub use limits::Limits;
pub use material_payload::{
    MaterialFailureReason, MaterialLibraryMetadata, MaterialMetadata, MaterialPayload,
    MaterialProjectionFailure,
};
pub use material_provider::MaterialProvider;
pub use material_reference::{MaterialReference, MaterialReferenceRole};
pub use material_schema::{material_payload_schema, material_record_schema};
pub use material_shader_reference::{MaterialShaderReference, MaterialShaderRole};
pub use material_surface::{MaterialPalette, MaterialSurface, MaterialSurfaceBasis};
pub use media_writer::MediaWriter;
pub use numeric::{Float32, Float64, ValueOrigin};
pub use origin_contract::{
    OriginCoverage, OriginProfile, OriginProtocol, OriginReceipts, OriginScope, OriginStream,
    OriginStreamSummary,
};
pub use p4kx_extraction::{
    ArchiveByteRange, ArchiveCigInstallation, ArchiveDeclarationState, ArchiveLayout,
    ArchiveLayoutInfo as ArchiveLayoutData, ArchiveMetadataVersion, ArchiveRegion,
    ArchiveRegionState, ArchiveTableDeclaration, ArchiveV2Layout, ArchiveZip64End,
    ArchiveZip64Locator, ArchiveZipEnd, ArchiveZipLayout,
};
pub use provider::{Availability, Capability, FieldGroup, Projection, Provider, ProviderRegistry};
pub use reference::{
    CandidateInventory, CandidateState, NotSelectedReason, Reference, ReferenceCandidate,
    ReferenceResolution, ReferenceTarget, ReferenceUnavailable,
};
pub use required_input::RequiredInput;
pub use schema::{
    MediaFormat, MediaVersion, builtin_record_schema, index_schema, record_schema,
    reference_schema, source_schema,
};
pub use shader_declaration::{
    ShaderDeclaration, ShaderForm, ShaderMeaning, ShaderParent, ShaderParentNamespace,
    ShaderProperty,
};
pub use shader_mask::ShaderMask;
pub use shader_payload::{
    ShaderBudget, ShaderCache, ShaderCacheDecode, ShaderCacheKind, ShaderConstruct,
    ShaderDefinition, ShaderDefinitionCounts, ShaderDefinitionFailure, ShaderEncoding,
    ShaderFailureReason, ShaderNameOrigin, ShaderPayload,
};
pub use shader_provider::ShaderProvider;
pub use shader_schema::{shader_payload_schema, shader_record_schema};
pub use shader_span::{ShaderOffsetBasis, ShaderSpan, ShaderText};
pub use shader_word::ShaderWord;
pub use source::{
    BuildProvenance, ContainerIdentity, Digest, Identity, RawReference, RootSource, Source,
    SourceContext, Subresource,
};
