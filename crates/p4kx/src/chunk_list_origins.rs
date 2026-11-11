// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use anyhow::{Context, Result};
use p4kx_extraction::chunks::{ChunkTableRow, Container};
use p4kx_extraction::{
    ChunkSource, FieldOrigin, FieldOrigins, FieldPath, InputScope, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use serde::Serialize;

const SIGNATURE: Range<u64> = 0..4;
const VERSION: Range<u64> = 4..8;
const COUNT: Range<u64> = 8..12;
const TABLE_OFFSET: Range<u64> = 12..16;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChunkListOrigins {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl ChunkListOrigins {
    pub(super) fn capture(source: &ChunkSource) -> Result<Self> {
        let evidence = Evidence::new(source.original_input())?;
        let file = source.file();
        let signature = evidence.header("signature", SIGNATURE)?;
        let table_offset = evidence.header("tableOffset", TABLE_OFFSET)?;
        let count = evidence.header("rowCount", COUNT)?;
        let index = evidence.origin(&[table_offset.clone(), count]);
        let size = if file.container() == Container::Ivo && !file.chunks().is_empty() {
            Some(evidence.ivo_extent(&file.table_range(), table_offset)?)
        } else {
            None
        };
        let mut fields = FieldOrigins::new();
        let root = FieldPath::root();
        add(&mut fields, &root, "file", evidence.path()?)?;
        add(&mut fields, &root, "container", evidence.one(&signature))?;
        add(
            &mut fields,
            &root,
            "version",
            evidence.one(&evidence.header("version", VERSION)?),
        )?;
        add(&mut fields, &root, "chunks", index.clone())?;
        let layout = evidence.one(&signature);
        let shared = RowSelection {
            index: &index,
            signature: &signature,
            layout: &layout,
            size: size.as_ref(),
        };
        for row in 0..file.chunks().len() {
            let columns = file
                .table_row(row)
                .context("parsed chunk has no table row")?;
            evidence.row(&mut fields, row, columns, &shared)?;
        }
        Ok(Self {
            field_origins: fields,
            provenance: evidence.sources.freeze()?,
        })
    }
}

struct RowSelection<'a> {
    index: &'a FieldOrigin,
    signature: &'a SourceUse,
    layout: &'a FieldOrigin,
    size: Option<&'a [SourceUse; 3]>,
}

struct Evidence {
    sources: SourceUses,
    scope: SourceScope,
    path_scope: InputScope,
    complete: bool,
}

impl Evidence {
    fn new(input: SourceInput<'_>) -> Result<Self> {
        let sources = SourceUses::new();
        let scope = sources.observe(input, InputScope::Payload)?;
        let path_scope = if matches!(input, SourceInput::Archive(_)) {
            InputScope::ArchiveDirectoryMetadata
        } else {
            InputScope::FileMetadata
        };
        Ok(Self {
            sources,
            scope,
            path_scope,
            complete: !matches!(input, SourceInput::Unknown),
        })
    }

    fn path(&self) -> Result<FieldOrigin> {
        let retained = self.scope.for_scope(self.path_scope).at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
        Ok(self.one(&retained))
    }

    fn header(&self, name: &str, range: Range<u64>) -> Result<SourceUse> {
        self.site(
            "cryEngineChunkHeader",
            name,
            vec![range.start, range.end],
            SourceUseRole::Payload,
        )
    }

    fn column(&self, index: usize, name: &str, range: Range<u64>) -> Result<SourceUse> {
        self.site(
            "cryEngineChunkTable",
            name,
            vec![index as u64, range.start, range.end],
            SourceUseRole::Payload,
        )
    }

    fn ivo_extent(&self, table: &Range<u64>, table_offset: SourceUse) -> Result<[SourceUse; 3]> {
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

    fn site(
        &self,
        namespace: &str,
        name: &str,
        indices: Vec<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        Ok(self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: name.into(),
            },
            role,
        )?)
    }

    fn origin(&self, uses: &[SourceUse]) -> FieldOrigin {
        self.sources.origin(uses, self.complete)
    }

    fn one(&self, usage: &SourceUse) -> FieldOrigin {
        self.origin(std::slice::from_ref(usage))
    }

    fn row(
        &self,
        fields: &mut FieldOrigins,
        index: usize,
        columns: ChunkTableRow,
        shared: &RowSelection<'_>,
    ) -> Result<()> {
        let mut path = FieldPath::parse("/chunks")?;
        path.push_index(index)?;
        let kind_word = self.column(index, "type", columns.kind)?;
        let type_id = self.one(&kind_word);
        let kind = self.origin(&[kind_word, shared.signature.clone()]);
        let version_word = self.column(index, "version", columns.version)?;
        let version = self.one(&version_word);
        let offset_use = self.column(index, "offset", columns.offset)?;
        let offset = self.one(&offset_use);
        let id = match columns.id {
            Some(range) => self.one(&self.column(index, "id", range)?),
            None => shared.layout.clone(),
        };
        let order = if columns.size.is_some() {
            self.origin(&[version_word, shared.signature.clone()])
        } else {
            shared.layout.clone()
        };
        let size = if let Some(range) = columns.size {
            self.one(&self.column(index, "size", range)?)
        } else {
            let [offsets, extent, table] = shared.size.context("missing IVO extent")?;
            self.origin(&[offsets.clone(), extent.clone(), table.clone(), offset_use])
        };
        for (name, origin) in [
            ("index", shared.index.clone()),
            ("type", kind.clone()),
            ("type_id", type_id),
            ("known", kind),
            ("version", version),
            ("id", id),
            ("byte_order", order),
            ("offset", offset),
            ("size", size),
        ] {
            add(fields, &path, name, origin)?;
        }
        Ok(())
    }
}

fn add(
    fields: &mut FieldOrigins,
    parent: &FieldPath,
    name: &str,
    origin: FieldOrigin,
) -> Result<()> {
    let mut path = parent.clone();
    path.push_field(name)?;
    Ok(fields.insert(path, origin)?)
}
