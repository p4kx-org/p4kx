// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken, Guid, SourceUse, SourceValue};

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::entry_source::EntrySource;
use crate::starmap_containers::Limits;
use crate::starmap_containers::instance::{ContainerFile, ContainerInstance, ContainerOccurrence};
use crate::starmap_containers::proof::{PlacementProof, PlacementStep};
use crate::starmap_index::Index;
use crate::starmap_parse::{Body, Child, Level, Pose};
use crate::starmap_report::{Placement, StarmapReport, clip};
use crate::starmap_transform::Transform;
use crate::starmap_xml::AuthoredPose;
use crate::{CatalogError, ExportAccountingError};

/// The most lines the report keeps per star system of the containers read, and of the issues.
const REPORT_LINES: usize = 300;

/// The longest line the report keeps.
const LINE_LEN: usize = 300;

#[derive(Default)]
pub(crate) struct SystemRead {
    pub(crate) placed: Vec<(Guid, Placement)>,
    pub(crate) unreadable: usize,
    pub(crate) missing: usize,
    pub(crate) malformed: usize,
    pub(crate) named: usize,
    pub(crate) opened: Vec<usize>,
    pub(crate) deepest: usize,
    pub(crate) report: StarmapReport,
    pub(crate) containers: Vec<Opened>,
    pub(crate) instances: Vec<ContainerInstance>,
}

/// A container the walk opened and the starmap record it hangs below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Opened {
    pub(crate) system: Arc<str>,
    pub(crate) entry: usize,
    pub(crate) under: Option<Guid>,
}

#[derive(Clone)]
struct WalkPath {
    transform: Transform,
    under: Option<Guid>,
    external: usize,
    route: Option<Arc<PlacementStep>>,
    parent_instance: Option<Arc<PlacementStep>>,
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
        local: None,
        via: None,
    };

    fn child(&self, child: &Child, pose: AuthoredPose, transform: Transform) -> Self {
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
            local: Some(pose),
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

/// The walk through one star system, over containers that are already parsed.
pub(crate) struct System<'a> {
    index: &'a Index<'a>,
    bodies: &'a HashMap<usize, Body>,
    cancel: Option<&'a CancelToken>,
    name: Arc<str>,
    root: Arc<[SourceUse]>,
    root_entry: u32,
    capture: &'a EntryCapture<'a>,
    read: SystemRead,
    limits: Limits,
    budget: usize,
    open: Vec<usize>,
    seen: [usize; 3],
}

impl<'a> System<'a> {
    pub(crate) fn run(
        index: &'a Index<'a>,
        bodies: &'a HashMap<usize, Body>,
        name: &Arc<str>,
        entry: usize,
        cancel: Option<&'a CancelToken>,
        limits: Limits,
        (archive, capture): (&Archive, &'a EntryCapture<'a>),
    ) -> Result<SystemRead, CatalogError> {
        let root = index
            .entries
            .get(entry)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let source = EntrySource::outer(capture, archive, root)?;
        let directory = source.receipt().physical_reference(capture)?;
        let SourceValue::Present(reference) = directory.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let mut system = Self {
            index,
            bodies,
            cancel,
            name: Arc::clone(name),
            root: Arc::from(directory.contributions().0),
            root_entry: reference.entry_index,
            capture,
            read: SystemRead::default(),
            limits,
            budget: limits.children,
            open: Vec::new(),
            seen: [0; 3],
        };
        system.container(entry, &WalkPath::ROOT)?;
        Ok(system.read)
    }

    /// Keeps `line` in the report of the issues while there is room.
    fn issue(&mut self, line: &str) {
        if self.read.report.issues.len() < REPORT_LINES {
            self.read
                .report
                .issues
                .push(line.chars().take(LINE_LEN).collect());
        }
    }

    /// Counts a Child, or a limit, that left objects without a position, and reports it.
    fn skipped(&mut self, line: &str) {
        self.read.malformed += 1;
        self.issue(line);
    }

    /// Notes that the container at `entry`, `external` containers below the system container,
    /// was opened and hangs below the record `under`.
    fn opened(&mut self, entry: usize, under: Option<Guid>, external: usize) {
        self.read.opened.push(entry);
        self.read.containers.push(Opened {
            system: Arc::clone(&self.name),
            entry,
            under,
        });
        self.read.deepest = self.read.deepest.max(external);
    }

