// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model::{
    ModelNodeMeshChunk, ModelNodePropertySource, ModelNodeSource, ModelNodeSources, Node,
};
use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldPath, SourceEvidenceError, SourceLocation, SourceUse,
    SourceUseRole,
};

use crate::content_origin::EntryPageSources;
use crate::model_item_page::{self, EvidenceError};

pub(crate) struct NodeFields<'a> {
    pub(crate) entry: &'a EntryPageSources,
    pub(crate) sources: Option<&'a ModelNodeSources>,
    pub(crate) chunk: Option<&'a ModelNodeMeshChunk>,
    pub(crate) selection: &'a [SourceUse],
    pub(crate) complete: bool,
}

impl NodeFields<'_> {
    pub(crate) fn write(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        ordinal: usize,
        node: &Node,
    ) -> Result<(), EvidenceError> {
        let original = self.sources.and_then(|sources| sources.node(ordinal));
        let fallback = ordinal == 0
            && self
                .sources
                .is_some_and(ModelNodeSources::uses_synthetic_root);
        let mut selection = self.selection.to_vec();
        if let Some(original) = original
            && let Some(table) = original.table_row()
        {
            for (field, bytes) in [
                ("type", &table.kind),
                ("version", &table.version),
                ("offset", &table.offset),
            ] {
                selection.push(table_word(self.entry, original.chunk(), field, bytes)?);
            }
        }
        let proof = RowProof {
            entry: self.entry,
            original,
            ordinal,
            selection,
            complete: self.complete && (original.is_some() || fallback),
        };
        self.write_identity(fields, path, node, &proof, fallback)?;
        proof.write_transform(fields, path)?;
        proof.write_scale(fields, path)?;
        self.write_properties(fields, path, node, &proof)
    }

    fn write_identity(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        node: &Node,
        proof: &RowProof<'_>,
        fallback: bool,
    ) -> Result<(), EvidenceError> {
        model_item_page::put(fields, path, "index", proof.origin(&[]))?;
        let mut name_uses = self.layout(proof, "nameLayout", ModelNodeMeshChunk::name_layout)?;
        if fallback {
            name_uses.push(self.entry.leaf_path()?);
        }
        name_uses.extend(proof.site(
            "name",
            proof.original.and_then(ModelNodeSource::name),
            SourceUseRole::Conversion,
        )?);
        model_item_page::put(fields, path, "name", proof.origin(&name_uses))?;
        let parent = self.parent_origin(node, proof)?;
        model_item_page::put(fields, path, "parent", parent)?;
        let geometry = proof.original.and_then(ModelNodeSource::geometry_type);
        let stored = proof.site("geometryType", geometry.clone(), SourceUseRole::Payload)?;
        model_item_page::put(
            fields,
            path,
            "geometry_type",
            proof.origin(stored.as_slice()),
        )?;
        let selected = proof.site("geometryType", geometry, SourceUseRole::Conversion)?;
        model_item_page::put(fields, path, "helper", proof.origin(selected.as_slice()))
    }

    fn parent_origin(
        &self,
        node: &Node,
        proof: &RowProof<'_>,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let original = proof.original;
        let mut uses: Vec<_> = proof
            .site(
                "parent",
                original.map(ModelNodeSource::parent),
                SourceUseRole::Conversion,
            )?
            .into_iter()
            .collect();
        let mut complete = proof.complete;
        if original.is_some_and(|source| source.table_row().is_some())
            && let Some(parent) = node.parent
        {
            let target = self.sources.and_then(|sources| sources.node(parent));
            if let Some((source, id)) = target.and_then(|source| Some((source, source.id()?))) {
                uses.push(table_word(self.entry, source.chunk(), "id", &id)?);
            } else {
                complete = false;
            }
        }
        Ok(proof.origin_with(&uses, complete))
    }

    fn write_properties(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
        node: &Node,
        proof: &RowProof<'_>,
    ) -> Result<(), EvidenceError> {
        let layout = self.layout(proof, "propertyLayout", ModelNodeMeshChunk::property_layout)?;
        let mut group = layout.clone();
        group.extend(proof.site(
            "properties",
            proof.original.and_then(ModelNodeSource::properties),
            SourceUseRole::Selector,
        )?);
        model_item_page::put(fields, path, "properties", proof.origin(&group))?;
        let mut path = path.clone();
        path.push_field("properties")?;
        let authored = proof
            .original
            .map_or(&[][..], ModelNodeSource::property_rows);
        for index in 0..node.properties.len() {
            path.push_index(index)?;
            let member = authored.get(index);
            let mut selection = layout.clone();
            selection.extend(proof.site(
                "propertyLine",
                member.map(ModelNodePropertySource::line),
                SourceUseRole::Selector,
            )?);
            let complete = proof.complete && member.is_some();
            for (name, bytes) in [
                ("key", member.map(ModelNodePropertySource::key)),
                ("value", member.map(ModelNodePropertySource::value)),
            ] {
                let mut used = selection.clone();
                used.extend(proof.site(name, bytes, SourceUseRole::Conversion)?);
                model_item_page::put(fields, &path, name, proof.origin_with(&used, complete))?;
            }
            path.pop();
        }
        Ok(())
    }

    fn layout(
        &self,
        proof: &RowProof<'_>,
        label: &str,
        field: impl Fn(&ModelNodeMeshChunk) -> Range<u64>,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        let bytes = self.chunk.filter(|_| proof.original.is_some()).map(field);
        Ok(proof
            .site(label, bytes, SourceUseRole::Selector)?
            .into_iter()
            .collect())
    }
}

