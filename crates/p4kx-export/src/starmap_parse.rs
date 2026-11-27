// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use p4kx_extraction::{Archive, CancelToken, Entry, Guid, PathKeys, SourceUse};

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::entry_source::EntrySource;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_containers::jump_gate::JumpGate;
use crate::starmap_containers::location_declaration::ChildLocation;
use crate::starmap_containers::proof::ChildOrigins;
use crate::starmap_containers::unresolved::UnresolvedChild;
use crate::starmap_index::Index;
use crate::starmap_read::{self, DescriptorSurveyRead, MainXml};
use crate::starmap_xml::AuthoredPose;
use crate::{CatalogError, Kind};
#[path = "starmap_children.rs"]
mod children;

mod descriptor_scope;
pub(crate) use descriptor_scope::DescriptorScope;

mod descriptor_frontier;
#[cfg(test)]
pub(crate) use descriptor_frontier::read_all;
pub(crate) use descriptor_frontier::{Read, read_all_scoped};

mod placement_survey;
use placement_survey::DescriptorObservations;
pub(crate) use placement_survey::{DescriptorSurvey, PlacementSurvey};

#[path = "starmap_descriptor_root.rs"]
mod descriptor_root;

#[path = "starmap_external_sources.rs"]
mod external_sources;
#[path = "starmap_failed_links.rs"]
mod failed_links;

#[path = "starmap_hcd.rs"]
pub(crate) mod hcd;

#[path = "starmap_entxml.rs"]
pub(crate) mod entxml;

const CHILDREN: &str = "ChildObjectContainers";
const CHILD: &str = "Child";

/// The position and rotation of a Child, or the clipped attribute texts of one that has none.
pub(crate) enum Pose {
    Valid(AuthoredPose),
    Malformed {
        pos: String,
        rot: String,
        entity: String,
    },
}

/// One Child element of a container.
pub(crate) struct Child {
    pub(crate) pose: Pose,
    pub(crate) record: Option<Guid>,
    /// The path of the external container the Child names.
    pub(crate) external: Option<AuthoredEntryPath>,
    pub(crate) unresolved_external: Option<Arc<UnresolvedChild>>,
    pub(crate) deferred_external: Option<Box<crate::starmap_containers::unresolved::DeferredChild>>,
    pub(crate) origins: Option<Arc<ChildOrigins>>,
    pub(crate) location: Option<Arc<ChildLocation>>,
    pub(crate) below: Level,
}

impl Child {
    pub(crate) fn proves_external_absence(
        &self,
        archive: &Archive,
        path: &AuthoredEntryPath,
        candidates: &[usize],
    ) -> bool {
        self.unresolved_external
            .as_ref()
            .is_some_and(|declaration| declaration.proves_missing_target(archive, path, candidates))
    }

    pub(crate) fn entity_name(&self) -> &str {
        self.origins
            .as_deref()
            .and_then(ChildOrigins::entity_name)
            .unwrap_or_default()
    }
}

/// The Children of one element.
#[derive(Default)]
pub(crate) struct Level {
    pub(crate) children: Vec<Child>,
    /// The list holds more Children than the budget of the parse allowed; the rest was not read.
    pub(crate) truncated: bool,
}

impl Drop for Level {
    fn drop(&mut self) {
        if self.children.is_empty() {
            return;
        }
        let mut children = mem::take(&mut self.children).into_iter();
        let mut parents = Vec::new();
        loop {
            if let Some(mut child) = children.next() {
                if !child.below.children.is_empty() {
                    let below = mem::take(&mut child.below.children);
                    if !children.as_slice().is_empty() {
                        parents.push(children);
                    }
                    children = below.into_iter();
                }
            } else if let Some(parent) = parents.pop() {
                children = parent;
            } else {
                return;
            }
        }
    }
}

/// What reading one container found.
pub(crate) enum Body {
    Unreadable(String),
    Unparsed,
    Parsed {
        root: Level,
        pending_external: bool,
        file: Box<ContainerFile>,
        fallback: Option<String>,
        gate: Option<Box<JumpGate>>,
        gate_proof: Option<crate::catalog_read::original::XmlNodeSource>,
    },
}

/// The shared Child count and read controls for the discovered containers.
struct Bounds<'a> {
    cancel: Option<&'a CancelToken>,
    children: Option<AtomicUsize>,
    scope: DescriptorScope,
    directory: Option<SourceUse>,
    location_grid_paths: OnceLock<PathKeys>,
}