    /// Walks the original Children of one readable container instance.
    fn container(&mut self, entry: usize, path: &WalkPath) -> Result<(), CatalogError> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Ok(());
        }
        let label = self
            .index
            .entries
            .get(entry)
            .map_or_else(String::new, |socpak| socpak.path().to_owned());
        if self.open.contains(&entry) {
            self.skipped(&format!("{label}: names a container that is open above it"));
            return Ok(());
        }
        let bodies = self.bodies;
        let Some(body) = bodies.get(&entry) else {
            self.read.unreadable += 1;
            self.issue(&format!("{label}: not read"));
            return Ok(());
        };
        let before = self.seen;
        match body {
            Body::Unreadable(reason) => {
                self.read.unreadable += 1;
                self.issue(&format!("{label}: {reason}"));
                return Ok(());
            }
            Body::Unparsed => {
                self.opened(entry, path.under, path.external);
                self.read.unreadable += 1;
                self.issue(&format!("{label}: the XML cannot be parsed"));
                return Ok(());
            }
            Body::Parsed {
                root,
                file,
                fallback,
            } => {
                self.opened(entry, path.under, path.external);
                self.record_instance(path, file)?;
                if let Some(taken) = fallback {
                    self.issue(&format!(
                        "{label}: no XML named like the container, read {} instead",
                        clip(taken)
                    ));
                }
                self.open.push(entry);
                self.children(root, &path.within_container(), &label)?;
                self.open.pop();
            }
        }
        self.report_placements(before, &label);
        Ok(())
    }

    fn report_placements(&mut self, before: [usize; 3], label: &str) {
        if self.seen[1] > before[1] {
            self.read.named += 1;
            if self.read.report.containers.len() < REPORT_LINES {
                self.read.report.containers.push(format!(
                    "{label}: {} children, {} name a record, {} external",
                    self.seen[0] - before[0],
                    self.seen[1] - before[1],
                    self.seen[2] - before[2]
                ));
            }
        }
    }

    fn record_instance(
        &mut self,
        path: &WalkPath,
        file: &ContainerFile,
    ) -> Result<(), CatalogError> {
        let instance = ContainerOccurrence {
            system: &self.name,
            root_entry: self.root_entry,
            root_inputs: &self.root,
            route: path.route.as_deref(),
            parent_route: path.parent_instance.as_deref(),
            declared: path.via.as_ref(),
            local: path.local,
            world: path.transform,
        };
        self.read
            .instances
            .push(instance.project(file, self.capture)?);
        Ok(())
    }

    fn children(
        &mut self,
        level: &Level,
        parent: &WalkPath,
        label: &str,
    ) -> Result<(), CatalogError> {
        if level.capped {
            let limit = self.limits.depth;
            self.skipped(&format!("{label}: Children nested deeper than {limit}"));
            return Ok(());
        }
        for child in &level.children {
            if self.budget == 0 {
                let limit = self.limits.children;
                self.skipped(&format!("{label}: more than {limit} Children in a system"));
                return Ok(());
            }
            self.budget -= 1;
            self.seen[0] += 1;
            let pose = match &child.pose {
                Pose::Valid(pose) => *pose,
                Pose::Malformed { pos, rot, entity } => {
                    self.skipped(&format!(
                        "{label}: malformed pos {pos} rot {rot} of {entity}"
                    ));
                    continue;
                }
            };
            let local = pose.transform();
            let world = parent.transform.place(local.position, local.rotation);
            if !world.is_sane() {
                self.skipped(&format!(
                    "{label}: out of range: pos {:?} places {:?} of {}",
                    local.position,
                    world.position,
                    clip(&child.entity_name)
                ));
                continue;
            }
            let child_path = parent.child(child, pose, world);
            self.note(child, &child_path, parent.under)?;
            if let Some(path) = &child.external {
                self.follow(path, &child_path, label)?;
            }
            self.children(&child.below, &child_path, label)?;
        }
        if level.truncated {
            let limit = self.limits.children;
            self.skipped(&format!(
                "{label}: more than {limit} Children in the containers of the Archive"
            ));
        }
        Ok(())
    }

    fn follow(
        &mut self,
        path: &AuthoredEntryPath,
        parent: &WalkPath,
        label: &str,
    ) -> Result<(), CatalogError> {
        self.seen[2] += 1;
        let declared = path.as_str();
        if parent.external >= self.limits.external {
            let limit = self.limits.external;
            self.skipped(&format!(
                "{label}: external containers nested deeper than {limit}: {}",
                clip(declared)
            ));
        } else if let Some(entry) = self.index.find(declared) {
            self.container(entry, &parent.external(path))?;
        } else {
            self.read.missing += 1;
            self.issue(&format!(
                "{label}: the Archive holds no container {}",
                clip(declared)
            ));
        }
        Ok(())
    }

    fn note(
        &mut self,
        child: &Child,
        path: &WalkPath,
        under: Option<Guid>,
    ) -> Result<(), CatalogError> {
        let Some(record) = child.record else {
            return Ok(());
        };
        let last = path
            .route
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.seen[1] += 1;
        self.read.placed.push((
            record,
            Placement {
                system: Arc::clone(&self.name),
                position: path.transform.position,
                entity_name: child.entity_name.clone(),
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
