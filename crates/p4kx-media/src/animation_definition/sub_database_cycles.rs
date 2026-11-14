// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::{HashMap, TryReserveError};
use std::io;

use p4kx_extraction::{DecimalU64, InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::origins;
use super::payload::{self as wire, DefinitionPayload};
use super::sub_database_components::{Components, Graph, Work};
use super::sub_database_frontier::{Frontier, FrontierReason};
use crate::audio_control_payload::AtlPosition;
use crate::audio_record_origins as records;
use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{DirectoryContext, MediaError, MediaWriter, Status, Subresource, origin_error};

#[derive(Default)]
pub(crate) struct SubDatabaseCycles(RefCell<Observed>);

#[derive(Default)]
struct Observed {
    documents: HashMap<usize, Document>,
    edges: Vec<Edge>,
}

struct Document {
    state: DocumentState,
    proof: OriginContributions,
}

pub(super) enum DocumentState {
    Readable(AtlPosition),
    Unavailable(FrontierReason),
}

pub(super) struct Edge {
    pub(super) original: OriginContributions,
    pub(super) source: usize,
    pub(super) target: usize,
    pub(super) indices: [u64; 2],
    pub(super) evidence: OriginContributions,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SubDatabaseEdge {
    source_entry_index: DecimalU64,
    target_entry_index: DecimalU64,
    source_element_index: DecimalU64,
    source_declaration_ordinal: DecimalU64,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SubDatabaseCycle {
    representative_entry_index: DecimalU64,
    member_count: DecimalU64,
    edge_count: DecimalU64,
}

impl SubDatabaseCycles {
    pub(super) fn document(
        &self,
        entry: usize,
        input: &MediaOriginInput,
        state: DocumentState,
        size: u64,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut observed = self.0.try_borrow_mut().map_err(|_| borrowed())?;
        observed
            .documents
            .try_reserve(1)
            .map_err(|cause| allocation(&cause, writer))?;
        let proof = match state {
            DocumentState::Readable(root) => {
                origins::position(input, root, SourceUseRole::Selector, writer)?
            }
            DocumentState::Unavailable(_) => origins::file(input, size, writer)?,
        };
        if observed
            .documents
            .insert(entry, Document { state, proof })
            .is_some()
        {
            return Err(MediaError::Contract(
                "duplicate observed animation database".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn edge(&self, edge: Edge, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        let mut observed = self.0.try_borrow_mut().map_err(|_| borrowed())?;
        observed
            .edges
            .try_reserve(1)
            .map_err(|cause| allocation(&cause, writer))?;
        observed.edges.push(edge);
        Ok(())
    }

    pub(super) fn write(
        &self,
        context: &DirectoryContext<'_, '_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let observed = self.0.try_borrow().map_err(|_| borrowed())?;
        if observed.edges.is_empty() {
            return Ok(());
        }
        let owner = writer
            .origins
            .directory()
            .cloned()
            .ok_or_else(|| MediaError::Contract("SubADB graph has no owning Archive".into()))?;
        let budget = context.lookup.input_state()?.budget;
        let mut graph = Graph::default();
        let mut edges = Vec::new();
        for edge in &observed.edges {
            let work = Work::new(budget, writer);
            work.charge()?;
            let target = observed.documents.get(&edge.target).ok_or_else(|| {
                MediaError::Contract("selected SubADB has no source observation".into())
            })?;
            if !observed
                .documents
                .get(&edge.source)
                .is_some_and(|document| matches!(document.state, DocumentState::Readable(_)))
            {
                continue;
            }
            match target.state {
                DocumentState::Readable(_) => {
                    edges
                        .try_reserve(1)
                        .map_err(|cause| work.allocation(&cause))?;
                    graph.insert_edge(edge.source, edge.target, &work)?;
                    edges.push(edge);
                }
                DocumentState::Unavailable(reason) => {
                    write_target_frontier(edge, target, reason, &owner, writer)?;
                }
            }
        }
        let components = graph.components(&Work::new(budget, writer))?;
        write_graph(&observed, &graph, &components, &edges, &owner, writer)
    }
}

struct CycleEvidence {
    edges: u64,
    evidence: OriginCapture,
}

fn write_graph(
    observed: &Observed,
    graph: &Graph,
    components: &Components,
    edges: &[&Edge],
    owner: &MediaOriginInput,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut cycles = Vec::new();
    cycles
        .try_reserve_exact(components.count())
        .map_err(|cause| allocation(&cause, writer))?;
    cycles.resize_with(components.count(), || None);
    for (edge, adjacency) in edges.iter().zip(graph.edges()) {
        writer.checkpoint()?;
        let proof = write_edge(observed, edge, owner, writer)?;
        let Some(component) = components.cycle(adjacency)? else {
            continue;
        };
        let slot: &mut Option<CycleEvidence> = cycles
            .get_mut(component)
            .ok_or_else(origin_error::counter)?;
        let cycle = slot.get_or_insert_with(|| CycleEvidence {
            edges: 0,
            evidence: writer.capture_origins(),
        });
        cycle.edges = cycle
            .edges
            .checked_add(1)
            .ok_or_else(origin_error::counter)?;
        cycle.evidence.include(&proof, writer)?;
    }
    for (component, cycle) in cycles.into_iter().enumerate() {
        writer.checkpoint()?;
        if let Some(cycle) = cycle {
            write_cycle(components, component, owner, cycle, writer)?;
        }
    }
    Ok(())
}

fn write_edge(
    observed: &Observed,
    edge: &Edge,
    owner: &MediaOriginInput,
    writer: &mut MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.include(&edge.evidence, writer)?;
    for entry in [edge.source, edge.target] {
        let root = observed
            .documents
            .get(&entry)
            .ok_or_else(origin_error::counter)?;
        capture.include(&root.proof, writer)?;
    }
    let proof = capture.finish();
    let mut prepared = origins::record(owner, proof.clone(), writer)?;
    records::field(
        &mut prepared,
        "/data/payload/source_entry_index",
        entry_index(owner, edge.source, writer)?,
        writer,
    )?;
    for address in [
        "/data/payload/source_element_index",
        "/data/payload/source_declaration_ordinal",
    ] {
        records::field(&mut prepared, address, edge.original.clone(), writer)?;
    }
    let [element, declaration] = edge.indices;
    prepared.write(
        writer,
        &Subresource::new(
            wire::SUB_DATABASE_EDGE,
            vec![edge.source as u64, element, declaration],
        ),
        Status::Complete,
        &DefinitionPayload::SubDatabaseEdge(SubDatabaseEdge {
            source_entry_index: DecimalU64::new(edge.source as u64),
            target_entry_index: DecimalU64::new(edge.target as u64),
            source_element_index: DecimalU64::new(element),
            source_declaration_ordinal: DecimalU64::new(declaration),
        }),
        &[],
    )?;
    Ok(proof)
}

fn write_target_frontier(
    edge: &Edge,
    target: &Document,
    reason: FrontierReason,
    owner: &MediaOriginInput,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut capture = writer.capture_origins();
    capture.include(&edge.evidence, writer)?;
    capture.include(&target.proof, writer)?;
    let [element, declaration] = edge.indices;
    let record = Frontier::new(element, reason, Some(declaration));
    origins::record(owner, capture.finish(), writer)?.write(
        writer,
        &Subresource::new(
            wire::SUB_DATABASE_TARGET_FRONTIER,
            vec![edge.source as u64, element, declaration],
        ),
        Status::Complete,
        &DefinitionPayload::SubDatabaseFrontier(record),
        &[],
    )
}

fn write_cycle(
    components: &Components,
    component: usize,
    owner: &MediaOriginInput,
    cycle: CycleEvidence,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let group = components.group(component)?;
    let representative = group.representative as u64;
    origins::record(owner, cycle.evidence.finish(), writer)?.write(
        writer,
        &Subresource::new(wire::SUB_DATABASE_CYCLE, vec![representative]),
        Status::Complete,
        &DefinitionPayload::SubDatabaseCycle(SubDatabaseCycle {
            representative_entry_index: DecimalU64::new(representative),
            member_count: DecimalU64::new(group.members),
            edge_count: DecimalU64::new(cycle.edges),
        }),
        &[],
    )
}

fn entry_index(
    input: &MediaOriginInput,
    entry: usize,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::Payload,
        OriginSite::new("archive-directory", &[entry as u64], "entry_index"),
        SourceUseRole::Selector,
        writer,
    )?;
    Ok(capture.finish())
}

fn borrowed() -> MediaError {
    MediaError::Contract("SubADB graph is already borrowed".into())
}

pub(super) fn allocation(cause: &TryReserveError, writer: &MediaWriter<'_>) -> MediaError {
    MediaError::Io {
        path: writer.root.clone(),
        source: io::Error::new(io::ErrorKind::OutOfMemory, cause.to_string()),
    }
}
