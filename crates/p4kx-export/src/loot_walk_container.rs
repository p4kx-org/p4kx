// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, FieldOrigin, SourceUse, SourceValue};

use super::{Placed, PlacedTarget, Summary};
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::entry_source::{self, EntrySource};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_container_source::{self, AuthoredEntityLink, EntityReadScope};
use crate::loot_entxml::CapturedEntityLink;
use crate::loot_entxml::documents::{
    LootEntityContainerDocument, SelectedContainer, SelectedLookup,
};
use crate::starmap_index::Index;
use crate::{CatalogError, ExportAccountingError, Kind};

#[derive(Clone, Copy)]
pub(super) enum DocumentOutput {
    Keep(Option<usize>),
    LinksOnly,
    LinkCensus,
}

pub(super) struct ContainerRead {
    pub(super) summary: Summary,
    pub(super) ledger: ExportLedger,
    pub(super) documents: Vec<LootEntityContainerDocument>,
    pub(super) scope_unknown: bool,
    pub(super) route_unknown: bool,
    pub(super) census: Option<Vec<SourceUse>>,
}

pub(super) fn summarise(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    directory: Option<&SourceUse>,
    output: DocumentOutput,
) -> Result<ContainerRead, CatalogError> {
    let (scope, retain_documents) = match output {
        DocumentOutput::Keep(Some(index)) => (EntityReadScope::Exact(index), true),
        DocumentOutput::Keep(None) => (EntityReadScope::All, true),
        DocumentOutput::LinksOnly => (EntityReadScope::All, false),
        DocumentOutput::LinkCensus => (EntityReadScope::Census, false),
    };
    let read = loot_container_source::read(archive, entry, cancel, capture, scope)?;
    let mut ledger = read.ledger;
    let scope_unknown = read.unreadable.is_some() || read.original_scope_unknown;
    let mut documents = Vec::new();
    let mut route_unknown = false;
    let mut summary = Summary {
        unreadable: read.unreadable,
        ..Summary::default()
    };
    for mut authored in read.links {
        let link = &mut authored.link;
        let original = resolve_original(
            link,
            archive,
            index,
            directory,
            capture,
            cancel,
            retain_documents.then_some(&mut documents),
        )?;
        let target = resolve_selected(archive, capture, &mut ledger, link, original.as_ref())?;
        route_unknown |= link.value().container.is_some()
            && matches!(target, PlacedTarget::Missing | PlacedTarget::Ambiguous);
        summary.add(
            archive,
            capture,
            entry,
            &authored,
            target,
            original.as_ref(),
        )?;
        if authored.link.value().several {
            summary.several.push(authored.path);
        }
    }
    summary.nested.sort_unstable();
    summary.nested.dedup();
    Ok(ContainerRead {
        summary,
        ledger,
        documents,
        scope_unknown,
        route_unknown,
        census: read.census,
    })
}

fn resolve_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    link: &CapturedEntityLink,
    lookup: Option<&SelectedLookup<'_>>,
) -> Result<PlacedTarget, CatalogError> {
    let Some(path) = link.value().container.as_deref() else {
        return Ok(PlacedTarget::Missing);
    };
    let selector = link
        .selected_container()
        .ok_or(ExportAccountingError::UnboundHealthSubject)?;
    if selector.as_str() != path {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let lookup = lookup.ok_or(ExportAccountingError::UnboundHealthSubject)?;
    let (candidates, absence_proved) = lookup.target.map_or((lookup.literal, false), |target| {
        (target.candidates, target.absence_proved)
    });
    resolve_target(
        archive,
        capture,
        ledger,
        selector,
        candidates,
        absence_proved,
    )
}

fn resolve_original<'index>(
    link: &mut CapturedEntityLink,
    archive: &Archive,
    index: &'index Index<'_>,
    directory: Option<&SourceUse>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    documents: Option<&mut Vec<LootEntityContainerDocument>>,
) -> Result<Option<SelectedLookup<'index>>, CatalogError> {
    let Some(original) = link.take_original() else {
        return Ok(None);
    };
    let selected = link.selected_container().map(|selector| SelectedContainer {
        selector,
        permit_repair: !link.value().several && !link.value().malformed_config,
    });
    let projected = original.project(archive, index, directory, capture, cancel, selected)?;
    if let Some(documents) = documents {
        documents.push(projected.document);
    }
    Ok(projected.selection)
}

impl Summary {
    fn add(
        &mut self,
        archive: &Archive,
        capture: &EntryCapture<'_>,
        entry: usize,
        authored: &AuthoredEntityLink,
        target: PlacedTarget,
        lookup: Option<&SelectedLookup<'_>>,
    ) -> Result<(), CatalogError> {
        let link = &authored.link;
        if let Some(found) = target.entry() {
            self.nested.push(found);
        }
        if let Some(config) = link.value().config {
            let (path, path_origin) = placement_path(archive, capture, link, target, lookup)?;
            self.add_original_placement(
                entry,
                Placed {
                    author: authored.author,
                    config,
                    config_origin: link.config_origin(capture)?,
                    path,
                    path_origin,
                    target,
                },
            )?;
        }
        Ok(())
    }

    fn add_original_placement(&mut self, entry: usize, row: Placed) -> Result<(), CatalogError> {
        if row.author.containing_entry_index != entry
            || self.placed.last().is_some_and(|previous| {
                previous.author.document_entry_index >= row.author.document_entry_index
            })
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.placed.push(row);
        Ok(())
    }
}

fn placement_path(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    link: &CapturedEntityLink,
    target: PlacedTarget,
    lookup: Option<&SelectedLookup<'_>>,
) -> Result<(String, FieldOrigin), CatalogError> {
    let sources = capture.source_uses();
    let survey = link.placement_survey()?;
    let original = match target {
        PlacedTarget::Resolved(entry) => Some(
            EntrySource::outer(
                capture,
                archive,
                archive
                    .entries()
                    .get(entry)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
            )?
            .receipt()
            .physical_path()?,
        ),
        _ => link
            .selected_container()
            .map(|selector| selector.retain_in(capture))
            .transpose()?,
    };
    let (path, mut uses, complete) = match original {
        Some(original) => {
            let (value, uses, complete) = original.into_parts();
            let SourceValue::Present(path) = value else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            (path.as_str().to_owned(), uses, complete)
        }
        None => (String::new(), Vec::new(), true),
    };
    uses.push(survey.clone());
    let value = sources.origin(&uses, complete && link.original_complete());
    let origin = sources.origin_with_origins(
        std::iter::once(&value).chain(lookup.map(|lookup| &lookup.origin)),
        link.selected_container().is_none() || lookup.is_some(),
    )?;
    Ok((path, origin))
}

fn resolve_target(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    selector: &AuthoredEntryPath,
    candidates: &[usize],
    absence_proved: bool,
) -> Result<PlacedTarget, CatalogError> {
    match candidates {
        [entry] => Ok(PlacedTarget::Resolved(*entry)),
        [] if absence_proved => {
            EntrySource::authored_missing(selector.clone()).record(
                ledger,
                Kind::LootContainers,
                SourceDisposition::VerifiedMissingReference,
            );
            Ok(PlacedTarget::VerifiedMissing)
        }
        [] => {
            EntrySource::authored_missing(selector.clone()).record(
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
                Some(selector),
                candidates,
            )?;
            Ok(PlacedTarget::Ambiguous)
        }
    }
}
