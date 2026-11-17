// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Archive, CancelToken, Guid, SourceValue, SourcedValue};

use super::checkpoint;
use crate::catalog_read::original::{EntryCapture, EntryReceipt, OriginalDatum, PhysicalEntryRef};
use crate::harvest_locations::Objects;
use crate::mining_locations::{
    ContainerInstanceKey, HarvestableLocation, HarvestableOccurrence, HarvestableOccurrenceRoute,
    ReachedHarvestableOccurrence,
};
use crate::starmap_containers::proof::{PlacementStep, container_route_uses};
use crate::starmap_walk::{HarvestOccurrence, Opened};
use crate::{CatalogError, ExportAccountingError};

pub(super) struct HarvestLocationsRead {
    pub(super) logical: Vec<HarvestableLocation>,
    pub(super) physical: Vec<HarvestableOccurrence>,
}

pub(super) fn project(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    reached: &[HarvestOccurrence],
    opened: &[Opened],
    positives: &HashMap<usize, SourcedValue<Guid>>,
    objects: &Objects,
) -> Result<Vec<HarvestableOccurrence>, CatalogError> {
    let mut routes = Vec::with_capacity(reached.len());
    let mut reached_entries = HashSet::new();
    for occurrence in reached {
        checkpoint(cancel)?;
        let entry = occurrence.opened.entry;
        let preset = positives
            .get(&entry)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let receipt = container(archive, capture, entry)?;
        let reference = receipt.physical_reference(capture)?;
        let route =
            HarvestableOccurrenceRoute::Reached(reached_route(occurrence, &reference, capture)?);
        routes.push(project_row(
            &occurrence.opened,
            preset,
            &receipt,
            reference,
            route,
            objects,
            capture,
        )?);
        reached_entries.insert(entry);
    }
    for container_entry in opened {
        checkpoint(cancel)?;
        let entry = container_entry.entry;
        if reached_entries.contains(&entry) {
            continue;
        }
        if let Some(preset) = positives.get(&entry) {
            let receipt = container(archive, capture, entry)?;
            let reference = receipt.physical_reference(capture)?;
            routes.push(project_row(
                container_entry,
                preset,
                &receipt,
                reference,
                HarvestableOccurrenceRoute::Unreached,
                objects,
                capture,
            )?);
        }
    }
    Ok(routes)
}

fn container(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    index: usize,
) -> Result<EntryReceipt, CatalogError> {
    let entry = archive
        .entries()
        .get(index)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    EntryReceipt::outer(capture, archive, entry)
}

fn project_row(
    opened: &Opened,
    preset_guid: &SourcedValue<Guid>,
    receipt: &EntryReceipt,
    reference: OriginalDatum<PhysicalEntryRef>,
    route: HarvestableOccurrenceRoute,
    objects: &Objects,
    capture: &EntryCapture<'_>,
) -> Result<HarvestableOccurrence, CatalogError> {
    let sources = capture.source_uses();
    Ok(HarvestableOccurrence {
        preset_guid: preset_guid.clone(),
        container: reference.into_cell(sources),
        container_path: receipt
            .physical_path()?
            .into_cell(sources)
            .map(|path| path.as_str().to_owned()),
        system: opened.system.to_string(),
        starmap_object_guid: objects.location(opened.under).ok().map(|guid| guid.0),
        route,
    })
}

fn reached_route(
    occurrence: &HarvestOccurrence,
    container: &OriginalDatum<PhysicalEntryRef>,
    capture: &EntryCapture<'_>,
) -> Result<ReachedHarvestableOccurrence, CatalogError> {
    let sources = capture.source_uses();
    let route = occurrence.route.as_deref();
    if occurrence.declared.is_some() && route.is_none() {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let (mut uses, complete) =
        container_route_uses(&occurrence.root_inputs, route, occurrence.declared.as_ref())?;
    uses.extend_from_slice(container.contributions().0);
    let shared = sources.shared_origin(&uses, complete);
    let parent = occurrence.declared.as_ref().map(|_| {
        ContainerInstanceKey::from_route(occurrence.root_entry, occurrence.parent_route.as_deref())
    });
    let record = match occurrence.opened.under {
        Some(guid) => {
            let step = route
                .into_iter()
                .flat_map(PlacementStep::steps)
                .find(|step| step.has_record)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            Some((guid, step.child.record_selector()?))
        }
        None => None,
    };
    let containing_record_guid = match record {
        Some((guid, selector)) => sources.value_with_shared_origin(
            SourceValue::Present(guid),
            &shared,
            &[selector],
            true,
        )?,
        None => sources.value_with_shared_origin(SourceValue::Absent, &shared, &[], true)?,
    };
    Ok(ReachedHarvestableOccurrence {
        root: sources.value(
            SourceValue::Present(occurrence.root_reference.clone()),
            &occurrence.root_inputs,
            true,
        ),
        id: sources.value_with_shared_origin(
            SourceValue::Present(ContainerInstanceKey::from_route(
                occurrence.root_entry,
                route,
            )),
            &shared,
            &[],
            true,
        )?,
        parent: sources.value_with_shared_origin(
            parent.map_or(SourceValue::Absent, SourceValue::Present),
            &shared,
            &[],
            true,
        )?,
        containing_record_guid,
    })
}
