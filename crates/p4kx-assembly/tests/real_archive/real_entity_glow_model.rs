// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_assembly::Part;
use p4kx_extraction::Archive;
use p4kx_model::{Model, ModelFile, ModelNodeSources};

use super::{TestResult, check};

pub(super) struct PrimaryModel<'a> {
    pub(super) model: &'a Model,
    pub(super) nodes: &'a ModelNodeSources,
    bytes: Vec<u8>,
}

impl<'a> PrimaryModel<'a> {
    pub(super) fn read(archive: &Archive, part: &'a Part) -> TestResult<Self> {
        let model = part.model().ok_or("original owner model unavailable")?;
        let source = part
            .model_archive_sources()
            .iter()
            .find(|row| row.file() == ModelFile::Model)
            .ok_or("primary owner model source unavailable")?;
        let origin = source.origin().ok_or("unproved primary model Entry")?;
        check(
            origin.ancestors().len() == 0 && archive.owns_origin(origin),
            "the witness requires a root model Entry",
        )?;
        let entry = archive
            .entries()
            .get(origin.entry().index())
            .ok_or("original primary model Entry missing")?;
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)
            .map_err(|_| "open original model Entry")?
            .read_to_end(&mut bytes)
            .map_err(|_| "read original model Entry")?;
        Ok(Self {
            model,
            nodes: source
                .node_sources()
                .ok_or("missing original model node coordinates")?,
            bytes,
        })
    }

    pub(super) fn matching(&self, selector: &str) -> TestResult<Vec<usize>> {
        let mut matches = Vec::new();
        for (index, node) in self.model.nodes.iter().enumerate() {
            let range = self
                .nodes
                .node(index)
                .and_then(p4kx_model::ModelNodeSource::name)
                .ok_or("missing source node name")?;
            let start = usize::try_from(range.start)?;
            let end = usize::try_from(range.end)?;
            let raw = self
                .bytes
                .get(start..end)
                .ok_or("node name lies beyond original file")?;
            let text = raw
                .split(|&byte| matches!(byte, 0 | b'\n'))
                .next()
                .ok_or("original node name extent")?;
            let name = String::from_utf8_lossy(text);
            check(
                name == node.name,
                "decoded model name differs from original range",
            )?;
            if name.eq_ignore_ascii_case(selector) {
                matches.push(index);
            }
        }
        Ok(matches)
    }
}
