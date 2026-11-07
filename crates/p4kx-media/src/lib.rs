// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded media export with physical identities and atomic generation publication.

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
    MediaFormat, MediaVersion, index_schema, record_schema, reference_schema, source_schema,
};
pub use source::{
    BuildProvenance, ContainerIdentity, Digest, Identity, RawReference, RootSource, Source,
    SourceContext, Subresource,
};
