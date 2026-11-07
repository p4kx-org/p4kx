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
mod checkpoint;
mod chunk;
mod chunk_copy;
mod contract;
mod directory_context;
mod directory_index;
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
mod identifier;
mod index_destination;
mod index_file;
mod integrity;
mod kind;
mod limits;
mod lock_namespace;
mod lookup_work;
mod material_payload;
mod material_projection;
mod material_provider;
mod material_reference;
mod material_schema;
mod material_surface;
mod media_record;
mod media_writer;
mod metadata_input;
#[cfg(test)]
mod metadata_input_tests;
mod numeric;
mod observed_write;
mod output_path;
#[cfg(test)]
mod owned_output_tests;
mod provider;
mod publication;
mod raw_output;
mod reference;
mod reference_resolve;
mod retained_payload;
mod retained_source;
mod schema;
mod schema_merge;
#[cfg(test)]
mod schema_merge_tests;
mod shader_declaration;
mod shader_diagnostic;
mod shader_mask;
mod shader_payload;
mod shader_projection;
mod shader_provider;
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
#[cfg(test)]
mod write_context_tests;

pub use archive_metadata_diagnostic::{ArchiveMetadataDiagnostic, ArchiveMetadataFailure};
pub use archive_provider::ArchiveProvider;
pub use archive_raw::{ArchiveRawChunk, ArchiveRawKey, ArchiveRawSpan};
pub use archive_record::{ArchiveDirectoryRecord, ArchiveExtraData, ArchiveExtraState};
pub use archive_section::{ArchiveSection, ArchiveSectionData, ArchiveSectionPresence};
pub use archive_table::{
    ArchiveFreeRange, ArchiveFreeState, ArchiveInstallation, ArchiveProgressBound,
    ArchiveReservedState,
};
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
pub use export::{ExportOptions, ExportOutcome, export, export_with};
pub use identifier::{IntegerWidth, OriginalIdentifier};
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
pub use material_surface::{MaterialPalette, MaterialSurface, MaterialSurfaceBasis};
pub use media_writer::MediaWriter;
pub use numeric::{Float32, Float64, ValueOrigin};
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
