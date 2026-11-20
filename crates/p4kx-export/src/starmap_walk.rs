// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken, Guid, SourceUse, SourcedValue};

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, PhysicalEntryRef};
use crate::catalog_rows::spool::RowSpoolWriter;
use crate::harvest_locations::HarvestSurvey;
use crate::starmap_containers::Limits;
use crate::starmap_containers::instance::ContainerInstance;
use crate::starmap_containers::jump_class::JumpEndpointRecords;
use crate::starmap_containers::jump_endpoint::JumpEndpoint;
#[cfg(test)]
use crate::starmap_containers::location_instance::LocationInstance;
use crate::starmap_containers::proof::{PlacementProof, PlacementStep};
use crate::starmap_containers::unresolved::UnresolvedContainerLink;
use crate::starmap_index::Index;
use crate::starmap_parse::{Body, Child};
use crate::starmap_report::{Placement, StarmapReport};
use crate::starmap_transform::Transform;
use crate::starmap_xml::AuthoredPose;
use crate::{CatalogError, ExportAccountingError, HealthText};

#[path = "starmap_walk_frames.rs"]
mod frames;

#[path = "starmap_walk_location.rs"]
mod location;

#[path = "starmap_walk_unresolved.rs"]
mod unresolved;

#[path = "starmap_walk_jump.rs"]
mod jump;

#[path = "starmap_walk_report.rs"]
mod report;

#[path = "starmap_walk_run.rs"]
mod run;

#[derive(Default)]
pub(crate) struct SystemRead {
    pub(crate) placed: Vec<(Guid, Placement)>,
    pub(crate) unreadable: usize,
    pub(crate) missing: usize,
    pub(crate) verified_missing: usize,
    pub(crate) malformed: usize,
    pub(crate) routes_incomplete: bool,
    pub(crate) placement_budget_exhausted: bool,
    pub(crate) named: usize,
    pub(crate) deepest: usize,
    pub(crate) report: StarmapReport,
    pub(crate) containers: Vec<Opened>,
    pub(crate) harvest_occurrences: Vec<HarvestOccurrence>,
    pub(crate) harvest_reached: HashSet<usize>,
    #[cfg(test)]
    pub(crate) instances: Vec<ContainerInstance>,
    #[cfg(test)]
    pub(crate) locations: Vec<LocationInstance>,
    #[cfg(test)]
    pub(crate) unresolved: Vec<UnresolvedContainerLink>,
    pub(crate) jump_endpoints: Vec<JumpEndpoint>,
    pub(crate) location_physics: crate::location_physics::GridObservations,
}

/// A container the walk opened and the starmap record it hangs below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Opened {
    pub(crate) system: Arc<str>,
    pub(crate) entry: usize,
    pub(crate) under: Option<Guid>,
}

/// A reached container and its detached original route.
#[derive(Debug)]
pub(crate) struct HarvestOccurrence {
    pub(crate) opened: Opened,
    pub(crate) root_entry: u32,
    pub(crate) root_reference: PhysicalEntryRef,
    pub(crate) root_inputs: Arc<[SourceUse]>,
    pub(crate) parent_route: Option<Arc<PlacementStep>>,
    pub(crate) declared: Option<AuthoredEntryPath>,
    pub(crate) route: Option<Arc<PlacementStep>>,
}

#[derive(Clone)]
struct WalkPath {
    transform: Transform,
    under: Option<Guid>,
    external: usize,
    route: Option<Arc<PlacementStep>>,
    parent_instance: Option<Arc<PlacementStep>>,
    parent_location: Option<Arc<PlacementStep>>,
    local: Option<AuthoredPose>,
    via: Option<AuthoredEntryPath>,
}

impl WalkPath {
    const ROOT: Self = Self {
        transform: Transform::IDENTITY,
        under: None,
        external: 0,
        route: None,
        parent_instance: None,
        parent_location: None,
        local: None,
        via: None,
    };

    fn child(&self, child: &Child, pose: Option<AuthoredPose>, transform: Transform) -> Self {
        let route = child.origins.as_ref().map(|origins| {
            Arc::new(PlacementStep {
                preceding: self.route.clone(),
                via: self.via.clone(),
                child: Arc::clone(origins),
                has_record: child.record.is_some(),
            })
        });
        Self {
            transform,
            under: child.record.or(self.under),
            external: self.external,
            route,
            parent_instance: self.parent_instance.clone(),
            parent_location: self.parent_location.clone(),
            local: pose,
            via: None,
        }
    }

    fn external(&self, via: &AuthoredEntryPath) -> Self {
        Self {
            external: self.external + 1,
            via: Some(via.clone()),
            ..self.clone()
        }
    }

    fn within_container(&self) -> Self {
        Self {
            parent_instance: self.route.clone(),
            ..self.clone()
        }
    }
}

pub(crate) struct SelectedContainerRows {
    pub(crate) child_node_indices: Option<Vec<u32>>,
    pub(crate) external_child_step_indices: Option<Vec<usize>>,
    pub(crate) instances: Vec<ContainerInstance>,
    pub(crate) unresolved: Vec<UnresolvedContainerLink>,
    pub(crate) location_physics: crate::location_physics::GridObservations,
}

