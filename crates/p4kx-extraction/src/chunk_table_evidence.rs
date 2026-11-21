// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::ChunkTableRow;
use thiserror::Error;

use crate::{
    ChunkDumpMode, ChunkSource, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath,
    FieldPathError, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

const SIGNATURE: Range<u64> = 0..4;
const VERSION: Range<u64> = 4..8;
const COUNT: Range<u64> = 8..12;
const TABLE_OFFSET: Range<u64> = 12..16;

/// Failure to represent a selected chunk table field.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ChunkEvidenceError {
    /// The selected physical chunk has no original row.
    #[error("parsed chunk {index} has no table row")]
    MissingRow {
        /// The physical table ordinal.
        index: usize,
    },
    /// The original input could not be bound.
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    /// The output field address could not be represented.
    #[error(transparent)]
    Path(#[from] FieldPathError),
    /// The output field was already assigned.
    #[error(transparent)]
    Field(#[from] FieldOriginsError),
}

/// Original inputs of one admitted chunk file.
pub struct ChunkTableEvidence<'source> {
    source: &'source ChunkSource,
    sources: SourceUses,
    scope: SourceScope,
    path_scope: InputScope,
    complete: bool,
}

impl<'source> ChunkTableEvidence<'source> {
    /// Binds the evidence to the actual file supplied by the chunk reader.
    ///
    /// # Errors
    /// Returns a source observation error.
    pub fn new(source: &'source ChunkSource) -> Result<Self, ChunkEvidenceError> {
        let input = source.original_input();
        let sources = SourceUses::new();
        let scope = sources.observe(input, InputScope::Payload)?;
        let path_scope = if matches!(input, SourceInput::Archive(_)) {
            InputScope::ArchiveDirectoryMetadata
        } else {
            InputScope::FileMetadata
        };
        Ok(Self {
            source,
            sources,
            scope,
            path_scope,
            complete: !matches!(input, SourceInput::Unknown),
        })
    }

    /// Returns the registry of actual uses bound to this file.
    #[must_use]
    pub const fn uses(&self) -> &SourceUses {
        &self.sources
    }

    /// Returns the scope of the admitted payload.
    #[must_use]
    pub const fn scope(&self) -> &SourceScope {
        &self.scope
    }

    /// Returns whether the chunk reader retained its original input.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    /// Returns the original source name's metadata origin.
    ///
    /// # Errors
    /// Returns a source observation error.
    pub fn path(&self) -> Result<FieldOrigin, ChunkEvidenceError> {
        let retained = self.scope.for_scope(self.path_scope).at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
        Ok(self.one(&retained))
    }

    /// Retains the original chunk table layout and its addressing inputs.
    ///
    /// # Errors
    /// Returns a source observation error.
    pub fn rows(&self) -> Result<ChunkTableRows<'_, '_>, ChunkEvidenceError> {
        let signature = self.header("signature", SIGNATURE)?;
        let table_offset = self.header("tableOffset", TABLE_OFFSET)?;
        let count = self.header("rowCount", COUNT)?;
        let index = self.origin(&[table_offset.clone(), count.clone()]);
        let layout = self.one(&signature);
        Ok(ChunkTableRows {
            evidence: self,
            index,
            signature,
            layout,
            count,
            table_offset,
        })
    }

    /// Returns the origin of the declared file container.
    ///
    /// # Errors
    /// Returns a source observation error.
    pub fn container(&self) -> Result<FieldOrigin, ChunkEvidenceError> {
        Ok(self.one(&self.header("signature", SIGNATURE)?))
    }

    /// Returns the origin of the declared container version.
    ///
    /// # Errors
    /// Returns a source observation error.
    pub fn version(&self) -> Result<FieldOrigin, ChunkEvidenceError> {
        Ok(self.one(&self.header("version", VERSION)?))
    }

    /// Returns the original selectors used to locate a prepared chunk payload.
    ///
    /// # Errors
    /// Returns a missing-row or source observation error.
    pub fn selected_payload(
        &self,
        index: usize,
        mode: ChunkDumpMode,
    ) -> Result<Vec<SourceUse>, ChunkEvidenceError> {
        let file = self.source.file();
        let columns = self.columns(index)?;
        let role = SourceUseRole::Selector;
        let header = |name, range: Range<u64>| {
            self.site(
                "cryEngineChunkHeader",
                name,
                vec![range.start, range.end],
                role,
            )
        };
        let word = |name, range: Range<u64>| {
            self.site(
                "cryEngineChunkTable",
                name,
                vec![index as u64, range.start, range.end],
                role,
            )
        };
        let mut uses = vec![
            header("signature", SIGNATURE)?,
            header("rowCount", COUNT)?,
            word("offset", columns.offset)?,
        ];
        let table = header("tableOffset", TABLE_OFFSET)?;
        if let Some(range) = columns.size {
            uses.push(table);
            uses.push(word("size", range)?);
        } else {
            uses.extend(self.ivo_extent(&file.table_range(), table)?);
        }
        if matches!(mode, ChunkDumpMode::Text) {
            uses.push(word("type", columns.kind)?);
        }
        Ok(uses)
    }

    fn header(&self, name: &str, range: Range<u64>) -> Result<SourceUse, ChunkEvidenceError> {
        self.site(
            "cryEngineChunkHeader",
            name,
            vec![range.start, range.end],
            SourceUseRole::Payload,
        )
    }

    fn column(
        &self,
        index: usize,
        name: &str,
        range: Range<u64>,
    ) -> Result<SourceUse, ChunkEvidenceError> {
        self.site(
            "cryEngineChunkTable",
            name,
            vec![index as u64, range.start, range.end],
            SourceUseRole::Payload,
        )
    }

    fn ivo_extent(
        &self,
        table: &Range<u64>,
        table_offset: SourceUse,
    ) -> Result<[SourceUse; 3], ChunkEvidenceError> {
        let offsets = self.site(
            "cryEngineChunkTable",
            "offsetColumn",
            vec![table.start, table.end],
            SourceUseRole::Conversion,
        )?;
        let extent = self.site(
            "cryEngineChunkFile",
            "decodedExtent",
            Vec::new(),
            SourceUseRole::Conversion,
        )?;
        Ok([offsets, extent, table_offset])
    }

    /// Binds a format-relative location to the admitted chunk file.
    ///
    /// # Errors
    /// Returns a source observation error.
    pub fn site(
        &self,
        namespace: &str,
        name: &str,
        indices: Vec<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse, ChunkEvidenceError> {
        Ok(self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: name.into(),
            },
            role,
        )?)
    }

    /// Derives an origin from uses in this chunk-file observation.
    #[must_use]
    pub fn origin(&self, uses: &[SourceUse]) -> FieldOrigin {
        self.sources.origin(uses, self.complete)
    }

    fn one(&self, usage: &SourceUse) -> FieldOrigin {
        self.origin(std::slice::from_ref(usage))
    }

    fn columns(&self, index: usize) -> Result<ChunkTableRow, ChunkEvidenceError> {
        self.source
            .file()
            .table_row(index)
            .ok_or(ChunkEvidenceError::MissingRow { index })
    }
}

