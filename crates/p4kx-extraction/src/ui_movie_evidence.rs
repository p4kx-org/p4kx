// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::{MovieSource, MovieSpan, SpanRuns, TagSite};

use crate::{
    InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, UiMovie,
};

use SourceUseRole::{Conversion, Payload, Selector};

/// The admitted physical input and decoded coordinate domains of one UI movie.
pub struct UiMovieEvidence {
    source: SourceScope,
    bound: bool,
    name_scope: InputScope,
    read: MovieSource,
    decoded_from: Vec<SourceUse>,
}

/// A retained contribution's coordinate domain within a parsed GFx or SWF movie.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum UiSourceSite<'a> {
    /// One logical decoded header range.
    Bytes(MovieSpan),
    /// One admitted tag header.
    Header(TagSite),
    /// One admitted tag and its payload range.
    Body(TagSite),
    /// One ordinal in a multi-item tag.
    Item(TagSite, usize),
    /// The extent examined to determine a scoped result.
    Survey(MovieSpan),
    /// A set of repeated tag headers of one code.
    Runs(u16, &'a SpanRuns),
}

impl UiSourceSite<'_> {
    fn coordinates(self, compressed: bool) -> (&'static str, Vec<u64>) {
        let domain = |direct, decoded| if compressed { decoded } else { direct };
        match self {
            Self::Bytes(span) => (
                domain("gfxBytes", "gfxDecodedBytes"),
                span_indices(span).to_vec(),
            ),
            Self::Header(tag) => (
                domain("gfxTagHeader", "gfxDecodedTagHeader"),
                vec![
                    u64::from(tag.code),
                    tag.header.offset as u64,
                    tag.header.length as u64,
                ],
            ),
            Self::Body(tag) => (domain("gfxTag", "gfxDecodedTag"), tag_indices(tag).to_vec()),
            Self::Item(tag, ordinal) => {
                let mut indices = tag_indices(tag).to_vec();
                indices.push(ordinal as u64);
                (domain("gfxTagItem", "gfxDecodedTagItem"), indices)
            }
            Self::Survey(span) => (
                domain("gfxSurvey", "gfxDecodedSurvey"),
                span_indices(span).to_vec(),
            ),
            Self::Runs(code, runs) => {
                let mut indices = vec![u64::from(code)];
                for run in runs.runs() {
                    indices.extend([
                        run.first.offset as u64,
                        run.first.length as u64,
                        run.stride as u64,
                        run.count as u64,
                    ]);
                }
                (domain("gfxTagRuns", "gfxDecodedTagRuns"), indices)
            }
        }
    }
}

fn tag_indices(tag: TagSite) -> [u64; 5] {
    [
        u64::from(tag.code),
        tag.header.offset as u64,
        tag.header.length as u64,
        tag.body.offset as u64,
        tag.body.length as u64,
    ]
}

const fn span_indices(span: MovieSpan) -> [u64; 2] {
    [span.offset as u64, span.length as u64]
}

impl UiMovieEvidence {
    /// Registers the original input that supplied the selected movie.
    ///
    /// # Errors
    ///
    /// Returns a source-evidence registration error.
    pub fn observe(movie: &UiMovie<'_>, uses: &SourceUses) -> Result<Self, SourceEvidenceError> {
        let input = movie.source_input();
        let source = uses.observe(input.unwrap_or(SourceInput::Unknown), InputScope::Payload)?;
        let mut result = Self {
            source,
            bound: input.is_some(),
            name_scope: match input {
                Some(SourceInput::Archive(_) | SourceInput::ArchiveView(_)) => {
                    InputScope::ArchiveDirectoryMetadata
                }
                _ => InputScope::FileMetadata,
            },
            read: *movie.movie().read_source(),
            decoded_from: Vec::new(),
        };
        let read = result.read;
        if let Some(compressed) = read.compressed {
            result.decoded_from = vec![
                result.original(read.signature, "compressionSignature", Selector)?,
                result.original(read.declared_length, "decodedLengthLimit", Selector)?,
                result.original(compressed, "compressedPayload", Conversion)?,
            ];
        }
        Ok(result)
    }

    /// Returns whether the actual original input was bound to the reader.
    #[must_use]
    pub const fn is_bound(&self) -> bool {
        self.bound
    }

    pub(crate) const fn stage(&self) -> MovieSpan {
        self.read.stage
    }

    /// Registers the stored Entry name or the observed standalone filename.
    ///
    /// # Errors
    ///
    /// Returns a source-evidence registration error.
    pub fn name_use(&self) -> Result<SourceUse, SourceEvidenceError> {
        self.source.for_scope(self.name_scope).at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            Payload,
        )
    }

    /// Registers a field in the unmodified original movie header.
    ///
    /// # Errors
    ///
    /// Returns a source-evidence registration error.
    pub fn original(
        &self,
        span: MovieSpan,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.use_at("gfxMovieInput", field, span_indices(span).to_vec(), role)
    }

    /// Registers the original contributions that determine the decoded length.
    ///
    /// # Errors
    ///
    /// Returns a source-evidence registration error.
    pub fn length_uses(&self) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        if !self.decoded_from.is_empty() {
            return Ok(self.decoded_from.clone());
        }
        let read = self.read;
        Ok(vec![
            self.original(read.signature, "compressionSignature", Selector)?,
            self.original(read.declared_length, "declaredLength", Selector)?,
            self.use_at(
                "gfxMovieInput",
                "measuredByteExtent",
                vec![0, read.input_length as u64],
                Conversion,
            )?,
        ])
    }

    /// Registers an admitted logical field under its original coordinate domain.
    ///
    /// # Errors
    ///
    /// Returns a source-evidence registration error.
    pub fn decoded(
        &self,
        site: UiSourceSite<'_>,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        let (namespace, indices) = site.coordinates(!self.decoded_from.is_empty());
        self.use_at(namespace, field, indices, role)
    }

    /// Appends the consumed original compression input for decoded fields.
    pub fn append_decoding(&self, actual: &mut Vec<SourceUse>) {
        actual.extend_from_slice(&self.decoded_from);
    }

    fn use_at(
        &self,
        namespace: &str,
        field: &str,
        indices: Vec<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.source.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: field.into(),
            },
            role,
        )
    }
}
