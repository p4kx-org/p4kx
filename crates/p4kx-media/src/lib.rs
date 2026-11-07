// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded media export with physical identities and atomic generation publication.

mod archive_metadata_schema;
mod archive_provider;
mod artifact;
mod checkpoint;
mod chunk;
mod chunk_copy;
mod contract;
mod directory_issue;
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
mod index_file;
mod integrity;
mod kind;
mod limits;
mod media_record;
mod media_writer;
mod numeric;
mod observed_write;
mod output_path;
mod provider;
mod raw_output;
mod reference;
mod reference_resolve;
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
mod traversal;

pub use archive_provider::ArchiveProvider;
pub use chunk::{
    ByteOrder, ChunkLayout, ChunkReference, DataOrder, DataType, OffsetBasis, SourceRange,
};
pub use contract::{
    Artifact, Counts, Index, KindSummary, MediaIssue, MediaRecord, Severity, Status, Tally,
};
pub use directory_issue::DirectoryIssue;
pub use error::{ExtractionCounts, MediaError};
pub use export::{ExportOptions, ExportOutcome, export, export_with};
pub use identifier::{IntegerWidth, OriginalIdentifier};
pub use integrity::{Algorithm, ByteScope, Verification};
pub use kind::MediaKind;
pub use limits::Limits;
pub use media_writer::MediaWriter;
pub use numeric::{Float32, Float64, ValueOrigin};
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
