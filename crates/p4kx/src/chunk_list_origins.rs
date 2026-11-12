// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use anyhow::{Context, Result};
use p4kx_extraction::chunks::{ChunkFile, ChunkTableRow, Container};
use p4kx_extraction::{
    ChunkDumpMode, ChunkSource, FieldOrigin, FieldOrigins, FieldPath, InputScope, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
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
        let shared = evidence.row_selection(file)?;
        let mut fields = FieldOrigins::new();
        let root = FieldPath::root();
        add(&mut fields, &root, "file", evidence.path()?)?;
        add(&mut fields, &root, "container", shared.layout.clone())?;
        add(
            &mut fields,
            &root,
            "version",
            evidence.one(&evidence.header("version", VERSION)?),
        )?;
        add(&mut fields, &root, "chunks", shared.index.clone())?;
        let mut path = FieldPath::parse("/chunks")?;
        for row in 0..file.chunks().len() {
            let columns = file
                .table_row(row)
                .context("parsed chunk has no table row")?;
            path.push_index(row)?;
            evidence.row(&mut fields, &path, row, columns, &shared)?;
            path.pop();
        }
        Ok(Self {
            field_origins: fields,
            provenance: evidence.sources.freeze()?,
        })
    }
}

pub(super) struct RowSelection {
    index: FieldOrigin,
    signature: SourceUse,
    layout: FieldOrigin,
    size: Option<[SourceUse; 3]>,
}

pub(super) struct Evidence {
    pub(super) sources: SourceUses,
    pub(super) scope: SourceScope,
    path_scope: InputScope,
    pub(super) complete: bool,
}

impl Evidence {
    pub(super) fn new(input: SourceInput<'_>) -> Result<Self> {
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

    pub(super) fn path(&self) -> Result<FieldOrigin> {
        let retained = self.scope.for_scope(self.path_scope).at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
        Ok(self.one(&retained))
    }

    pub(super) fn row_selection(&self, file: &ChunkFile<Vec<u8>>) -> Result<RowSelection> {
        let signature = self.header("signature", SIGNATURE)?;
        let table_offset = self.header("tableOffset", TABLE_OFFSET)?;
        let count = self.header("rowCount", COUNT)?;
        let index = self.origin(&[table_offset.clone(), count]);
        let size = if file.container() == Container::Ivo && !file.chunks().is_empty() {
            Some(self.ivo_extent(&file.table_range(), table_offset)?)
        } else {
            None
        };
        let layout = self.one(&signature);
        Ok(RowSelection {
            index,
            signature,
            layout,
            size,
        })
    }

    pub(super) fn selected_payload(
        &self,
        file: &ChunkFile<Vec<u8>>,
        index: usize,
        columns: ChunkTableRow,
        mode: ChunkDumpMode,
    ) -> Result<Vec<SourceUse>> {
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

    pub(super) fn site(
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

    pub(super) fn origin(&self, uses: &[SourceUse]) -> FieldOrigin {
        self.sources.origin(uses, self.complete)
    }

    fn one(&self, usage: &SourceUse) -> FieldOrigin {
        self.origin(std::slice::from_ref(usage))
    }

    pub(super) fn row(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        index: usize,
        columns: ChunkTableRow,
        shared: &RowSelection,
    ) -> Result<()> {
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
            let [offsets, extent, table] = shared.size.as_ref().context("missing IVO extent")?;
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
) -> Result<()> {
    let mut path = parent.clone();
    path.push_field(name)?;
    Ok(fields.insert(path, origin)?)
}