impl Bounds<'_> {
    fn take(&self) -> bool {
        self.children.as_ref().is_none_or(|remaining| {
            remaining
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |left| {
                    left.checked_sub(1)
                })
                .is_ok()
        })
    }
}

struct DescriptorRead {
    body: Body,
    ledger: ExportLedger,
    observations: DescriptorObservations,
}

/// Reads and parses the container at `entry`.
fn parse(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    bounds: &Bounds<'_>,
    capture: &EntryCapture<'_>,
) -> Result<DescriptorRead, CatalogError> {
    let mut ledger = ExportLedger::default();
    let mut observations = DescriptorObservations::new(bounds.scope.captures_loot_survey());
    let body = parse_body(
        archive,
        index,
        entry,
        bounds,
        capture,
        &mut ledger,
        &mut observations,
    )?;
    Ok(DescriptorRead {
        body,
        ledger,
        observations,
    })
}

fn parse_body(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    bounds: &Bounds<'_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    observations: &mut DescriptorObservations,
) -> Result<Body, CatalogError> {
    let Some(socpak) = index.entries.get(entry) else {
        ledger.incomplete_inventory(Kind::StarmapObjects);
        return Ok(Body::Unreadable("no such entry".to_owned()));
    };
    let opened = match &mut observations.survey {
        Some(inputs) => starmap_read::main_xml_with_survey(
            archive,
            socpak,
            capture,
            ledger,
            Some(DescriptorSurveyRead {
                inputs,
                cancel: bounds.cancel,
            }),
        ),
        None => starmap_read::main_xml(archive, socpak, capture, ledger),
    }?;
    let main = match opened {
        Ok(main) => main,
        Err(reason) => {
            ledger.incomplete_inventory(Kind::StarmapObjects);
            return Ok(Body::Unreadable(reason));
        }
    };
    parse_descriptor(
        archive,
        index,
        (socpak, main),
        bounds,
        capture,
        ledger,
        observations,
    )
}

fn parse_descriptor(
    archive: &Archive,
    index: &Index<'_>,
    (socpak, main): (&Entry, MainXml),
    bounds: &Bounds<'_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    observations: &mut DescriptorObservations,
) -> Result<Body, CatalogError> {
    let files = entxml::EntXmlFiles {
        archive,
        outer: socpak,
        container: &main.container,
        capture,
        cancel: bounds.cancel,
    };
    let Ok(descriptor_root::ParsedRoot {
        mut root,
        file,
        entities,
        gate,
        gate_proof,
        pending_external,
        name_inputs_complete: observed_name_inputs,
        survey_document,
    }) = descriptor_root::read(&main.bytes, bounds, &files, index, ledger)?
    else {
        observations.observe_payload(&main.bytes, None)?;
        ledger.incomplete_inventory(Kind::StarmapObjects);
        main.source.record(
            ledger,
            Kind::StarmapObjects,
            SourceDisposition::ProjectionFailed,
        );
        return Ok(Body::Unparsed);
    };
    let loss = children::source_loss(&mut root, &entities, capture.source_uses(), bounds.cancel)?;
    observations.name_inputs_complete = observed_name_inputs && !loss.projection;
    observations.observe_payload(&main.bytes, survey_document)?;
    let grid_failed = bounds.scope.reads_location_physics()
        && matches!(
            file.location_physics,
            crate::location_physics::GridRead::Unavailable
        );
    record_projection(ledger, main.source, &loss, grid_failed);
    Ok(Body::Parsed {
        root,
        file: Box::new(file),
        pending_external,
        fallback: main.fallback,
        gate,
        gate_proof,
    })
}

fn record_projection(
    ledger: &mut ExportLedger,
    source: EntrySource,
    loss: &children::SourceLoss,
    grid_failed: bool,
) {
    if loss.inventory || grid_failed {
        ledger.incomplete_inventory(Kind::StarmapObjects);
    }
    let disposition = if loss.projection || grid_failed {
        SourceDisposition::ProjectionFailed
    } else {
        SourceDisposition::Projected
    };
    source.record(ledger, Kind::StarmapObjects, disposition);
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "starmap_parse/installed.rs"]
pub(crate) mod installed;
