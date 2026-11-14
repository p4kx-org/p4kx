// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    MaterialElementInfo, MaterialElementRow, MaterialSourceEntryInfo, MaterialSourceLossInfo,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::material_part_batch::{PART, REFERENCE, SHADER_REFERENCE};
use crate::material_parts::{
    MaterialLibraryEnd, MaterialLibraryHeader, MaterialPart, MaterialTree,
};
use crate::material_parts_write::{END, HEADER};
use crate::material_payload::MaterialLibraryMetadata;
use crate::material_projection::{FAILURE_NAMESPACE, LIBRARY_NAMESPACE};
use crate::material_reference::MaterialReference;
use crate::material_shader_reference::MaterialShaderReference;
use crate::{MediaError, MediaKind, MediaRecord, Source, Status};

#[path = "material_parts_read_surface.rs"]
mod surface;

type Rows<'a> = std::iter::Peekable<std::slice::Iter<'a, MediaRecord>>;

/// Reassembles one physical material library in the logical library record shape.
/// A legacy library or failure remains a single record in the same shape.
///
/// # Errors
/// Rejects missing, duplicate, foreign, out-of-order or structurally inconsistent rows.
pub fn reconstruct_material_library(records: &[MediaRecord]) -> Result<MediaRecord, MediaError> {
    let mut rows = records.iter().peekable();
    let first = rows.next().ok_or_else(invalid)?;
    let mut reader = Reader {
        rows,
        source: &first.source,
        parts: 0,
        next: Vec::new().into_iter().peekable(),
    };
    if !header_identity(first) {
        return Err(invalid());
    }
    let data = match first.subresource.namespace.as_str() {
        LIBRARY_NAMESPACE if reader.rows.next().is_none() => {
            let _: MaterialLibraryMetadata<'static> = typed(first, "library")?;
            first.data.clone()
        }
        FAILURE_NAMESPACE if first.status == Status::Failed && reader.rows.next().is_none() => {
            typed::<crate::material_payload::MaterialProjectionFailure>(first, "failure")?;
            first.data.clone()
        }
        HEADER => reader.library(first)?,
        _ => return Err(invalid()),
    };
    Ok(MediaRecord {
        kind: first.kind,
        source: first.source.clone(),
        subresource: crate::Subresource::new(
            if first.status == Status::Failed {
                FAILURE_NAMESPACE
            } else {
                LIBRARY_NAMESPACE
            },
            Vec::new(),
        ),
        status: first.status,
        data,
        issues: first.issues.clone(),
    })
}

struct Reader<'a> {
    rows: Rows<'a>,
    source: &'a Source,
    parts: u64,
    next: std::iter::Peekable<std::vec::IntoIter<MaterialPart<'static>>>,
}