impl SelectedContainerRows {
    pub(crate) fn new(
        child_node_indices: Option<Vec<u32>>,
        external_child_step_indices: Option<Vec<usize>>,
    ) -> Self {
        Self {
            child_node_indices,
            external_child_step_indices,
            instances: Vec::new(),
            unresolved: Vec::new(),
            location_physics: crate::location_physics::GridObservations::default(),
        }
    }

    fn matches(&self, route: Option<&PlacementStep>) -> bool {
        let Some(indices) = &self.child_node_indices else {
            return true;
        };
        let steps = || route.into_iter().flat_map(PlacementStep::steps);
        steps()
            .map(PlacementStep::child_index)
            .eq(indices.iter().copied().rev())
            && self
                .external_child_step_indices
                .as_ref()
                .is_none_or(|expected| {
                    (0..indices.len())
                        .rev()
                        .zip(steps())
                        .filter_map(|(index, step)| step.via.as_ref().map(|_| index))
                        .eq(expected.iter().copied().rev())
                })
    }
}

pub(crate) enum PlacementRows<'writer> {
    Spool {
        containers: &'writer mut RowSpoolWriter,
        locations: &'writer mut RowSpoolWriter,
        unresolved: &'writer mut RowSpoolWriter,
    },
    #[cfg(test)]
    Typed,
    Harvest(&'writer HarvestSurvey<'writer>),
    SelectedContainers(&'writer mut SelectedContainerRows),
}

impl PlacementRows<'_> {
    fn includes_container(&self, route: Option<&PlacementStep>) -> bool {
        match self {
            Self::SelectedContainers(rows) => rows.matches(route),
            Self::Harvest(_) => false,
            Self::Spool { .. } => true,
            #[cfg(test)]
            Self::Typed => true,
        }
    }
}

/// The walk through one star system, over containers that are already parsed.
pub(crate) struct System<'a> {
    archive: &'a Archive,
    index: &'a Index<'a>,
    bodies: &'a HashMap<usize, Body>,
    cancel: Option<&'a CancelToken>,
    name: Arc<str>,
    name_cell: SourcedValue<HealthText>,
    root: Arc<[SourceUse]>,
    root_entry: u32,
    root_reference: PhysicalEntryRef,
    capture: &'a EntryCapture<'a>,
    endpoints: Option<&'a JumpEndpointRecords>,
    read: SystemRead,
    rows: PlacementRows<'a>,
    limits: Limits,
    budget: Option<usize>,
    open: HashSet<usize>,
    seen: [usize; 3],
}

impl System<'_> {
    /// Counts a Child, or a limit, that left objects without a position, and reports it.
    fn skipped(&mut self, line: &str) {
        self.read.malformed += 1;
        self.issue(line);
    }

    fn skip_route(&mut self, line: &str) {
        self.read.routes_incomplete = true;
        self.skipped(line);
    }

    const fn note_blocked_descendants(&mut self, child: &Child) {
        self.read.routes_incomplete |=
            child.external.is_some() || !child.below.children.is_empty() || child.below.truncated;
    }

    /// Notes that the container at `entry`, `external` containers below the system container,
    /// was opened and hangs below the record `under`.
    fn opened(&mut self, entry: usize, path: &WalkPath) -> Result<(), CatalogError> {
        let opened = Opened {
            system: Arc::clone(&self.name),
            entry,
            under: path.under,
        };
        if let PlacementRows::Harvest(harvest) = &self.rows {
            self.read.harvest_reached.insert(entry);
            if harvest
                .probe(self.archive, entry, self.capture, self.cancel)?
                .is_some()
            {
                self.read.harvest_occurrences.push(HarvestOccurrence {
                    opened: opened.clone(),
                    root_entry: self.root_entry,
                    root_reference: self.root_reference.clone(),
                    root_inputs: Arc::clone(&self.root),
                    parent_route: path.parent_instance.clone(),
                    declared: path.via.clone(),
                    route: path.route.clone(),
                });
                self.read.containers.push(opened);
            }
        } else if !matches!(&self.rows, PlacementRows::SelectedContainers(_)) {
            self.read.containers.push(opened);
        }
        self.read.deepest = self.read.deepest.max(path.external);
        Ok(())
    }

    fn note(
        &mut self,
        child: &Child,
        path: &WalkPath,
        under: Option<Guid>,
    ) -> Result<(), CatalogError> {
        if matches!(&self.rows, PlacementRows::SelectedContainers(_)) {
            return Ok(());
        }
        let Some(record) = child.record else {
            return Ok(());
        };
        let last = path
            .route
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.seen[1] = self.seen[1]
            .checked_add(1)
            .ok_or(ExportAccountingError::CounterOverflow)?;
        self.read.placed.push((
            record,
            Placement {
                system: Arc::clone(&self.name),
                position: path.transform.position,
                entity_name: child.entity_name().to_owned(),
                placed_under: under,
                proof: PlacementProof {
                    root: Arc::clone(&self.root),
                    last: Arc::clone(last),
                },
            },
        ));
        Ok(())
    }
}
