// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::ops::Range;

use base64::engine::general_purpose::STANDARD;
use p4kx_extraction::{
    CancelToken, DecimalU64, PhysicalSourceRef, SourceLocation, SourceUseRole, SourceValue,
    SourcedValue,
};
use p4kx_model::{
    IncludedObjectKind, IncludedObjectTables, IncludedObjectsChunk, IncludedObjectsError,
    IncludedPathSlot, SocEntityFile,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes::check;
use crate::soc_scene_included_row::ObjectRow;
use crate::soc_scene_origin::XmlOrigin;

#[derive(Default)]
pub(crate) struct IncludedSources {
    members: BTreeMap<PhysicalSourceRef, Vec<usize>>,
    tables: Vec<IncludedSource>,
}

impl IncludedSources {
    pub(crate) fn bind(
        &mut self,
        file: &SocEntityFile<'_>,
        member: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Vec<usize>, SocSceneError> {
        let source = member.source();
        if let Some(indices) = self.members.get(&source) {
            return Ok(indices.clone());
        }
        let start = self.tables.len();
        for chunk in file.included_objects_chunks() {
            check(cancel)?;
            self.tables.try_reserve(1)?;
            self.tables
                .push(IncludedSource::read(chunk, member, cancel)?);
        }
        let indices: Vec<_> = (start..self.tables.len()).collect();
        self.members.insert(source, indices.clone());
        Ok(indices)
    }

    pub(crate) fn finish(self) -> Vec<IncludedSource> {
        self.tables
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IncludedSource {
    source: PhysicalSourceRef,
    ordinal: usize,
    byte_offset: DecimalU64,
    byte_length: DecimalU64,
    version: u32,
    byte_order: &'static str,
    #[schemars(description = "Original chunk payload encoded with standard padded Base64.")]
    raw: SourcedValue<String>,
    tables: SourcedValue<Tables>,
}

impl IncludedSource {
    fn read(
        chunk: IncludedObjectsChunk<'_>,
        member: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let header = chunk.header();
        let length = header.size();
        let origin = IncludedOrigin {
            member,
            base: header.offset(),
            ordinal: chunk.chunk_index(),
        };
        let raw = encode_raw(chunk.bytes(), cancel)?;
        let (data, complete) = match chunk.tables() {
            Ok(tables) => {
                let data = Tables::read(&tables, length, &origin, cancel)?;
                let complete = data.complete;
                (SourceValue::Present(data), complete)
            }
            Err(IncludedObjectsError::BigEndian) => (SourceValue::Unsupported, false),
            Err(_) => (SourceValue::Invalid, false),
        };
        Ok(Self {
            source: member.source(),
            ordinal: chunk.chunk_index(),
            byte_offset: DecimalU64::new(header.offset()),
            byte_length: DecimalU64::new(length),
            version: header.version(),
            byte_order: header.byte_order().name(),
            raw: origin.cell(0..length, "raw", SourceValue::Present(raw), true)?,
            tables: origin.cell(0..length, "tables", data, complete)?,
        })
    }
}

fn encode_raw(bytes: &[u8], cancel: &CancelToken) -> Result<String, SocSceneError> {
    use base64::Engine as _;

    const BLOCK_LEN: usize = 3 * 16_384;
    let mut result = String::new();
    let encoded_len = base64::encoded_len(bytes.len(), true).ok_or(SocSceneError::Position)?;
    result.try_reserve_exact(encoded_len)?;
    for block in bytes.chunks(BLOCK_LEN) {
        check(cancel)?;
        STANDARD.encode_string(block, &mut result);
    }
    Ok(result)
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Tables {
    layout: SourcedValue<u32>,
    declared_object_bytes: SourcedValue<u32>,
    object_range: ByteRange,
    trailing_range: ByteRange,
    models: Vec<SourcePath>,
    materials: Vec<SourcePath>,
    palettes: Vec<SourcePath>,
    rows: Vec<ObjectRow>,
    complete: bool,
}

impl Tables {
    fn read(
        tables: &IncludedObjectTables<'_>,
        length: u64,
        origin: &IncludedOrigin<'_, '_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let bounds = tables.object_range();
        let declared = tables.object_len_declared();
        let model_count = tables.models().len();
        let palette_count = tables.palettes().len();
        let mut complete = bounds.end - bounds.start == u64::from(declared);
        let mut rows = Vec::new();
        for row in tables.rows() {
            check(cancel)?;
            complete &= matches!(
                row.kind(),
                IncludedObjectKind::Placement(_) | IncludedObjectKind::Kind16
            );
            rows.try_reserve(1)?;
            rows.push(ObjectRow::read(row, origin, model_count, palette_count)?);
        }
        let declaration_start = bounds.start - size_of::<u32>() as u64;
        Ok(Self {
            layout: origin.cell(
                tables.layout_range(),
                "layout",
                SourceValue::Present(tables.layout()),
                true,
            )?,
            declared_object_bytes: origin.cell(
                declaration_start..bounds.start,
                "declaredObjectBytes",
                SourceValue::Present(declared),
                true,
            )?,
            trailing_range: ByteRange::new(bounds.end..length),
            object_range: ByteRange::new(bounds),
            models: paths(tables.models(), origin, "modelPath", cancel)?,
            materials: paths(tables.materials(), origin, "materialPath", cancel)?,
            palettes: paths(tables.palettes(), origin, "palettePath", cancel)?,
            rows,
            complete,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
struct SourcePath {
    ordinal: usize,
    range: ByteRange,
    path: SourcedValue<String>,
}

fn paths<'chunk>(
    slots: impl ExactSizeIterator<Item = IncludedPathSlot<'chunk>>,
    origin: &IncludedOrigin<'_, '_>,
    field: &str,
    cancel: &CancelToken,
) -> Result<Vec<SourcePath>, SocSceneError> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(slots.len())?;
    for slot in slots {
        check(cancel)?;
        let path = if slot.bytes().contains(&0) {
            std::str::from_utf8(slot.path_bytes()).ok()
        } else {
            None
        };
        let value = path.map_or(SourceValue::Invalid, |path| {
            SourceValue::Present(path.to_owned())
        });
        let range = slot.range();
        rows.push(SourcePath {
            ordinal: slot.ordinal(),
            range: ByteRange::new(range.clone()),
            path: origin.cell(range, field, value, true)?,
        });
    }
    Ok(rows)
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct ByteRange {
    start: DecimalU64,
    end: DecimalU64,
}

impl ByteRange {
    pub(super) const fn new(range: Range<u64>) -> Self {
        Self {
            start: DecimalU64::new(range.start),
            end: DecimalU64::new(range.end),
        }
    }
}

pub(super) struct IncludedOrigin<'scope, 'source> {
    member: &'scope XmlOrigin<'source>,
    base: u64,
    ordinal: usize,
}

impl IncludedOrigin<'_, '_> {
    pub(super) fn cell<T>(
        &self,
        range: Range<u64>,
        field: &str,
        value: SourceValue<T>,
        complete: bool,
    ) -> Result<SourcedValue<T>, SocSceneError> {
        let used = self.member.scope.at(
            SourceLocation::Subresource {
                namespace: "socIncludedObjects".to_owned(),
                indices: vec![
                    self.ordinal as u64,
                    self.base + range.start,
                    self.base + range.end,
                ],
                field: field.to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        Ok(self.member.sources.value(value, &[used], complete))
    }
}