struct RowProof<'a> {
    entry: &'a EntryPageSources,
    original: Option<&'a ModelNodeSource>,
    ordinal: usize,
    selection: Vec<SourceUse>,
    complete: bool,
}

impl RowProof<'_> {
    fn origin(&self, uses: &[SourceUse]) -> FieldOrigin {
        self.origin_with(uses, self.complete)
    }

    fn origin_with(&self, uses: &[SourceUse], complete: bool) -> FieldOrigin {
        let all: Vec<_> = self.selection.iter().chain(uses).cloned().collect();
        self.entry.origin_when(&all, complete)
    }

    fn site(
        &self,
        field: &str,
        bytes: Option<Range<u64>>,
        role: SourceUseRole,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        let Some((original, bytes)) = self
            .original
            .zip(bytes)
            .filter(|(_, bytes)| !bytes.is_empty())
        else {
            return Ok(None);
        };
        self.entry
            .subresource(
                SourceLocation::Subresource {
                    namespace: "cryEngineNodes".into(),
                    field: field.into(),
                    indices: vec![
                        original.chunk() as u64,
                        self.ordinal as u64,
                        bytes.start,
                        bytes.end,
                    ],
                },
                role,
            )
            .map(Some)
    }

    fn write_transform(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
    ) -> Result<(), EvidenceError> {
        let selection = self.site(
            "matrixDefault",
            self.original.map(ModelNodeSource::matrix),
            SourceUseRole::Selector,
        )?;
        let selector = selection.as_slice();
        model_item_page::put(fields, path, "transform", self.origin(selector))?;
        let mut path = path.clone();
        path.push_field("transform")?;
        for row in 0..3 {
            path.push_index(row)?;
            fields.insert(path.clone(), self.origin(selector))?;
            for column in 0..4 {
                let bytes = self
                    .original
                    .and_then(|source| source.matrix_component(row, column));
                let origin = self.word("matrixWord", bytes, selector)?;
                path.push_index(column)?;
                fields.insert(path.clone(), origin)?;
                path.pop();
            }
            path.pop();
        }
        Ok(())
    }

    fn write_scale(
        &self,
        fields: &mut FieldOrigins,
        path: &FieldPath,
    ) -> Result<(), EvidenceError> {
        let selection = self.site(
            "scaleDefault",
            self.original.and_then(ModelNodeSource::scale),
            SourceUseRole::Selector,
        )?;
        let selector = selection.as_slice();
        model_item_page::put(fields, path, "scale", self.origin(selector))?;
        let mut path = path.clone();
        path.push_field("scale")?;
        for axis in 0..3 {
            let bytes = self
                .original
                .and_then(|source| source.scale_component(axis));
            let origin = self.word("scaleWord", bytes, selector)?;
            path.push_index(axis)?;
            fields.insert(path.clone(), origin)?;
            path.pop();
        }
        Ok(())
    }

    fn word(
        &self,
        name: &str,
        bytes: Option<Range<u64>>,
        selector: &[SourceUse],
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut uses = selector.to_vec();
        uses.extend(self.site(name, bytes, SourceUseRole::Payload)?);
        Ok(self.origin(&uses))
    }
}

fn table_word(
    entry: &EntryPageSources,
    chunk: usize,
    field: &str,
    bytes: &Range<u64>,
) -> Result<SourceUse, SourceEvidenceError> {
    entry.subresource(
        SourceLocation::Subresource {
            namespace: "cryEngineChunkTable".into(),
            field: field.into(),
            indices: vec![chunk as u64, bytes.start, bytes.end],
        },
        SourceUseRole::Selector,
    )
}
