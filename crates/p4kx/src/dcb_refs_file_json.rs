// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, hash_map::Entry};
use std::io::Write;
use std::path::Path;

use anyhow::{Context as _, Result};
use p4kx_extraction::{
    FieldOrigins, ObservedFileEdge, ObservedFileGraph, SourceInventory, SourceUseRole,
};
use serde::Serialize;

use super::origins::{CountEvidence, HeaderField, Origins};
use crate::json;

#[derive(Serialize)]
struct EdgeJson<'source> {
    from: &'source str,
    to: &'source str,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
}

#[derive(Serialize)]
struct FilesJson<'source> {
    archive: String,
    file_count: usize,
    edge_count: usize,
    edges: Vec<EdgeJson<'source>>,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn write(
    out: &mut impl Write,
    archive: &Path,
    targets: &[u32],
    origins: &Origins<'_, '_>,
) -> Result<()> {
    let graph = origins.file_graph()?;
    let filter = FileFilter::read(&graph, targets, origins)?;
    let file_count = origins.file_scan(&graph)?;
    let mut edge_count = origins.whole_scan()?;
    edge_count.include(&file_count)?;
    if let FileFilter::Selected { evidence, .. } = &filter {
        edge_count.include(evidence)?;
    }
    let mut edges = Vec::new();
    let mut displays = FileDisplays::default();
    for edge in graph.edges() {
        if let Some(selected) = filter.select(edge.files()) {
            let prepared = edge_row(&graph, &edge, selected, origins, &mut displays)?;
            edge_count.include(&prepared.evidence)?;
            edges.try_reserve(1)?;
            edges.push(prepared.row);
        }
    }
    let mut fields = FieldOrigins::new();
    file_count.field(origins, &mut fields, "/file_count")?;
    edge_count.field(origins, &mut fields, "/edge_count")?;
    Ok(super::line(
        out,
        &FilesJson {
            archive: json::path(archive),
            file_count: graph.paths().len(),
            edge_count: edges.len(),
            edges,
            field_origins: fields,
            provenance: origins.inventory()?,
        },
    )?)
}

enum FileFilter {
    All,
    Selected {
        records: HashMap<u32, u32>,
        evidence: CountEvidence,
    },
}

#[derive(Clone, Copy)]
enum EdgeSelection {
    All,
    Requested(u32),
}

impl FileFilter {
    fn read(
        graph: &ObservedFileGraph<'_, '_>,
        targets: &[u32],
        origins: &Origins<'_, '_>,
    ) -> Result<Self> {
        if targets.is_empty() {
            return Ok(Self::All);
        }
        let mut records = HashMap::new();
        let mut evidence = CountEvidence::empty();
        for &record in targets {
            if let Some(file) = graph.position_of_record(record) {
                records.try_reserve(1)?;
                records.entry(file).or_insert(record);
                evidence.add(origins.header(
                    record,
                    HeaderField::FileName,
                    SourceUseRole::Selector,
                )?)?;
            } else {
                origins.header(record, HeaderField::FileName, SourceUseRole::Attempted)?;
                evidence.incomplete();
            }
        }
        Ok(Self::Selected { records, evidence })
    }

    fn select(&self, (from, to): (u32, u32)) -> Option<EdgeSelection> {
        match self {
            Self::All => Some(EdgeSelection::All),
            Self::Selected { records, .. } => records
                .get(&from)
                .or_else(|| records.get(&to))
                .copied()
                .map(EdgeSelection::Requested),
        }
    }
}

struct PreparedEdge<'source> {
    row: EdgeJson<'source>,
    evidence: CountEvidence,
}

fn edge_row<'source>(
    graph: &ObservedFileGraph<'_, 'source>,
    edge: &ObservedFileEdge<'_, 'source>,
    selection: EdgeSelection,
    origins: &Origins<'_, 'source>,
    displays: &mut FileDisplays,
) -> Result<PreparedEdge<'source>> {
    let (from, to) = edge.files();
    let mut evidence = edge_evidence(edge, selection, origins)?;
    let mut fields = FieldOrigins::new();
    for (path, file) in [("/from", from), ("/to", to)] {
        let display = displays.evidence(graph, file, origins)?;
        evidence.field_with(origins, &mut fields, path, display)?;
    }
    let path = |file: u32| {
        graph
            .paths()
            .get(file as usize)
            .copied()
            .context("original file path")
    };
    Ok(PreparedEdge {
        row: EdgeJson {
            from: path(from)?,
            to: path(to)?,
            field_origins: fields,
        },
        evidence,
    })
}

#[derive(Default)]
struct FileDisplays {
    files: HashMap<u32, CountEvidence>,
}

impl FileDisplays {
    fn evidence(
        &mut self,
        graph: &ObservedFileGraph<'_, '_>,
        file: u32,
        origins: &Origins<'_, '_>,
    ) -> Result<&CountEvidence> {
        if !self.files.contains_key(&file) {
            self.files.try_reserve(1)?;
        }
        Ok(match self.files.entry(file) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(display_evidence(graph, file, origins)?),
        })
    }
}

fn display_evidence(
    graph: &ObservedFileGraph<'_, '_>,
    file: u32,
    origins: &Origins<'_, '_>,
) -> Result<CountEvidence> {
    let representative = graph
        .representative(file)
        .context("original file representative")?;
    let mut evidence = CountEvidence::empty();
    evidence.add(origins.header(
        representative.index(),
        HeaderField::FileName,
        SourceUseRole::Conversion,
    )?)?;
    let members = graph.member_records(file).context("original file group")?;
    if members.len() > 1 {
        for member in members {
            let record = member?;
            evidence.add(origins.header(
                record.index(),
                HeaderField::FileName,
                SourceUseRole::Selector,
            )?)?;
            let role = record.stored_name().map_or_else(
                |_| {
                    evidence.incomplete();
                    SourceUseRole::Attempted
                },
                |name| {
                    name.fold(
                        || SourceUseRole::Conversion,
                        || SourceUseRole::Conversion,
                        |_| SourceUseRole::Selector,
                    )
                },
            );
            let name = origins.header(record.index(), HeaderField::Name, role)?;
            if role != SourceUseRole::Attempted {
                evidence.add(name)?;
            }
        }
    }
    Ok(evidence)
}

fn edge_evidence(
    edge: &ObservedFileEdge<'_, '_>,
    selection: EdgeSelection,
    origins: &Origins<'_, '_>,
) -> Result<CountEvidence> {
    let mut evidence = CountEvidence::empty();
    if let EdgeSelection::Requested(record) = selection {
        evidence.add(origins.header(record, HeaderField::FileName, SourceUseRole::Selector)?)?;
    }
    for witness in edge.witnesses() {
        let target = witness
            .target()
            .context("a file edge requires its original resolved Reference")?;
        evidence.add(origins.relation(witness)?)?;
        for record in [witness.source(), target] {
            evidence.add(origins.header(
                record,
                HeaderField::FileName,
                SourceUseRole::Selector,
            )?)?;
        }
    }
    Ok(evidence)
}
