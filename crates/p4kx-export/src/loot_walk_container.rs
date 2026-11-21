// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, Guid, SourceUse};

use super::{Placed, PlacedTarget, Summary, path_of};
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::entry_source::{self, EntrySource};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_entxml::documents::{
    LootEntityContainerDocument, SelectedTarget, literal_candidates,
};
use crate::loot_entxml::{CapturedEntityLink, EntityLink};
use crate::starmap_index::Index;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(super) struct ContainerRead {
    pub(super) summary: Summary,
    pub(super) ledger: ExportLedger,
    pub(super) documents: Vec<LootEntityContainerDocument>,
    pub(super) scope_unknown: bool,
    pub(super) route_unknown: bool,
}

pub(super) fn summarise(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    directory: Option<&SourceUse>,
    entity_index: Option<usize>,
) -> Result<ContainerRead, CatalogError> {
    let read = crate::loot_container_source::read(archive, entry, cancel, capture, entity_index)?;
    let mut ledger = read.ledger;
    let scope_unknown = read.unreadable.is_some() || read.original_scope_unknown;
    let mut documents = Vec::new();
    let mut route_unknown = false;
    let mut summary = Summary {
        unreadable: read.unreadable,
        ..Summary::default()
    };
    for (path, mut link) in read.links {
        let original = retain_original(
            &mut link,
            archive,
            index,
            directory,
            capture,
            cancel,
            &mut documents,
        )?;
        let (link, selected) = link.into_parts();
        let EntityLink {
            container: written,
            config,
            several,
            malformed_config: _,
        } = link;
        if several {
            summary.several.push(path);
        }
        let target = if let Some(path) = written.as_deref() {
            let selector = require_authored_selector(path, selected)?;
            let (candidates, absence_proved) = original.map_or_else(
                || (literal_candidates(index, path).unwrap_or_default(), false),
                |selected| (selected.candidates, selected.absence_proved),
            );
            resolve_target(
                archive,
                capture,
                &mut ledger,
                selector,
                candidates,
                absence_proved,
            )?
        } else {
            PlacedTarget::Missing
        };
        route_unknown |=
            written.is_some() && matches!(target, PlacedTarget::Missing | PlacedTarget::Ambiguous);
        summary.add(archive, config, written, target);
    }
    summary.nested.sort_unstable();
    summary.nested.dedup();
    Ok(ContainerRead {
        summary,
        ledger,
        documents,
        scope_unknown,
        route_unknown,
    })
}

fn require_authored_selector(
    path: &str,
    selected: Option<AuthoredEntryPath>,
) -> Result<AuthoredEntryPath, ExportAccountingError> {
    let selector = selected.ok_or(ExportAccountingError::UnboundHealthSubject)?;
    if selector.as_str() != path {
        return Err(ExportAccountingError::ConflictingSourceDatum);
    }
    Ok(selector)
}

fn retain_original<'index>(
    link: &mut CapturedEntityLink,
    archive: &Archive,
    index: &'index Index<'_>,
    directory: Option<&SourceUse>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    documents: &mut Vec<LootEntityContainerDocument>,
) -> Result<Option<SelectedTarget<'index>>, CatalogError> {
    let Some(original) = link.take_original() else {
        return Ok(None);
    };
    let selected = if link.value().several || link.value().malformed_config {
        None
    } else {
        link.selected_container()
    };
    let projected = original.project(archive, index, directory, capture, cancel, selected)?;
    documents.push(projected.document);
    Ok(projected.selected_target)
}

impl Summary {
    fn add(
        &mut self,
        archive: &Archive,
        config: Option<Guid>,
        written: Option<String>,
        target: PlacedTarget,
    ) {
        if let Some(found) = target.entry() {
            self.nested.push(found);
        }
        if let Some(config) = config {
            self.placed
                .push(configured_placement(archive, config, written, target));
        }
    }
}

fn configured_placement(
    archive: &Archive,
    config: Guid,
    written: Option<String>,
    target: PlacedTarget,
) -> Placed {
    let path = target.entry().map_or_else(
        || written.unwrap_or_default(),
        |found| path_of(archive, found).to_owned(),
    );
    Placed {
        config,
        path,
        target,
    }
}

fn resolve_target(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    selector: AuthoredEntryPath,
    candidates: &[usize],
    absence_proved: bool,
) -> Result<PlacedTarget, CatalogError> {
    match candidates {
        [entry] => Ok(PlacedTarget::Resolved(*entry)),
        [] if absence_proved => {
            EntrySource::authored_missing(selector).record(
                ledger,
                Kind::LootContainers,
                SourceDisposition::VerifiedMissingReference,
            );
            Ok(PlacedTarget::VerifiedMissing)
        }
        [] => {
            EntrySource::authored_missing(selector).record(
                ledger,
                Kind::LootContainers,
                SourceDisposition::ReadFailed,
            );
            Ok(PlacedTarget::Missing)
        }
        candidates => {
            entry_source::record_ambiguous_outer_entries(
                archive,
                capture,
                ledger,
                Kind::LootContainers,
                Some(&selector),
                candidates,
            )?;
            Ok(PlacedTarget::Ambiguous)
        }
    }
}