/// Physical table evidence that shares the owning chunk file and its layout.
pub struct ChunkTableRows<'evidence, 'source> {
    evidence: &'evidence ChunkTableEvidence<'source>,
    index: FieldOrigin,
    signature: SourceUse,
    layout: FieldOrigin,
    count: SourceUse,
    table_offset: SourceUse,
}

impl ChunkTableRows<'_, '_> {
    /// Returns the header fields that locate and enumerate the rows.
    #[must_use]
    pub const fn index(&self) -> &FieldOrigin {
        &self.index
    }

    /// Returns the original declared row count.
    #[must_use]
    pub fn count(&self) -> FieldOrigin {
        self.evidence.one(&self.count)
    }

    /// Returns the original type-column inputs used by a table-wide predicate.
    ///
    /// # Errors
    /// Returns a source observation error.
    pub fn type_selection(&self) -> Result<FieldOrigin, ChunkEvidenceError> {
        let file = self.evidence.source.file();
        let mut uses = vec![
            self.signature.clone(),
            self.count.clone(),
            self.table_offset.clone(),
        ];
        let count = file.chunks().len() as u64;
        let table = file.table_range();
        if let Some(stride) = (table.end - table.start).checked_div(count) {
            let word = self.evidence.columns(0)?.kind;
            uses.push(self.evidence.site(
                "cryEngineChunkTable",
                "typeColumn",
                vec![word.start, word.end, stride, count],
                SourceUseRole::Selector,
            )?);
        }
        Ok(self.evidence.origin(&uses))
    }

    /// Returns the original type word used to derive the hexadecimal representation.
    ///
    /// # Errors
    /// Returns a row or source observation error.
    pub fn hexadecimal_type(&self, index: usize) -> Result<FieldOrigin, ChunkEvidenceError> {
        let range = self.evidence.columns(index)?.kind;
        let source = self.evidence.site(
            "cryEngineChunkTable",
            "type",
            vec![index as u64, range.start, range.end],
            SourceUseRole::Conversion,
        )?;
        Ok(self.evidence.one(&source))
    }

    /// Writes the selected physical row's immediate field origins at the output path.
    ///
    /// # Errors
    /// Returns a row, field address or source observation error.
    pub fn write_row(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        index: usize,
    ) -> Result<(), ChunkEvidenceError> {
        let evidence = self.evidence;
        let columns = evidence.columns(index)?;
        let kind_word = evidence.column(index, "type", columns.kind)?;
        let type_id = evidence.one(&kind_word);
        let kind = evidence.origin(&[kind_word, self.signature.clone()]);
        let version_word = evidence.column(index, "version", columns.version)?;
        let version = evidence.one(&version_word);
        let offset_use = evidence.column(index, "offset", columns.offset)?;
        let offset = evidence.one(&offset_use);
        let id = match columns.id {
            Some(range) => evidence.one(&evidence.column(index, "id", range)?),
            None => self.layout.clone(),
        };
        let order = if columns.size.is_some() {
            evidence.origin(&[version_word, self.signature.clone()])
        } else {
            self.layout.clone()
        };
        let size = if let Some(range) = columns.size {
            evidence.one(&evidence.column(index, "size", range)?)
        } else {
            let [offsets, extent, table] = evidence.ivo_extent(
                &evidence.source.file().table_range(),
                self.table_offset.clone(),
            )?;
            evidence.origin(&[offsets, extent, table, offset_use])
        };
        for (name, origin) in [
            ("index", self.index.clone()),
            ("type", kind.clone()),
            ("type_id", type_id),
            ("known", kind),
            ("version", version),
            ("id", id),
            ("byte_order", order),
            ("offset", offset),
            ("size", size),
        ] {
            add(fields, path, name, origin)?;
        }
        Ok(())
    }
}

fn add(
    fields: &mut FieldOrigins,
    parent: &FieldPath,
    name: &str,
    origin: FieldOrigin,
) -> Result<(), ChunkEvidenceError> {
    let mut path = parent.clone();
    path.push_field(name)?;
    Ok(fields.insert(path, origin)?)
}