impl Reader<'_> {
    fn library(&mut self, first: &MediaRecord) -> Result<Value, MediaError> {
        let header: MaterialLibraryHeader<'static> = typed(first, "library-header")?;
        if first.status
            != if header.projection_complete {
                Status::Complete
            } else {
                Status::Partial
            }
            || (!header.authored_root || header.source_losses != 0) && header.projection_complete
        {
            return Err(invalid());
        }
        let authored = self.authored(&header)?;
        let mut materials = Vec::new();
        for index in 0..header.materials {
            materials.push(self.surface(index, header.projection_complete)?);
        }
        if self.peek_part()?.is_some() {
            return Err(invalid());
        }
        let references = self.collect::<MaterialReference>(REFERENCE, "reference")?;
        let shader =
            self.collect::<MaterialShaderReference>(SHADER_REFERENCE, "shader-reference")?;
        let end = self.rows.next().ok_or_else(invalid)?;
        if !self.matches(end, END, &[]) {
            return Err(invalid());
        }
        let end: MaterialLibraryEnd = typed(end, "library-end")?;
        if end.parts != self.parts
            || usize::try_from(end.references) != Ok(references.len())
            || usize::try_from(end.shader_references) != Ok(shader.len())
            || self.rows.next().is_some()
        {
            return Err(invalid());
        }
        Ok(json!({
            "record_type": "library",
            "payload": {
                "path": header.path,
                "single": header.single,
                "default_palette": header.default_palette,
                "unknown_entries": header.unknown_entries,
                "projection_complete": header.projection_complete,
                "authored": authored,
                "materials": materials,
                "references": references,
                "shader_references": shader,
            }
        }))
    }

    fn authored(&mut self, header: &MaterialLibraryHeader<'_>) -> Result<Value, MediaError> {
        let mut entries: Vec<MaterialSourceEntryInfo<'static>> = Vec::new();
        for ordinal in 0..header.source_entries {
            match self.part()? {
                MaterialPart::SourceEntry { position, entry } if ordinal == position => {
                    entries.push(entry);
                }
                _ => return Err(invalid()),
            }
        }
        let mut losses: Vec<MaterialSourceLossInfo> = Vec::new();
        for ordinal in 0..header.source_losses {
            match self.part()? {
                MaterialPart::SourceLoss { position, loss } if ordinal == position => {
                    losses.push(loss);
                }
                _ => return Err(invalid()),
            }
        }
        let root = header
            .authored_root
            .then(|| self.tree(MaterialTree::Library))
            .transpose()?;
        Ok(
            json!({ "encoding": header.encoding, "root": root, "losses": losses, "entries": entries }),
        )
    }

    fn tree(&mut self, expected: MaterialTree) -> Result<MaterialElementInfo<'static>, MediaError> {
        let mut parts = Vec::new();
        while let Some(MaterialPart::Element { tree, .. }) = self.peek_part()? {
            if *tree != expected {
                break;
            }
            match self.part()? {
                MaterialPart::Element { part, .. } => parts.push(part),
                _ => return Err(invalid()),
            }
        }
        MaterialElementRow::reconstruct(parts)
            .map_err(|error| MediaError::Destination(error.to_string()))
    }

    fn peek_part(&mut self) -> Result<Option<&MaterialPart<'static>>, MediaError> {
        if self.next.peek().is_none()
            && self
                .rows
                .peek()
                .is_some_and(|row| row.subresource.namespace == PART)
        {
            let row = self.rows.next().ok_or_else(invalid)?;
            if !self.matches(row, PART, &[self.parts]) {
                return Err(invalid());
            }
            let items = typed::<BatchRead<MaterialPart<'static>>>(row, "part")?.items;
            if items.is_empty() {
                return Err(invalid());
            }
            self.parts = self
                .parts
                .checked_add(u64::try_from(items.len()).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            self.next = items.into_iter().peekable();
        }
        Ok(self.next.peek())
    }

    fn part(&mut self) -> Result<MaterialPart<'static>, MediaError> {
        self.peek_part()?;
        self.next.next().ok_or_else(invalid)
    }

    fn collect<T: DeserializeOwned>(
        &mut self,
        namespace: &str,
        variant: &str,
    ) -> Result<Vec<T>, MediaError> {
        let mut items = Vec::new();
        while self
            .rows
            .peek()
            .is_some_and(|row| row.subresource.namespace == namespace)
        {
            let row = self.rows.next().ok_or_else(invalid)?;
            let count = u64::try_from(items.len()).map_err(|_| invalid())?;
            if !self.matches(row, namespace, &[count]) {
                return Err(invalid());
            }
            let batch = typed::<BatchRead<T>>(row, variant)?.items;
            if batch.is_empty() {
                return Err(invalid());
            }
            items.extend(batch);
        }
        Ok(items)
    }

    fn matches(&self, row: &MediaRecord, namespace: &str, positions: &[u64]) -> bool {
        row.kind == MediaKind::Material
            && row.subresource.namespace == namespace
            && row.subresource.indices == positions
            && row.status == Status::Complete
            && row.issues.is_empty()
            && same_source(&row.source, self.source)
    }
}

fn header_identity(first: &MediaRecord) -> bool {
    first.kind == MediaKind::Material && first.subresource.indices.is_empty()
}

fn same_source(left: &Source, right: &Source) -> bool {
    left.root == right.root
        && left.identity == right.identity
        && left.size == right.size
        && left.declared_integrity == right.declared_integrity
        && left.verification == right.verification
        && left.raw == right.raw
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchRead<T> {
    items: Vec<T>,
}

fn typed<T: DeserializeOwned>(row: &MediaRecord, variant: &str) -> Result<T, MediaError> {
    if row.data.get("record_type").and_then(Value::as_str) != Some(variant) {
        return Err(invalid());
    }
    piece(row, "/payload")
}

fn piece<T: DeserializeOwned>(row: &MediaRecord, path: &str) -> Result<T, MediaError> {
    T::deserialize(row.data.pointer(path).ok_or_else(invalid)?)
        .map_err(|cause| MediaError::Destination(format!("invalid typed material row: {cause}")))
}

fn invalid() -> MediaError {
    MediaError::Destination("material library has absent, inconsistent or out-of-order rows".into())
}
